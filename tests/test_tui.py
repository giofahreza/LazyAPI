"""Exercise real terminal input with isolated collections and a loopback HTTP server.

Only the Python standard library is needed. Resize requests force a complete frame,
so assertions can inspect terminal output without an emulator dependency.
"""
import copy
import codecs
import fcntl
import http.server
import json
import os
from pathlib import Path
import pty
import re
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time
import unittest
from terminal_screen import TerminalScreen

REPOSITORY = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get("LAZYAPI_BIN", REPOSITORY / "target/debug/lazyapi")).resolve()
CSI = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")


class TerminalApp:
    def __init__(self, directory, extra_env=None):
        self.master, slave = pty.openpty()
        self.width, self.height = 160, 42
        self.terminal = TerminalScreen(self.width, self.height)
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", self.height, self.width, 0, 0))
        env = {**os.environ, "TERM": "xterm-256color", "XDG_CONFIG_HOME": str(directory / "config")}
        for name in ("LAZYAPI_MAIN_DIR", "LAZYAPI_THEME", "LAZYAPI_KEY_BINDINGS"):
            env.pop(name, None)
        env.update(extra_env or {})
        self.process = subprocess.Popen(
            [str(BINARY), "--directory", str(directory)], cwd=directory, env=env,
            stdin=slave, stdout=slave, stderr=slave, start_new_session=True,
        )
        os.close(slave)
        self.output = ""
        try:
            self.wait_for("LazyAPI")
            self.read(0.4)
        except Exception:
            self.close()
            raise

    def read(self, duration=0.25):
        chunks = []
        end = time.monotonic() + duration
        while time.monotonic() < end:
            if select.select([self.master], [], [], max(0, min(0.05, end - time.monotonic())))[0]:
                try:
                    chunk = os.read(self.master, 65536)
                except OSError:
                    break
                if not chunk:
                    break
                chunks.append(chunk)
        raw = self.decoder.decode(b"".join(chunks))
        self.terminal.feed(raw)
        text = CSI.sub("", raw)
        self.output += text
        return text

    def wait_for(self, text, timeout=12):
        end = time.monotonic() + timeout
        while text not in self.output and time.monotonic() < end:
            self.read(0.1)
            if self.process.poll() is not None:
                raise AssertionError(f"App exited {self.process.returncode}: {self.output[-4000:]}")
        if text not in self.output:
            raise AssertionError(f"Timed out waiting for {text!r}: {self.output[-4000:]}")

    def press(self, keys):
        os.write(self.master, keys.encode() if isinstance(keys, str) else keys)
        self.read(0.25)
        self.assert_alive()

    def screen(self, width=None, height=None):
        self.read(0.1)
        self.width = width if width is not None else (161 if self.width == 160 else 160)
        self.height = height if height is not None else self.height
        self.terminal.resize(self.width, self.height)
        fcntl.ioctl(self.master, termios.TIOCSWINSZ, struct.pack("HHHH", self.height, self.width, 0, 0))
        os.kill(self.process.pid, signal.SIGWINCH)
        text = self.read(0.6)
        self.assert_alive()
        return text

    def assert_alive(self):
        if self.process.poll() is not None:
            raise AssertionError(f"App exited {self.process.returncode}: {self.output[-4000:]}")

    def close(self):
        if self.master is None:
            return
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=3)
        os.close(self.master)
        self.master = None


