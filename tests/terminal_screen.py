"""Small VT screen reader for the cursor moves and colors emitted by ratatui.

Keeps the actual screen between draws so tests can check live typing without
forcing a resize/repaint, and inspect the foreground/background of each glyph.
"""
import re
import unicodedata


class TerminalScreen:
    CSI = re.compile(r"\x1b\[([0-?]*)([ -/]*)([@-~])")

    def __init__(self, width, height):
        self.width, self.height = width, height
        self.x = self.y = 0
        self.fg = self.bg = None
        self.bold = False
        self.italic = False
        self.pending = ""
        self.saved_cursor = (0, 0)
        self.cells = [[(" ", None, None, False, False) for _ in range(width)] for _ in range(height)]

    def resize(self, width, height):
        self.cells = [
            [self.cells[y][x] if y < self.height and x < self.width else (" ", None, None, False, False)
             for x in range(width)] for y in range(height)
        ]
        self.width, self.height = width, height

    @property
    def text(self):
        return "\n".join("".join(cell[0] for cell in row) for row in self.cells)

    def feed(self, text):
        text = self.pending + text
        self.pending = ""
        i = 0
        while i < len(text):
            ch = text[i]
            if ch == "\x1b":
                if i + 1 == len(text):
                    break
                if text[i + 1] == "[":
                    match = self.CSI.match(text, i)
                    if not match:
                        break
                    self.csi(match[1], match[3])
                    i = match.end()
                    continue
                if text[i + 1] in "]P_":
                    end = re.search(r"\x07|\x1b\\", text[i + 2:])
                    if not end:
                        break
                    i += 2 + end.end()
                    continue
                if text[i + 1] in "()":
                    if i + 2 == len(text):
                        break
                    i += 3
                    continue
                if text[i + 1] == "7":
                    self.saved_cursor = self.x, self.y
                elif text[i + 1] == "8":
                    self.x, self.y = self.saved_cursor
                i += 2
                continue
            if ch == "\r":
                self.x = 0
            elif ch == "\n":
                self.y = min(self.height - 1, self.y + 1)
            elif ch == "\b":
                self.x = max(0, self.x - 1)
            elif ch >= " " and ch != "\x7f":
                if self.x >= self.width:
                    self.x, self.y = 0, min(self.height - 1, self.y + 1)
                if unicodedata.combining(ch):
                    if self.x > 0:
                        old = self.cells[self.y][self.x - 1]
                        self.cells[self.y][self.x - 1] = (old[0] + ch, old[1], old[2], old[3], old[4])
                else:
                    self.cells[self.y][self.x] = (ch, self.fg, self.bg, self.bold, self.italic)
                    self.x += 2 if unicodedata.east_asian_width(ch) in "WF" else 1
            i += 1
        self.pending = text[i:]

    def csi(self, parameters, command):
        if parameters.startswith("?"):
            if parameters == "?1049" and command == "h":
                self.x = self.y = 0
                self.erase(0, self.width * self.height)
            return
        args = [int(value or 0) for value in parameters.split(";")]
        count = args[0] or 1
        if command in "Hf":
            self.y = min(self.height - 1, max(0, count - 1))
            self.x = min(self.width - 1, max(0, (args[1] if len(args) > 1 else 1) - 1))
        elif command == "G":
            self.x = min(self.width - 1, count - 1)
        elif command in "ABCD":
            if command == "A": self.y = max(0, self.y - count)
            if command == "B": self.y = min(self.height - 1, self.y + count)
            if command == "C": self.x = min(self.width - 1, self.x + count)
            if command == "D": self.x = max(0, self.x - count)
        elif command == "J":
            position = self.y * self.width + self.x
            self.erase(0 if args[0] in (1, 2, 3) else position,
                       position + 1 if args[0] == 1 else self.width * self.height)
        elif command == "K":
            self.erase(self.y * self.width + (self.x if args[0] == 0 else 0),
                       self.y * self.width + (self.x + 1 if args[0] == 1 else self.width))
        elif command == "m":
            self.sgr(args)

    def erase(self, start, end):
        for offset in range(start, end):
            self.cells[offset // self.width][offset % self.width] = (" ", self.fg, self.bg, self.bold, self.italic)

    def sgr(self, args):
        i = 0
        while i < len(args):
            code = args[i]
            if code == 0: self.fg = self.bg = None; self.bold = self.italic = False
            elif code == 1: self.bold = True
            elif code == 22: self.bold = False
            elif code == 3: self.italic = True
            elif code == 23: self.italic = False
            elif code == 39: self.fg = None
            elif code == 49: self.bg = None
            elif 30 <= code <= 37: self.fg = code - 30
            elif 90 <= code <= 97: self.fg = code - 90 + 8
            elif 40 <= code <= 47: self.bg = code - 40
            elif 100 <= code <= 107: self.bg = code - 100 + 8
            elif code in (38, 48) and i + 2 < len(args):
                if args[i + 1] == 5:
                    color = args[i + 2]
                    i += 2
                elif args[i + 1] == 2 and i + 4 < len(args):
                    color = tuple(args[i + 2:i + 5])
                    i += 4
                else:
                    i += 1
                    continue
                if code == 38: self.fg = color
                else: self.bg = color
            i += 1
