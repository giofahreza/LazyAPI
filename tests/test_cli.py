"""Exercise the built CLI with isolated data; no network requests are sent."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


REPOSITORY = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get("LAZYAPI_BIN", REPOSITORY / "target/debug/lazyapi")).resolve()


@unittest.skipUnless(sys.platform.startswith("linux"), "XDG directory checks use Linux paths")
class CommandLineTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="lazyapi-cli-test-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.config = self.root / "config"
        self.data = self.config / "lazyapi"
        self.data.mkdir(parents=True)
        self.env = {**os.environ, "XDG_CONFIG_HOME": str(self.config)}
        for name in ("LAZYAPI_MAIN_DIR", "LAZYAPI_THEME", "LAZYAPI_KEY_BINDINGS"):
            self.env.pop(name, None)

    def cli(self, *arguments, success=True, **environment):
        result = subprocess.run(
            [str(BINARY), "--no-ansi-log", *map(str, arguments)],
            cwd=self.root,
            env={**self.env, **environment},
            capture_output=True,
            text=True,
            timeout=20,
        )
        if success:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        return result

    def collection(self, directory, name):
        directory.mkdir(parents=True, exist_ok=True)
        path = directory / f"{name}.json"
        path.write_text(json.dumps({"name": name, "last_position": None, "requests": []}) + "\n")
        return path

    def test_default_directory_and_dry_run_preserve_collection(self):
        path = self.collection(self.data, "Saved")
        before = path.read_bytes()
        result = self.cli("--dry-run", "collection", "list")
        self.assertEqual(result.stdout.strip(), "Saved")
        self.assertEqual(path.read_bytes(), before)

    def test_explicit_directory_overrides_environment_and_default(self):
        self.collection(self.data, "Default")
        alternate = self.root / "alternate"
        explicit = self.root / "explicit"
        self.collection(alternate, "Environment")
        self.collection(explicit, "Explicit")
        result = self.cli("--dry-run", "collection", "list", LAZYAPI_MAIN_DIR=str(alternate))
        self.assertEqual(result.stdout.strip(), "Environment")
        result = self.cli(
            "--dry-run", "--directory", explicit, "collection", "list",
            LAZYAPI_MAIN_DIR=str(alternate),
        )
        self.assertEqual(result.stdout.strip(), "Explicit")

    def test_local_settings_file_is_read(self):
        self.collection(self.data, "Saved")
        settings = self.data / "lazyapi.toml"
        settings.write_text('save_requests_response = "invalid boolean"\n')
        failure = self.cli("--dry-run", "collection", "list", success=False)
        self.assertIn("Could not parse config file", failure.stdout)
        settings.write_text("save_requests_response = true\n")
        self.assertEqual(self.cli("--dry-run", "collection", "list").stdout.strip(), "Saved")

    def test_new_request_identity_and_saved_header_preservation(self):
        self.cli("collection", "new", "Example")
        self.cli("request", "new", "Example/Get", "--url", "https://example.com")
        path = self.data / "Example.json"
        collection = json.loads(path.read_text())
        headers = collection["requests"][0]["headers"]
        user_agent = next(header for header in headers if header["data"][0] == "user-agent")
        version = self.cli("--version").stdout.strip().split()[1]
        self.assertEqual(user_agent["data"][1], f"LazyAPI/v{version}")
        user_agent["data"][1] = "saved-custom-client/1.0"
        path.write_text(json.dumps(collection, indent=2) + "\n")
        environment = self.data / ".env.personal"
        environment.write_text('BASE_URL="https://example.com"\n')
        before = {file.name: file.read_bytes() for file in (path, environment)}
        self.cli("--dry-run", "collection", "list", "--request-names")
        for file in (path, environment):
            self.assertEqual(file.read_bytes(), before[file.name])
        self.cli("collection", "list")
        saved = json.loads(path.read_text())["requests"][0]["headers"]
        self.assertIn(["user-agent", "saved-custom-client/1.0"], [header["data"] for header in saved])
        self.assertEqual(environment.read_bytes(), before[environment.name])

    def test_manual_and_all_shell_completions_are_generated(self):
        generated = self.root / "generated"
        generated.mkdir()
        for shell, filename in (("bash", "lazyapi.bash"), ("zsh", "_lazyapi"),
                                ("fish", "lazyapi.fish"), ("powershell", "_lazyapi.ps1"),
                                ("elvish", "lazyapi.elv")):
            with self.subTest(shell=shell):
                self.cli("completions", shell, generated)
                self.assertIn("lazyapi", (generated / filename).read_text())
        self.cli("man", generated)
        manual = (generated / "lazyapi.1").read_text()
        self.assertIn("lazyapi", manual)
        self.assertIn("LazyAPI", manual)

    def test_environment_crud_preserves_values_and_unrelated_files(self):
        other = self.data / ".env.existing"
        other.write_text('# keep this comment\nTOKEN="existing"\n')
        original = other.read_bytes()
        self.cli("env", "new", "Local Development")
        self.cli("env", "key", "Local Development", "add", "BASE_URL", "https://example.test")
        self.cli("env", "key", "Local Development", "set", "BASE_URL", "https://new.test")
        self.cli("env", "key", "Local Development", "rename", "BASE_URL", "API_URL")
        self.assertEqual(self.cli("env", "key", "Local Development", "get", "API_URL").stdout.strip(), "https://new.test")
        source = self.data / ".env.Local Development"
        source.write_text('# preserved on rename\n' + source.read_text())
        saved = source.read_bytes()
        self.cli("env", "rename", "Local Development", "Staging.env.v2")
        renamed = self.data / ".env.Staging.env.v2"
        self.assertFalse(source.exists())
        self.assertEqual(renamed.read_bytes(), saved)
        self.assertIn("Staging.env.v2", self.cli("env", "list").stdout.splitlines())
        self.cli("env", "key", "Staging.env.v2", "delete", "API_URL")
        self.assertEqual(renamed.read_text(), "")
        self.cli("env", "delete", "Staging.env.v2")
        self.assertFalse(renamed.exists())
        self.assertEqual(other.read_bytes(), original)

    def test_environment_values_round_trip_quotes_spaces_backslashes_and_newlines(self):
        self.cli("env", "new", "roundtrip")
        values = ["", "  padded value  ", 'say "hello"', "it's quoted", r"C:\Users\name\file", "first\nsecond\tvalue", "a=b&token=$TOKEN#frag", "日本語 👋"]
        for i, value in enumerate(values):
            with self.subTest(value=value):
                self.cli("env", "key", "roundtrip", "add", f"KEY_{i}", value)
                self.assertEqual(self.cli("env", "key", "roundtrip", "get", f"KEY_{i}").stdout, value + "\n")
        keys = [line.split("=", 1)[0] for line in (self.data / ".env.roundtrip").read_text().splitlines()]
        self.assertEqual(keys, [f"KEY_{i}" for i in range(len(values))])

    def test_environment_validation_and_collisions_never_overwrite_data(self):
        self.cli("env", "new", "one")
        self.cli("env", "key", "one", "add", "KEEP", "original")
        self.cli("env", "new", "two")
        files = {p: p.read_bytes() for p in self.data.glob('.env.*')}
        for args in [
            ("new", "one"), ("new", ""), ("new", "../outside"), ("new", "bad/name"),
            ("new", "bad\\name"), ("new", " spaced "), ("rename", "one", "two"),
            ("key", "one", "add", "KEEP", "replacement"),
            ("key", "one", "add", "BAD=KEY", "replacement"),
            ("key", "one", "rename", "KEEP", "BAD\nKEY"),
        ]:
            with self.subTest(args=args):
                self.cli("env", *args, success=False)
                self.assertEqual({p: p.read_bytes() for p in self.data.glob('.env.*')}, files)
        self.cli("env", "key", "one", "rename", "KEEP", "KEEP")
        self.assertEqual(self.cli("env", "key", "one", "get", "KEEP").stdout, "original\n")

    def test_environment_dry_run_does_not_create_rename_edit_or_delete_files(self):
        self.cli("env", "new", "saved")
        self.cli("env", "key", "saved", "add", "KEEP", "original")
        before = {p: p.read_bytes() for p in self.data.glob('.env.*')}
        for args in [("new", "draft"), ("rename", "saved", "renamed"),
                     ("key", "saved", "set", "KEEP", "draft"), ("delete", "saved")]:
            self.cli("--dry-run", "env", *args)
            self.assertEqual({p: p.read_bytes() for p in self.data.glob('.env.*')}, before)

    def test_theme_and_key_binding_environment_variables_are_read(self):
        for variable, message in (("LAZYAPI_THEME", "Could not open theme file"),
                                  ("LAZYAPI_KEY_BINDINGS", "Could not open key bindings file")):
            with self.subTest(variable=variable):
                failure = self.cli(success=False, **{variable: str(self.root / "missing.toml")})
                self.assertIn(message, failure.stdout)
                self.assertTrue((self.data / "lazyapi.log").is_file())


if __name__ == "__main__":
    unittest.main()