@unittest.skipUnless(sys.platform.startswith("linux"), "PTY checks use Linux terminal ioctls")
class TerminalWorkflowTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="lazyapi-tui-test-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        source = json.loads((REPOSITORY / "example_resources/collections/HTTP_methods.json").read_text())["requests"][0]
        self.requests = []
        for name in ("One", "Two", "Three"):
            request = copy.deepcopy(source)
            request["name"] = name
            request["url"] = f"http://127.0.0.1:9/{name.lower()}"
            request["headers"] = [{"enabled": True, "data": ["X-Test", "original"]}]
            request["params"] = [{"enabled": True, "data": ["first", name]}, {"enabled": True, "data": ["second", "keep"]}]
            request["response"] = {"duration": "1ms", "status_code": "200 OK", "content": "\n".join(f"{name.upper()}_ROW_{i:03d}" for i in range(80)), "headers": [], "cookies": None}
            self.requests.append(request)
        self.write_fixtures()
        (self.root / "lazyapi.toml").write_text("save_requests_response = true\n")
        (self.root / ".env.test").write_text('BASE_URL="http://127.0.0.1:9"\n')

    def write_fixtures(self):
        for index, (name, requests) in enumerate((("First", self.requests[:2]), ("Second", self.requests[2:]))):
            (self.root / f"{name}.json").write_text(json.dumps({"name": name, "last_position": index, "requests": requests}))

    def start(self, env=None):
        app = TerminalApp(self.root, env)
        self.addCleanup(app.close)
        return app

    def saved(self, name="First"):
        return json.loads((self.root / f"{name}.json").read_text())["requests"]

    def open_first(self, app):
        app.press("jl j".replace(" ", ""))  # first collection -> expand -> first request
        app.press(b"\r")
        self.assertIn("Requests 1/1", app.screen())

    def open_second(self, app):
        app.press(b"\x08j\r")  # Ctrl+h, next tree row, Enter
        self.assertIn("Requests 2/2", app.screen())

    def assert_theme_text_and_cursor(self, app, insert=False):
        cells = [cell for row in app.terminal.cells for cell in row]
        text_cells = [cell for cell in cells if any(
            not ch.isspace() and not "\u2500" <= ch <= "\u259f" for ch in cell[0]
        )]
        self.assertTrue(text_cells)
        palette = {(7, 11, 18), (12, 19, 30), (17, 28, 43), (43, 66, 86), (15, 34, 44),
                   (184, 235, 229), (58, 72, 84), (215, 226, 237), (105, 126, 146),
                   (99, 230, 216), (101, 167, 255), (26, 55, 70), (92, 200, 150),
                   (244, 184, 96), (255, 107, 122), (130, 170, 255), 15}
        self.assertTrue({cell[1] for cell in text_cells}.issubset(palette), "All text uses the LazyDB palette or solid-white shortcut hints")
        cursor_color = (101, 167, 255) if insert else (215, 226, 237)
        painted_cursors = [cell for cell in cells if cell[2] == cursor_color]
        if insert:
            # Insert mode: the real terminal draws a block cursor; edtui does
            # not repaint the cell, so there should be zero painted cells and
            # the terminal cursor must have been moved to the editor.
            self.assertNotEqual((app.terminal.x, app.terminal.y), (0, 0),
                                "Terminal cursor was never moved to the focused editor")
            self.assertEqual(len(painted_cursors), 0,
                             f"Expected zero painted cursor cells in Insert, found {len(painted_cursors)}:\n{app.terminal.text}")
        else:
            # Normal / Visual mode: the cursor is still the painted white cell
            # (the old behaviour). Exactly one cell should be repainted and the
            # terminal cursor should not have been moved onto the editor.
            self.assertEqual(len(painted_cursors), 1,
                             f"Expected one painted text cursor in Normal:\n{app.terminal.text}")
            self.assertEqual(painted_cursors[0][1], (7, 11, 18))

    def test_compact_tabs_and_white_shortcuts_share_bottom_branding_after_resize(self):
        app = self.start()
        self.open_first(app)
        self.open_second(app)
        for width in (160, 100, 76, 56, 160):
            with self.subTest(width=width):
                app.screen(width=width, height=28)
                rows = app.terminal.text.splitlines()
                self.assertIn("GET One ×", rows[0])
                self.assertIn("GET Two ×", rows[0])
                self.assertNotIn("LazyAPI", "\n".join(rows[:-1]))
                self.assertTrue(rows[-1].startswith(" LazyAPI "))
                hint_start = rows[-1].index("? help")
                hint_end = len(rows[-1].rstrip())
                self.assertTrue(all(cell[1] == 15 for cell in app.terminal.cells[-1][hint_start:hint_end]))
                active_tab = [cell for cell in app.terminal.cells[0] if cell[2] == (99, 230, 216)]
                self.assertEqual("".join(cell[0] for cell in active_tab), " GET Two × ")
                self.assertTrue(all(cell[1] == (7, 11, 18) for cell in active_tab))
                # The unused tab strip stays clear instead of a stretched tab or duplicate title.
                tab_start = rows[0].index("GET One") - 1
                self.assertFalse(rows[1][tab_start:].strip())
                self.assertIn("Method", rows[2])

        app.press("?")
        rows = app.terminal.text.splitlines()
        row_index = next(i for i, row in enumerate(rows) if "j/k scroll" in row)
        start = rows[row_index].index("j/k scroll")
        end = rows[row_index].index("close", start) + len("close")
        self.assertTrue(all(cell[1] == 15 for cell in app.terminal.cells[row_index][start:end]))

    def test_long_unicode_tabs_and_section_tabs_keep_active_label_visible(self):
        for index, prefix in enumerate(("One_日本語_", "Two_配置_")):
            self.requests[index]["name"] = prefix + "long_request_name_" * 8
        self.write_fixtures()
        app = self.start()
        self.open_first(app)
        self.open_second(app)
        for width in (160, 100, 76, 56, 160):
            app.screen(width=width, height=28)
            for key, prefix in (("[", "One_"), ("]", "Two_")):
                app.press(key)
                active = "".join(cell[0] for cell in app.terminal.cells[0] if cell[2] == (99, 230, 216))
                self.assertTrue(active.startswith(" GET " + prefix), active)
                self.assertIn("…", active)
                self.assertIn("‹", app.terminal.text.splitlines()[0])
                self.assertIn("›", app.terminal.text.splitlines()[0])
            app.press(b"\t\t\t\t")  # Scripts at the right edge of the section strip.
            self.assertIn("Scripts", app.terminal.text.splitlines()[6])
            self.assertIn("━", app.terminal.text.splitlines()[7])
            app.press(b"\t")  # Return to Params before the next resize.
        self.assertEqual(self.saved()[0]["name"], self.requests[0]["name"])
        self.assertEqual(self.saved()[1]["name"], self.requests[1]["name"])

    def test_narrow_layout_follows_focused_pane_and_restores_wide_split(self):
        app = self.start()
        self.open_first(app)
        app.screen(width=76, height=22)
        self.assertIn("▶ Parameters", app.terminal.text)
        self.assertNotIn("Collections", app.terminal.text)
        self.assertNotIn("ONE_ROW_000", app.terminal.text)
        app.press(b"\x0c")  # Ctrl+l reveals the response at full width.
        self.assertIn("▶ Response", app.terminal.text)
        self.assertIn("ONE_ROW_000", app.terminal.text)
        self.assertNotIn("Parameters", app.terminal.text)
        app.press(b"\x08\x08")  # Ctrl+h twice reveals Collections.
        self.assertIn("Collections", app.terminal.text)
        self.assertNotIn("Method", app.terminal.text)
        app.press(b"\x0c")
        self.assertIn("▶ Parameters", app.terminal.text)
        app.screen(width=160, height=42)
        self.assertIn("Collections", app.terminal.text)
        self.assertIn("Parameters", app.terminal.text)
        self.assertIn("ONE_ROW_000", app.terminal.text)

    def test_body_types_open_normal_and_show_each_typed_character_without_repaint(self):
        self.requests[0]["protocol"]["body"] = {"raw": ""}
        self.write_fixtures()
        app = self.start({"NO_COLOR": "1"})
        self.open_first(app)
        app.press(b"\t\t\t")
        for index, body_type in enumerate(("raw", "json", "xml", "html", "javascript")):
            with self.subTest(body_type=body_type):
                if index:
                    app.press(b"\x02")  # changing type should also focus its Normal editor
                self.assertIn("NORMAL · Edit body", app.terminal.text)
                self.assert_theme_text_and_cursor(app)
                saved = self.saved()[0]["protocol"]["body"]
                app.press("i")
                typed = ""
                for char in "DRAFT":
                    app.press(char)
                    typed += char
                    self.assertIn(typed, app.terminal.text)
                self.assert_theme_text_and_cursor(app, insert=True)
                self.assertEqual(self.saved()[0]["protocol"]["body"], saved)
                app.press(b"\x1b")
                self.assert_theme_text_and_cursor(app)
                app.press(b"\x03")
                self.assertEqual(self.saved()[0]["protocol"]["body"], saved)

    def test_body_cursor_survives_horizontal_vertical_scroll_resize_and_refocus(self):
        self.requests[0]["protocol"]["body"] = {"raw": "\n".join("row" + str(i) + "x" * 120 for i in range(70))}
        self.write_fixtures()
        app = self.start()
        self.open_first(app)
        app.press(b"\t\t\t")
        app.press("GA_SCROLL_LIVE")
        self.assertIn("_SCROLL_LIVE", app.terminal.text)
        self.assert_theme_text_and_cursor(app, insert=True)
        app.screen(width=76, height=22)
        self.assertIn("_SCROLL_LIVE", app.terminal.text)
        self.assert_theme_text_and_cursor(app, insert=True)
        app.press(b"\x1b")
        app.press(b"\x0c\x08")  # leave body for response; return in Normal
        self.assertIn("NORMAL · Edit body", app.terminal.text)
        self.assert_theme_text_and_cursor(app)
        self.assertTrue(self.saved()[0]["protocol"]["body"]["raw"].endswith("_SCROLL_LIVE"))

    def test_insert_cursor_follows_viewport_when_moving_up_from_scrolled_bottom(self):
        self.requests[0]["protocol"]["body"] = {"raw": "\n".join(f"row{i}" for i in range(70))}
        self.write_fixtures()
        app = self.start()
        self.open_first(app)
        app.press(b"\t\t\t")
        app.press("G")
        app.press("i")
        bottom_y = app.terminal.y

        # Move the editor cursor up one row in Normal mode, then re-enter
        # Insert mode. edtui keeps its viewport anchored while the row is
        # still visible; the terminal cursor must follow that row rather than
        # being recomputed and pinned to the bottom of the viewport.
        app.press(b"\x1b")
        app.press("k")
        app.press("i")
        self.assertEqual(app.terminal.y, bottom_y - 1)

    def test_single_line_auth_form_script_and_jwt_editors_render_live_themed_text(self):
        auth_examples = json.loads((REPOSITORY / "example_resources/collections/auth.json").read_text())["requests"]
        digest = next(request["auth"] for request in auth_examples if isinstance(request["auth"], dict) and "digest" in request["auth"])
        basic = {"basic_auth": {"username": "username", "password": "password"}}
        jwt = {"jwt_token": {"algorithm": "HS256", "secret_type": "Text", "secret": "secret", "payload": '{"name": "value"}'}}
        cases = [
            ("URL", b"\x0b\r", None, None),
            ("parameter", b"l\r", None, None),
            ("header", b"\t\tl\r", None, None),
            ("file", b"\t\t\t", None, {"file": "/tmp/example.txt"}),
            ("form", b"\t\t\tl\r", None, {"form": [{"enabled": True, "data": ["key", "value"]}]}),
            ("username", b"\t\r", basic, None),
            ("password", b"\tj\r", basic, None),
            ("bearer", b"\t\r", {"bearer_token": {"token": "token"}}, None),
            ("JWT secret", b"\tjj\r", jwt, None),
            ("JWT payload", b"\tjjj\r", jwt, None),
            ("digest", b"\tjjjjj\r", digest, None),
            ("pre-script", b"\t\t\t\t\r", None, None),
            ("post-script", b"\t\t\t\tj\r", None, None),
        ]
        for name, keys, auth, body in cases:
            with self.subTest(input=name):
                self.requests[0]["auth"] = copy.deepcopy(auth) if auth else "no_auth"
                self.requests[0]["protocol"]["body"] = copy.deepcopy(body) if body else "no_body"
                self.requests[0]["scripts"] = {"pre_request_script": "const before = 1;", "post_request_script": "const after = 2;"}
                self.write_fixtures()
                app = self.start()
                try:
                    self.open_first(app)
                    before = (self.root / "First.json").read_bytes()
                    app.press(keys)
                    self.assertIn("NORMAL · Edit", app.terminal.text)
                    self.assert_theme_text_and_cursor(app)
                    app.press("IWHITE_INPUT")
                    self.assertIn("WHITE_INPUT", app.terminal.text)
                    self.assert_theme_text_and_cursor(app, insert=True)
                    self.assertEqual((self.root / "First.json").read_bytes(), before)
                    app.press(b"\x03")
                    self.assertEqual((self.root / "First.json").read_bytes(), before)
                finally:
                    app.close()

    def test_environment_and_dialog_inputs_keep_visible_cursors_and_themed_drafts(self):
        app = self.start()
        app.press(b"\x05l\r")
        self.assertIn("NORMAL · Edit environment value", app.terminal.text)
        self.assert_theme_text_and_cursor(app)
        app.press("IENV_DRAFT")
        self.assertIn("ENV_DRAFT", app.terminal.text)
        self.assert_theme_text_and_cursor(app, insert=True)
        app.press(b"\x03\x1b")
        self.assertNotIn("ENV_DRAFT", (self.root / ".env.test").read_text())
        app.press(b"a\r")
        self.assertIn("INSERT · New collection", app.terminal.text)
        self.assert_theme_text_and_cursor(app, insert=True)
        app.press(b"\x1b")
        self.assert_theme_text_and_cursor(app)
        app.press("iCOLLECTION_DRAFT")
        self.assertIn("COLLECTION_DRAFT", app.terminal.text)
        self.assert_theme_text_and_cursor(app, insert=True)
        app.press(b"\x03")
        self.open_first(app)
        app.press(b"\x08r")
        app.press(b"\x1b")
        self.assertIn("NORMAL · Rename request", app.terminal.text)
        self.assert_theme_text_and_cursor(app)
        app.press("IRENAME_DRAFT")
        self.assertIn("RENAME_DRAFT", app.terminal.text)
        self.assert_theme_text_and_cursor(app, insert=True)
        app.press(b"\x03")
        self.assertEqual(self.saved()[0]["name"], "One")

    def test_websocket_message_opens_normal_and_renders_live_draft(self):
        self.requests[0]["protocol"] = {"type": "websocket", "message_type": {"type": "text", "content": ""}}
        self.write_fixtures()
        app = self.start()
        self.open_first(app)
        app.press(b"\t\t\t")
        self.assertIn("NORMAL · Edit message", app.terminal.text)
        self.assert_theme_text_and_cursor(app)
        app.press("iMESSAGE_DRAFT")
        self.assertIn("MESSAGE_DRAFT", app.terminal.text)
        self.assert_theme_text_and_cursor(app, insert=True)
        self.assertEqual(self.saved()[0]["protocol"]["message_type"]["content"], "")
        app.press(b"\x13")
        self.assertEqual(self.saved()[0]["protocol"]["message_type"]["content"], "MESSAGE_DRAFT")
        self.assertEqual(self.saved()[0]["protocol"]["messages"], [])

    def test_navigation_panes_tabs_and_close_preserve_requests(self):
        app = self.start()
        self.open_first(app)
        self.open_second(app)
        app.press("[")
        screen = app.screen()
        self.assertIn("Requests 1/2", screen)
        self.assertIn("ONE_ROW_000", screen)
        app.press(b"\x0c")  # Ctrl+l
        self.assertIn("NORMAL · Response", app.screen())
        app.press("j")
        self.assertIn("ONE_ROW_001", app.screen())
        app.press("G")
        self.assertIn("ONE_ROW_079", app.screen())
        app.press("gg")
        self.assertIn("ONE_ROW_000", app.screen())
        app.press(b"\x0b")  # Ctrl+k
        self.assertIn("NORMAL · URL", app.screen())
        app.press(b"\x0a")  # Ctrl+j must differ from Enter
        self.assertIn("NORMAL · Parameters", app.screen())
        app.press("]X")
        self.assertIn("Requests 1/1", app.screen())
        app.press("X")
        self.assertIn("No open requests", app.screen())
        self.assertEqual([r["name"] for r in self.saved()], ["One", "Two"])

    def test_insert_literal_keys_cancel_and_save_on_pane_and_tab_navigation(self):
        app = self.start()
        self.open_first(app)
        self.open_second(app)
        app.press("[")
        app.press("li")  # select parameter value and enter Insert
        self.assertIn("INSERT · Edit query value", app.screen())
        app.press("[]?")
        self.assertNotIn("Keyboard shortcuts", app.screen())
        app.press(b"\x03")  # cancel, never quit the app
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "One")
        app.press("i")
        app.press(b"\x1b")
        self.assertIn("NORMAL · Edit query value", app.screen())
        app.press("0Cvalue[]?")
        app.press(b"\x08")  # pane switch saves while Insert
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "value[]?")
        app.press(b"\x0c")
        app.press("i")
        app.press(b"\x1b")
        app.press("0Creplaced")
        app.press(b"\x1b")
        app.press("]")
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "replaced")
        self.assertEqual(self.saved()[1]["params"][0]["data"][1], "Two")

    def test_enter_ctrl_s_escape_and_vim_undo(self):
        app = self.start()
        self.open_first(app)
        app.press("li")
        app.press(b"\x1b")
        app.press("0Center-saved")
        app.press(b"\r")
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "enter-saved")
        app.press("i")
        app.press(b"\x1b")
        app.press("0Cdiscard")
        app.press(b"\x1b")
        app.press("u")
        app.press(b"\x13")
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "enter-saved")
        app.press("i")
        app.press(b"\x1b")
        app.press("0Cescape-saved")
        app.press(b"\x1b")
        app.press(b"\x1b")
        self.assertIn("NORMAL · Edit query value [+]", app.screen())
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "enter-saved")
        app.press(":wq\r")
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "escape-saved")

    def test_help_scrolls_resizes_and_restores_editor(self):
        app = self.start()
        self.open_first(app)
        app.press("i")
        app.press(b"\x1b")
        app.press("?")
        screen = app.screen()
        self.assertIn("Keyboard shortcuts", screen)
        self.assertIn("MODES AND EDITING", screen)
        app.press("G")
        self.assertIn("CURRENT CONTEXT", app.screen())
        app.screen(width=60, height=14)
        app.press("g")
        self.assertIn("Keyboard shortcuts", app.screen(width=61))
        app.press("?")
        app.screen(width=160, height=42)
        self.assertNotIn("Keyboard shortcuts", app.screen())
        app.press("ihello")
        app.press(b"\x03")
        self.assertEqual(self.saved()[0]["params"][0]["data"][0], "first")

    def test_enter_inspects_and_insert_append_commands_position_the_cursor(self):
        app = self.start()
        self.open_first(app)
        app.press(b"l\r")
        self.assertIn("NORMAL · Edit query value", app.screen())
        app.press("lai")  # append after 'n' in One
        app.press(b"\x1b")
        app.press("a!")  # Escape leaves the cursor on the last inserted char
        app.press(b"\x1b")
        app.press("Ipre-")
        app.press(b"\x1b")
        app.press("A-end")
        app.press(b"\x13")
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "pre-Oni!e-end")
        # Workspace entry aliases have the same start/end positioning.
        app.press("Istart-")
        app.press(b"\x13")
        app.press("A-finish")
        app.press(b"\x13")
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "start-pre-Oni!e-end-finish")

    def test_write_keeps_undo_and_quit_guards_unsaved_changes(self):
        app = self.start()
        self.open_first(app)
        app.press(b"l\r")
        app.press("ccsaved-value")
        app.press(b"\x1b")
        app.press(":w\r")
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "saved-value")
        screen = app.screen()
        self.assertIn("NORMAL · Edit query value", screen)
        self.assertNotIn("[+]", screen)
        app.press("u")
        self.assertIn("NORMAL · Edit query value [+]", app.screen())
        app.press(":q\r")
        self.assertIn("Unsaved changes", app.screen())
        app.press(":q!\r")

        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "saved-value")
        self.assertIn("NORMAL · Parameters", app.screen())
        app.press(b"\r")
        app.press("ccbad-command-draft")
        app.press(b"\x1b")
        app.press(":notacommand\r")
        self.assertIn("Unknown command", app.screen())
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "saved-value")
        app.press(":wq")
        app.press(b"\x1b")  # Escape cancels the command, not the field
        self.assertIn("NORMAL · Edit query value [+]", app.screen())
        app.press(":wq\r")
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "bad-command-draft")

    def test_single_line_paste_and_multiline_commands_cannot_submit_a_field(self):
        app = self.start()
        self.open_first(app)
        app.press(b"l\r")
        app.press("oO")  # opening lines is disabled in a single-line value
        self.assertIn("NORMAL · Edit query value", app.screen())
        app.press("cc")
        app.press("\x1b[200~one\r\ntwo[]?:wq\x1b[201~")
        self.assertIn("INSERT · Edit query value [+]", app.screen())
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "One")
        app.press(b"\x13")
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "one two[]?:wq")

    def test_inner_brackets_backward_delete_and_search_do_not_trigger_workspace(self):
        self.requests[0]["params"][0]["data"][1] = "[first] keep keep"
        self.write_fixtures()
        app = self.start()
        self.open_first(app)
        self.open_second(app)
        app.press("[")
        app.press(b"l\r")
        app.press("ci[changed")
        self.assertIn("INSERT · Edit query value", app.screen())
        self.assertIn("Requests 1/2", app.screen())
        app.press(b"\x1b")
        app.press("$X")
        self.assertIn("Requests 1/2", app.screen())
        app.press("/keep")
        self.assertIn("SEARCH · Edit query value", app.screen())
        app.press(b"\r")
        self.assertIn("NORMAL · Edit query value [+]", app.screen())
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "[first] keep keep")
        app.press(":wq\r")
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "[changed] keep kep")

    def test_nested_multiline_and_empty_text_objects_keep_their_delimiters(self):
        self.requests[0]["protocol"]["body"] = {"raw": '{\n  "nested": {"value": "}"}\n}'}
        self.write_fixtures()
        app = self.start()
        self.open_first(app)
        app.press(b"\t\t\t\r")
        app.press("ci{replacement")
        self.assertIn("INSERT · Edit body", app.screen())
        app.press(b"\x13")
        self.assertEqual(self.saved()[0]["protocol"]["body"]["raw"], "{replacement}")
        app.press(b"\r")
        app.press("di{")
        self.assertIn("NORMAL · Edit body [+]", app.screen())
        app.press("ci{empty-pair")
        app.press(b"\x1b")
        app.press(":w\r")
        self.assertEqual(self.saved()[0]["protocol"]["body"]["raw"], "{empty-pair}")
        app.press("ci(")  # unmatched object is a no-op
        self.assertIn("NORMAL · Edit body", app.screen())
        app.press(":q\r")
        self.assertEqual(self.saved()[0]["protocol"]["body"]["raw"], "{empty-pair}")

    def test_window_prefix_insert_word_delete_and_normal_panel_navigation(self):
        app = self.start()
        self.open_first(app)
        app.press("li")
        app.press(b"\x1b")
        app.press("cckeep remove")
        app.press(b"\x17")  # Insert Ctrl+w deletes a word
        app.press("word")
        app.press(b"\x1b")
        app.press(b"\x17")  # Normal Ctrl+w starts window prefix
        self.assertIn("Ctrl+w · h/j/k/l", app.screen())
        app.press(b"\x1b")
        self.assertIn("NORMAL · Edit query value [+]", app.screen())
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "One")
        app.press(b"\x17l")
        self.assertIn("NORMAL · Response", app.screen())
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "keep word")
        app.press(b"\x17h")
        app.press(b"\r")
        app.press("A-panel")
        app.press(b"\x1b")
        app.press(b"\t\t")  # save field and advance through Auth to Headers
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "keep word-panel")
        app.press("lA-header")
        app.press(b"\x13")
        self.assertEqual(self.saved()[0]["headers"][0]["data"][1], "original-header")

    def test_f1_and_bracketed_paste_preserve_mode_and_literal_multiline_text(self):
        self.requests[0]["protocol"]["body"] = {"raw": ""}
        self.write_fixtures()
        app = self.start()
        self.open_first(app)
        app.press(b"\t\t\ti")
        app.press(b"\x1bOP")  # F1 opens help even in Insert
        self.assertIn("Keyboard shortcuts", app.screen())
        app.press(b"\x1b")
        self.assertIn("INSERT · Edit body", app.screen())
        payload = "first line\n[]?:wq\nq hjkl"
        app.press("\x1b[200~" + payload + "\x1b[201~")
        self.assertIn("INSERT · Edit body [+]", app.screen())
        self.assertEqual(self.saved()[0]["protocol"]["body"]["raw"], "")
        app.press(b"\x13")
        self.assertEqual(self.saved()[0]["protocol"]["body"]["raw"], payload)

    def test_environment_save_stays_on_same_row_and_rejects_duplicate_key(self):
        env = self.root / ".env.test"
        env.write_text('FIRST="one"\nSECOND="two"\n')
        app = self.start()
        app.press(b"\x05")  # Ctrl+e environment editor
        app.press(b"jl\r")
        self.assertIn("NORMAL · Edit environment value", app.screen())
        app.press("ccsaved")
        app.press(b"\x1b")
        app.press(":w\r")
        self.assertIn('SECOND=saved', env.read_text())
        app.press("A-again")
        app.press(b"\x13")
        self.assertIn('FIRST=one', env.read_text())
        self.assertIn('SECOND=saved-again', env.read_text())
        app.press(b"h\r")
        app.press("ccFIRST")
        app.press(b"\x1b")
        app.press(":wq\r")
        self.assertIn("Could not save", app.screen())
        self.assertIn('SECOND=saved-again', env.read_text())
        app.press(":q!\r")

    def test_environment_manager_creates_first_environment_and_edits_variables(self):
        (self.root / ".env.test").unlink()
        app = self.start()
        collections = [self.saved("First"), self.saved("Second")]
        app.press(b"\x05")
        self.assertIn("No environments yet", app.terminal.text)
        app.press("a")
        self.assertIn("INSERT · New environment", app.terminal.text)
        self.assert_theme_text_and_cursor(app, insert=True)
        app.press("Local Development\r")
        path = self.root / ".env.Local Development"
        self.assertTrue(path.is_file())
        self.assertIn("Local Development [active]", app.terminal.text)
        app.press(b"a\r")
        self.assertIn("NORMAL · Edit environment key", app.terminal.text)
        app.press("ccBASE_URL")
        app.press(b"\x13lihttps://example.test\x13")
        self.assertIn("BASE_URL=https://example.test", path.read_text())
        app.press("a")
        self.assertIn("KEY_1=", path.read_text())
        app.press("d")
        self.assertNotIn("KEY_1", path.read_text())
        saved = path.read_bytes()
        app.press("R")
        app.press(b"\x1b")
        app.press("ccStaging")
        app.press(b"\x13")
        renamed = self.root / ".env.Staging"
        self.assertFalse(path.exists())
        self.assertEqual(renamed.read_bytes(), saved)
        self.assertIn("Staging [active]", app.terminal.text)
        app.close()
        app = self.start()
        app.press(b"\x05")
        self.assertIn("BASE_URL", app.terminal.text)
        self.assertIn("https://example.test", app.terminal.text)
        self.assertEqual([self.saved("First"), self.saved("Second")], collections)

    def test_environment_manager_browses_without_activating_and_keeps_correct_target(self):
        zeta = self.root / ".env.zeta"
        zeta.write_text("TOKEN=other\n")
        test = self.root / ".env.test"
        saved = test.read_bytes()
        app = self.start()
        app.press(b"\x05\x08j")
        self.assertIn("test [active]", app.terminal.text)
        self.assertNotIn("zeta [active]", app.terminal.text)
        app.press(b"\x0cl\r")
        app.press("ccchanged")
        app.press(b"\x08")  # changing focus saves the edited, inactive environment
        self.assertEqual(zeta.read_text(), "TOKEN=changed\n")
        self.assertEqual(test.read_bytes(), saved)
        self.assertIn("test [active]", app.terminal.text)
        app.press(b"\r")
        self.assertIn("zeta [active]", app.terminal.text)
        app.press(b"\x08kd")  # delete inactive test; active zeta must stay active
        self.assertIn("Delete environment?", app.terminal.text)
        app.press(b"l\r")
        self.assertFalse(test.exists())
        self.assertTrue(zeta.exists())
        self.assertIn("zeta [active]", app.terminal.text)

    def test_environment_manager_delete_confirmation_empty_state_and_help(self):
        path = self.root / ".env.test"
        saved = path.read_bytes()
        app = self.start()
        app.press(b"\x05D")
        app.press(b"\r")  # default is Cancel
        self.assertEqual(path.read_bytes(), saved)
        app.press("D")
        app.press(b"\x1b")
        self.assertEqual(path.read_bytes(), saved)
        app.press("?")
        self.assertIn("ENVIRONMENT MANAGER", app.terminal.text)
        app.press(b"\x1b")
        app.press("D")
        app.press(b"l\r")
        self.assertFalse(path.exists())
        self.assertIn("No environments yet", app.terminal.text)
        app.screen(width=58, height=20)
        self.assertIn("Environments", app.terminal.text)
        app.press("aRestored\r")
        self.assertTrue((self.root / ".env.Restored").exists())
        app.press(b"\x1b")
        app.press(b"\x05")
        self.assertIn("Restored", app.terminal.text)

    def test_environment_dialog_rejects_duplicates_and_invalid_names_without_losing_draft(self):
        path = self.root / ".env.test"
        saved = path.read_bytes()
        app = self.start()
        app.press(b"\x05Ntest\r")
        self.assertIn("already exists", app.terminal.text)
        self.assertIn("INSERT · New environment", app.terminal.text)
        self.assertEqual(path.read_bytes(), saved)
        app.press(b"\x1b")
        app.press("cc../outside\r")
        self.assertIn("without slashes", app.terminal.text)
        self.assertIn("../outside", app.terminal.text)
        app.press(b"\x03")
        self.assertIn("NORMAL · Environments", app.terminal.text)
        self.assertEqual(path.read_bytes(), saved)

    def test_environment_new_variable_never_overwrites_existing_generated_key(self):
        path = self.root / ".env.test"
        path.write_text("KEY_1=keep\n")
        app = self.start()
        app.press(b"\x05a")
        self.assertEqual(path.read_text(), "KEY_1=keep\nKEY_2=\n")
        app.press(b"\r")
        app.press("ccINVALID=KEY\r")
        self.assertIn("Variable names", app.terminal.text)
        self.assertIn("INVALID=KEY", app.terminal.text)
        self.assertEqual(path.read_text(), "KEY_1=keep\nKEY_2=\n")
        app.press(b"\x03")

    def test_environment_write_failure_keeps_draft_and_allows_retry(self):
        path = self.root / ".env.test"
        backup = self.root / "environment-backup"
        app = self.start()
        app.press(b"\x05l\r")
        app.press("ccRETRY_VALUE")
        path.rename(backup)
        path.mkdir()  # make replacing this path fail without touching real user data
        app.press(b"\x13")
        self.assertIn("Could not save", app.terminal.text)
        self.assertIn("RETRY_VALUE", app.terminal.text)
        self.assertIn("INSERT · Edit environment value", app.terminal.text)
        self.assertEqual(list(self.root.glob(".lazyapi-env-*.tmp")), [])
        path.rmdir()
        backup.rename(path)
        app.press(b"\x13")
        self.assertEqual(path.read_text(), "BASE_URL=RETRY_VALUE\n")

    def test_environment_placeholders_with_system_key_names_render_without_crashing(self):
        path = self.root / ".env.test"
        values = "PATH=local-path\nNOW=local-time\nALIASES={{PATH}}/{{NOW}}/{{PATH}}\n"
        path.write_text(values)
        app = self.start()
        app.press(b"\x05")
        self.assertIn("{{PATH}}/{{NOW}}/{{PATH}}", app.terminal.text)
        app.press(b"jjl\r")
        self.assert_theme_text_and_cursor(app)
        app.press(b"\x03")
        self.assertEqual(path.read_text(), values)

    def test_environment_deleted_during_request_is_not_restored_by_its_script(self):
        release, started = threading.Event(), threading.Event()

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                started.set()
                release.wait(12)
                self.send_response(200)
                self.send_header("Content-Length", "8")
                self.end_headers()
                self.wfile.write(b"ENV_DONE")

            def log_message(self, *_):
                pass

        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        self.addCleanup(release.set)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        path = self.root / ".env.test"
        path.write_text(f"BASE_URL=http://127.0.0.1:{server.server_port}\n")
        self.requests[0]["url"] = "{{BASE_URL}}/environment"
        self.requests[0]["scripts"] = {"pre_request_script": None, "post_request_script": 'env.AFTER = "script-write";'}
        self.write_fixtures()
        app = self.start()
        self.open_first(app)
        app.press(" ")
        self.assertTrue(started.wait(3), "request should resolve the active environment's URL")
        app.press(b"\x05Dl\r")
        self.assertFalse(path.exists())
        app.press("atest\r")  # reuse the name before the old request completes
        self.assertEqual(path.read_text(), "")
        release.set()
        end = time.monotonic() + 5
        while time.monotonic() < end:
            app.read(0.15)
            if self.saved()[0]["response"]["content"] == "ENV_DONE":
                break
        self.assertEqual(self.saved()[0]["response"]["content"], "ENV_DONE")
        self.assertEqual(path.read_text(), "", "an in-flight script must not overwrite the replacement environment")

    def test_panel_selection_and_response_scroll_are_per_tab(self):
        app = self.start()
        self.open_first(app)
        app.press(b"\t\t")  # Headers
        app.press(b"\x0c")
        app.press(b"\x04")
        self.assertIn("ONE_ROW_010", app.screen())
        app.press(b"\x08\x08")  # Response -> Parameters -> Collections
        app.press(b"j\r")
        app.press("[")
        screen = app.screen()
        self.assertIn("ONE_ROW_010", screen)
        self.assertIn("NORMAL · Parameters", screen)
        # Selected header panel survives even after opening a different request.
        app.press("li")
        app.press(b"\x1b")
        app.press("0Cheader-value")
        app.press(b"\x13")
        self.assertEqual(self.saved()[0]["headers"][0]["data"][1], "header-value")
        self.assertEqual(self.saved()[1]["headers"][0]["data"][1], "original")

    def test_tabs_survive_reorder_rename_and_deletion(self):
        app = self.start()
        self.open_first(app)
        self.open_second(app)
        app.press(b"\x08K")  # move Two above One
        app.press("[")
        self.assertIn("ONE_ROW_000", app.screen())
        app.press(b"\x08r")  # tree cursor still points at Two
        app.press(b"\x1b")
        app.press("0CRenamed")
        app.press(b"\r")
        app.press("]")
        self.assertIn("Renamed", app.screen())
        self.assertIn("TWO_ROW_000", app.screen())
        app.press(b"\x08d")
        self.assertIn("Confirm delete request", app.terminal.text)
        before = self.saved()
        app.press(b"\r")  # Cancel is selected by default.
        self.assertEqual(self.saved(), before)
        app.press(b"dl\r")  # confirm deletion of tree selection
        app.press("[")
        screen = app.screen()
        self.assertIn("Requests 1/1", screen)
        self.assertIn("ONE_ROW_000", screen)
        self.assertEqual([r["name"] for r in self.saved()], ["One"])

    def test_background_responses_save_originating_collection_after_tab_close(self):
        release = threading.Event()
        started = threading.Event()

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                if self.path.startswith("/one"):
                    started.set()
                    release.wait(8)
                body = ("LOCAL_" + self.path.split("?")[0]).encode()
                self.send_response(200)
                self.send_header("Content-Type", "text/plain")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *_):
                pass

        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        self.addCleanup(release.set)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        for request in self.requests:
            request["url"] = f"http://127.0.0.1:{server.server_port}/{request['name'].lower()}"
        self.write_fixtures()
        app = self.start()
        self.open_first(app)
        app.press(" ")
        self.assertTrue(started.wait(3), "local request was not sent")
        app.press(b"\x08G")  # Second collection
        app.press(b"lj\r")  # Third request
        self.assertIn("Requests 2/2", app.screen())
        app.press(" ")
        app.press("[X")  # close the pending first tab; its request remains saved
        release.set()
        end = time.monotonic() + 5
        while time.monotonic() < end:
            app.read(0.15)
            if self.saved()[0]["response"]["content"] == "LOCAL_/one":
                break
        self.assertEqual(self.saved()[0]["response"]["content"], "LOCAL_/one")
        self.assertEqual(self.saved("Second")[0]["response"]["content"], "LOCAL_/three")
        self.assertIn("LOCAL_/three", app.screen())

    def test_multiline_body_and_websocket_draft_save_without_sending(self):
        self.requests[0]["protocol"]["body"] = {"raw": "original body"}
        self.requests[1]["protocol"] = {"type": "websocket", "message_type": {"type": "text", "content": ""}}
        self.write_fixtures()
        app = self.start()
        self.open_first(app)
        app.press(b"\t\t\t")
        app.press("i")
        app.press(b"\x1b")
        app.press("ccfirst line")
        app.press(b"\r")
        app.press("second line[]?")
        app.press(b"\x08")
        body = self.saved()[0]["protocol"]["body"]
        self.assertEqual(body["raw"], "first line\nsecond line[]?")
        self.assertEqual(self.saved()[0]["response"]["content"].splitlines()[0], "ONE_ROW_000")
        app.press(b"j\r\t\t\t")
        app.press("imessage[]?")
        app.press(b"\x13")
        request = self.saved()[1]
        self.assertEqual(request["protocol"]["message_type"]["content"], "message[]?")
        self.assertEqual(request["protocol"]["messages"], [])
        app.press("[]")
        self.assertIn("message[]?", app.screen())

    def test_add_creates_collection_and_request_from_collections(self):
        app = self.start()
        originals = {p: p.read_bytes() for p in self.root.glob("*.json")}
        self.assertIn("a new", app.terminal.text)
        app.press("n")
        self.assertNotIn("Choose element to create", app.terminal.text)
        app.press(b"a\rAdded\r")
        created = self.root / "Added.json"
        self.assertEqual(json.loads(created.read_text())["requests"], [])
        app.press(b"Gal\ran example\r")  # Select the new collection, then add a request.
        self.assertEqual([r["name"] for r in json.loads(created.read_text())["requests"]], ["an example"])
        for path, contents in originals.items():
            self.assertEqual(path.read_bytes(), contents)

        app.close()
        for path in self.root.glob("*.json"):
            path.unlink()
        app = self.start()
        app.press("a")  # With no collections, open the name field directly.
        self.assertIn("INSERT · New collection", app.terminal.text)
        app.press("First again\r")
        self.assertTrue((self.root / "First again.json").is_file())

    def test_row_shortcuts_and_vim_editing_stay_in_their_contexts(self):
        self.requests[0]["protocol"]["body"] = {"form": [{"enabled": True, "data": ["original", "keep"]}]}
        self.write_fixtures()
        app = self.start()
        self.open_first(app)
        app.press("a")
        request = self.saved()[0]
        self.assertEqual(len(request["params"]), 3)
        self.assertEqual(request["params"][:2], self.requests[0]["params"])
        self.assertIn("NORMAL · Parameters", app.terminal.text)
        app.press(b"G\ra!")  # Inside the new cell, `a` appends instead of adding a row.
        self.assertIn("INSERT · Edit query key", app.terminal.text)
        self.assertEqual(self.saved()[0]["params"][-1]["data"][0], "param")
        app.press(b"\x13")
        self.assertEqual(self.saved()[0]["params"][-1]["data"][0], "p!aram")
        self.assertEqual(len(self.saved()[0]["params"]), 3)
        app.press(b"\t\ta")
        self.assertEqual(len(self.saved()[0]["headers"]), 2)
        self.assertEqual(self.saved()[0]["headers"][0], self.requests[0]["headers"][0])
        self.assertIn("NORMAL · Parameters", app.terminal.text)
        app.press(b"\ta")
        self.assertEqual(len(self.saved()[0]["protocol"]["body"]["form"]), 2)
        self.assertEqual(self.saved()[0]["protocol"]["body"]["form"][0]["data"], ["original", "keep"])
        self.assertIn("NORMAL · Parameters", app.terminal.text)

        app.press("Gd")
        self.assertEqual(len(self.saved()[0]["protocol"]["body"]["form"]), 1)
        app.press(b"\x1b[Z")  # Shift+Tab returns to Headers.
        app.press("Gd")
        self.assertEqual(self.saved()[0]["headers"], self.requests[0]["headers"])
        app.press(b"\t\t\tG")  # Return to Params, on the added row.
        app.press("x")
        self.assertEqual(len(self.saved()[0]["params"]), 3)
        app.press("d")
        self.assertEqual(self.saved()[0]["params"], self.requests[0]["params"])

        app.press(b"ggl\r")  # Open the first value in Normal mode.
        app.press("d")
        self.assertIn("NORMAL · Edit query value", app.screen())
        self.assertEqual(self.saved()[0]["params"], self.requests[0]["params"])
        app.press("d")  # Finish dd: only the field's draft is deleted.
        app.press(b"\x13")
        self.assertEqual(self.saved()[0]["params"][0]["data"], ["first", ""])
        self.assertEqual(len(self.saved()[0]["params"]), 2)
        app.press("iOne")
        app.press(b"\x1b")
        app.press(b"0x\x13")
        self.assertEqual(self.saved()[0]["params"][0]["data"], ["first", "ne"])
        self.assertEqual(len(self.saved()[0]["params"]), 2)

    def test_delete_collection_uses_d_and_requires_confirmation(self):
        app = self.start()
        before = {p: p.read_bytes() for p in self.root.glob("*.json")}
        app.press("j")
        app.press("x")
        self.assertNotIn("Confirm delete collection", app.terminal.text)
        app.press("d")
        self.assertIn("Confirm delete collection", app.screen())
        app.press(b"\r")
        for path, contents in before.items():
            self.assertEqual(path.read_bytes(), contents)
        app.press(b"d\x1b")
        for path, contents in before.items():
            self.assertEqual(path.read_bytes(), contents)
        app.press(b"dl\r")
        self.assertFalse((self.root / "First.json").exists())
        self.assertEqual((self.root / "Second.json").read_bytes(), before[self.root / "Second.json"])

    def test_new_request_dialog_accepts_literal_navigation_keys(self):
        app = self.start()
        app.press("al")
        app.press(b"\r")
        self.assertIn("INSERT", app.screen())
        app.press("hjkl[]?")
        app.press(b"\t\t\t")
        app.press(b"\r")
        self.assertIn("hjkl[]?", [r["name"] for r in self.saved()])

    def test_curl_request_dialog_imports_multiline_request(self):
        app = self.start()
        app.press("all\r")  # Create element -> cURL Request.
        self.assertIn("cURL command", app.screen())
        app.press("Imported from cURL")
        app.press(b"\x1b")  # Switch the name editor to Normal mode first.
        app.press(b"\t")
        # The cURL editor is a real multiline editor: its terminal cursor
        # must move into the editor and align after edtui's dynamic gutter.
        self.assertEqual((app.terminal.x, app.terminal.y), (44, 19))
        self.assert_theme_text_and_cursor(app, insert=True)
        payload = b"curl --request GET 'http://example.test/items?page=2' --header 'X-Test: imported'"
        app.press(b"\x1b[200~" + payload + b"\x1b[201~")
        app.press(b"\x13")  # Ctrl+s confirms the multiline cURL input.

        request = self.saved()[-1]
        self.assertEqual(request["name"], "Imported from cURL")
        self.assertEqual(request["url"], "http://example.test/items")
        self.assertEqual(request["params"][0]["data"], ["page", "2"])
        self.assertIn(["x-test", "imported"], [header["data"] for header in request["headers"]])

    def test_curl_editor_cursor_tracks_insert_position(self):
        app = self.start()
        app.press("all\r")
        app.screen()
        app.press(b"\x1b")
        app.press(b"\t")
        self.assertEqual((app.terminal.x, app.terminal.y), (44, 19))
        app.press("curl")
        self.assertEqual((app.terminal.x, app.terminal.y), (48, 19))

    def test_insert_tab_stays_text_and_normal_tab_switches_new_request_fields(self):
        app = self.start()
        app.press("all\r")
        app.screen()
        app.press("Name")
        app.press(b"\t")
        app.press("Suffix")
        self.assertIn("INSERT · New request", app.terminal.text)
        self.assertIn("1:12", app.terminal.text)

        app.press(b"\x1b")
        app.press(b"\t")
        self.assertEqual((app.terminal.x, app.terminal.y), (44, 19))
        app.press(b"\x03")

    def test_last_table_row_remains_editable_after_scroll_and_tab_switch(self):
        self.requests[0]["params"] = [{"enabled": True, "data": [f"row{i}", "unchanged"]} for i in range(60)]
        self.write_fixtures()
        app = self.start()
        self.open_first(app)
        app.press("Gli")
        app.press(b"\x1b")
        app.press("ccLAST_VALUE")
        app.press(b"\x13")
        self.assertEqual(self.saved()[0]["params"][59]["data"][1], "LAST_VALUE")
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "unchanged")
        self.open_second(app)
        app.press("[i")
        app.press(b"\x1b")
        app.press("ccSECOND_EDIT")
        app.press(b"\x13")
        self.assertEqual(self.saved()[0]["params"][59]["data"][1], "SECOND_EDIT")
        app.press("gg")
        self.assertIn("row0", app.screen())

    def test_older_keybinding_file_without_workspace_still_loads(self):
        self.requests[0]["params"] = []
        self.write_fixtures()
        preset = (REPOSITORY / "example_resources/key_bindings/vim_key_bindings.toml").read_text()
        preset = re.sub(r"\[keybindings.workspace\].*?(?=\[keybindings.main_menu\])", "", preset, flags=re.S)
        preset = preset.replace('create_element = "a"', 'create_element = "n"')
        preset = preset.replace('delete_element = "d"', 'delete_element = "x"')
        path = self.root / "keys.toml"
        path.write_text(preset)
        app = self.start({"LAZYAPI_KEY_BINDINGS": str(path)})
        self.open_first(app)
        self.assertIn("Add one with n", app.terminal.text)
        app.press("n")
        self.assertEqual(len(self.saved()[0]["params"]), 1)
        app.press("a!")  # An explicit old create mapping retains the append entry alias.
        self.assertIn("INSERT · Edit query key", app.terminal.text)
        app.press(b"\x13")
        self.assertEqual(self.saved()[0]["params"][0]["data"][0], "p!aram")
        app.press("d")
        self.assertEqual(len(self.saved()[0]["params"]), 1)
        app.press("x")
        self.assertEqual(self.saved()[0]["params"], [])
        app.press(b"\x0c")
        self.assertIn("NORMAL · Response", app.screen())

    def test_search_collections_finds_collapsed_requests_and_cycles_matches(self):
        app = self.start()
        before = {p: p.read_bytes() for p in self.root.glob("*.json")}
        app.press("/ThReE")
        self.assertIn("Search Collections · 1 match", app.screen())
        self.assertIn("Second / Three", app.terminal.text)
        app.press(b"\r")
        self.assertIn("›   GET Three", app.screen())
        self.assertNotIn("Requests 1/1", app.terminal.text)
        app.press("/127.0.0.1")
        self.assertIn("3 matches", app.screen())
        app.press(b"\r")
        self.assertIn("›   GET One", app.screen())
        for key, name in (("n", "Two"), ("n", "Three"), ("n", "One"), ("N", "Three")):
            app.press(key)
            self.assertIn("›   GET " + name, app.screen())
        app.press(b"\r")
        self.assertIn("THREE_ROW_000", app.screen())
        for path, contents in before.items():
            self.assertEqual(path.read_bytes(), contents)

    def test_search_query_is_literal_unicode_paste_cancel_and_resize_are_safe(self):
        self.requests[2]["name"] = "配置 café []?"
        self.write_fixtures()
        app = self.start()
        before = {p: p.read_bytes() for p in self.root.iterdir() if p.suffix == ".json" or p.name.startswith(".env")}
        app.press("/daq?[]")
        self.assertIn("No matches", app.screen())
        app.press(b"\r")
        self.assertIn("Search Collections", app.screen())
        app.press(b"\x15")
        app.press("\x1b[200~配置\r\nCAFÉ\t[]?\x1b[201~")
        self.assertIn("1 match", app.screen())
        for width, height in ((58, 14), (28, 8), (4, 3), (160, 42)):
            app.screen(width=width, height=height)
        app.press(b"\x03")
        self.assertNotIn("Search Collections", app.screen())
        app.press("/配置")
        self.assertIn("1 match", app.screen())
        app.press(b"\x1b")
        self.assertNotIn("›   GET", app.screen())
        for path, contents in before.items():
            self.assertEqual(path.read_bytes(), contents)

    def test_search_environments_and_variables_keeps_active_environment(self):
        path = self.root / ".env.zeta"
        path.write_text("FIRST=keep\nTOKEN=target-value\n")
        original = (self.root / ".env.test").read_bytes()
        app = self.start()
        app.press(b"\x05\x08")
        app.press("/ZETA")
        self.assertIn("Search Environments · 1 match", app.screen())
        app.press(b"\r")
        self.assertIn("test [active]", app.screen())
        self.assertNotIn("zeta [active]", app.terminal.text)
        app.press(b"\x0c")
        app.press("/TARGET")
        self.assertIn("Search Environment variables · 1 match", app.screen())
        app.press(b"\r")
        app.press("iupdated-")
        app.press(b"\x13")
        app.screen()
        self.assertEqual(path.read_text(), "FIRST=keep\nTOKEN=updated-target-value\n")
        self.assertEqual((self.root / ".env.test").read_bytes(), original)
        self.assertIn("test [active]", app.terminal.text)

    def test_search_shortcuts_jumps_and_preserves_underlying_editor_draft(self):
        app = self.start()
        self.open_first(app)
        app.press("liDRAFT-")
        app.press(b"\x1bOP")  # F1 while Insert is active.
        self.assertIn("Keyboard shortcuts", app.screen())
        app.press("/delete selected")
        self.assertIn("Search Keyboard shortcuts · 1 match", app.screen())
        app.press(b"\r")
        self.assertIn("Delete selected item", app.screen())
        app.press("n")
        self.assertIn("Delete selected item", app.screen())
        app.press(b"\x1b")  # Clear the completed search first.
        self.assertIn("Keyboard shortcuts", app.screen())
        app.press(b"\x1b")
        self.assertIn("INSERT · Edit query value", app.screen())
        app.press("/literal")
        app.press(b"\x13")
        app.screen()
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "DRAFT-/literalOne")

    def test_search_table_selects_matching_value_and_deletes_correct_row(self):
        app = self.start()
        self.open_first(app)
        app.press("/KEEP")
        self.assertIn("Search Query parameters · 1 match", app.screen())
        app.press(b"\r")
        app.press("iupdated-")
        app.press(b"\x13")
        app.screen()
        self.assertEqual(self.saved()[0]["params"][1]["data"], ["second", "updated-keep"])
        self.assertEqual(self.saved()[0]["params"][0]["data"], ["first", "One"])
        app.press("d")
        app.screen()
        self.assertEqual(self.saved()[0]["params"], self.requests[0]["params"][:1])
        app.press("n")
        self.assertIn("No matches", app.screen())
        app.press(b"\t\t")
        app.press("/original")
        self.assertIn("Search Request headers · 1 match", app.screen())
        app.press(b"\r")
        app.press("iheader-")
        app.press(b"\x13")
        app.screen()
        self.assertEqual(self.saved()[0]["headers"][0]["data"], ["X-Test", "header-original"])

    def test_search_response_cycles_lines_and_stays_scoped_to_request_tab(self):
        app = self.start()
        self.open_first(app)
        app.press(b"\x0c")
        app.press("/ONE_ROW_06")
        self.assertIn("Search Response body · 10 matches", app.screen())
        app.press(b"\r")
        self.assertIn("ONE_ROW_060", app.screen())
        app.press("n")
        self.assertIn("ONE_ROW_061", app.screen())
        app.press("N")
        self.assertIn("ONE_ROW_060", app.screen())
        app.press(b"\x08\x08j\r\x0c")
        self.assertIn("TWO_ROW_000", app.screen())
        app.press("n")
        self.assertIn("TWO_ROW_000", app.screen())
        app.press("/TWO_ROW_070")
        self.assertIn("1 match", app.screen())
        app.press(b"\r")
        self.assertIn("TWO_ROW_070", app.screen())

    def test_search_wrapped_response_reveals_match_far_along_a_single_line(self):
        self.requests[0]["response"]["content"] = "prefix " * 100 + "UNIQUE_NEEDLE" + " suffix" * 20
        self.write_fixtures()
        (self.root / "lazyapi.toml").write_text("save_requests_response = true\nwrap_responses = true\n")
        app = self.start()
        self.open_first(app)
        app.press(b"\x0c")
        app.press("/unique_needle")
        self.assertIn("1 match", app.screen(width=76, height=24))
        app.press(b"\r")
        self.assertIn("UNIQUE_NEEDLE", app.screen())
        self.assertEqual(self.saved()[0]["response"]["content"], self.requests[0]["response"]["content"])

    def test_editor_search_shows_query_accepts_paste_and_preserves_vim_navigation(self):
        self.requests[0]["params"][0]["data"][1] = "a target b target"
        self.write_fixtures()
        app = self.start()
        self.open_first(app)
        app.press(b"l\r/")
        app.press("\x1b[200~target\x1b[201~")
        self.assertIn("SEARCH · Edit query value", app.screen())
        self.assertIn("/target", app.terminal.text.splitlines()[-1])
        app.press(b"\rnx")
        self.assertIn("NORMAL · Edit query value [+]", app.screen())
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "a target b target")
        app.press(b"\x13")
        app.screen()
        self.assertEqual(self.saved()[0]["params"][0]["data"][1], "a target b arget")

    def test_search_empty_form_response_metadata_export_and_logs(self):
        self.requests[0]["protocol"]["body"] = {"form": []}
        self.requests[0]["response"]["cookies"] = "SessionCookie=CookieNeedle"
        self.requests[0]["response"]["headers"] = [["X-Response-Needle", "found"]]
        self.write_fixtures()
        app = self.start()
        self.open_first(app)
        before = self.saved()
        app.press(b"\t\t\t/")
        self.assertIn("Search Form body · 0 matches", app.screen())
        app.press(b"\x1b")
        app.press(b"\x0c\t/CookieNeedle")
        self.assertIn("Search Response cookies · 1 match", app.screen())
        app.press(b"\r")
        self.assertIn("SessionCookie=CookieNeedle", app.screen())
        app.press(b"\t/X-Response-Needle")
        self.assertIn("Search Response headers · 1 match", app.screen())
        app.press(b"\r")
        self.assertIn("X-Response-Needle", app.screen())
        app.press(b"E\r/X-Test")
        self.assertIn("Search Request export · 1 match", app.screen(width=70, height=26))
        app.press(b"\r")
        self.assertIn("X-Test", app.screen())
        app.press(b"\x1b")  # Clear search, then close export.
        app.press(b"\x1b")
        app.press(b"\x08\x08L/search_not_present_123")
        self.assertIn("Search Logs · 0 matches", app.screen())
        app.press(b"\x1b")
        self.assertIn("Logs", app.screen())
        self.assertEqual(self.saved(), before)
