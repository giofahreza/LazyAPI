# Keyboard reference

LazyAPI starts in Normal mode and uses Vim editing by default. The footer shows
the mode, the focused pane or edited field, and useful shortcuts. For example,
`NORMAL · Parameters` navigates the table; `NORMAL · Edit query value` moves
through text inside one value. `[+]` means the field has unsaved changes.
On wide terminals the footer also shows the request-tab count, cursor position,
and pending Vim operator. A `▶` and a teal border identify the focused pane.

Opening a text-body, body-file, or WebSocket-message panel focuses its editor
in Normal mode with a visible block cursor. Press `i` to type; the cursor changes
from a pale block to a blue underlined cell and every character appears immediately.
Normal-mode
`Ctrl+b` still changes the HTTP body type. Form-body tables retain row/column
navigation; Enter opens the selected cell in Normal mode. Auth fields, script
fields, and table cells also show a cursor when their editor is opened.

The UI uses LazyDB's Deep Space palette: dark surfaces, teal focus accents,
pale editor text, and solid-white shortcut hints and help text. Request tabs
use compact filled labels, with an underline for the active section tab.
LazyAPI branding sits at the bottom-left beside the mode and shortcuts.
Below 100 columns, the focused pane fills the workspace; pane navigation works
as usual. The full-screen UI enables its text/cursor palette even when the
launching shell sets `NO_COLOR`; CLI output keeps its existing color behavior.

To change a value, select it with `h/j/k/l`, press `Enter` to inspect it in
Normal mode, then `i` to insert. Press `Esc` when finished typing and `:wq`
followed by Enter to save and return to the pane. For a quick edit, press `i`
directly on a value, type, and use `Ctrl+s` to save and finish.

Press `?` in Normal mode for the scrollable shortcut modal, or `F1` in any
editing mode. Closing help restores the editor's mode, cursor, and draft.

## Search

Press `/` in a list or text view to open its search dialog. Type any part of a
name, key, value, shortcut, or line. Matching is literal and case-insensitive;
space-separated words must all appear in the result. Search includes requests
inside collapsed collections and matches collection/request names, methods,
and URLs. Environment search uses the focused environment list or variable table.
In keyboard help, press `/` to search key labels and their descriptions.

Search also works in query parameters, request headers, form and multipart
bodies, cookies, response bodies/headers/cookies, WebSocket messages, script
console output, logs, and request exports. Response images have no searchable text.

| Keys | Action |
| --- | --- |
| `/` | Open search in the focused list or view. |
| Type / paste | Filter results; shortcut letters remain query text. |
| `Up/Down`, `Ctrl+p/n`, `Ctrl+k/j`, `Shift+Tab/Tab` | Select previous / next result, wrapping at the ends. |
| `PageUp/PageDown` | Move ten results. |
| `Left/Right`, `Home/End`, `Backspace/Delete` | Edit the search query. |
| `Ctrl+u` / `Ctrl+w` | Clear the query / delete its previous word. |
| `Enter` | Jump to the selected item or text line and close search. No matches leaves search open. |
| `Esc` / `Ctrl+c` in search | Cancel without changing the underlying selection, draft, or saved data. |
| `n` / `N` after a search | Next / previous matching item or line in the same context. |
| `Esc` after a search | Clear it; another Esc performs the usual back/close action. |

Jumping to a request selects it in Collections; press Enter again to open it.
Jumping to an environment highlights it without activating it. Help search
scrolls to the shortcut's description. Search never executes the matched action.
In the environment manager, `N` means previous match while search is active;
clear search with Esc to restore its create-environment shortcut.

Inside a text editor, `/`, Enter, and `n/N` retain Vim field search. The current
query appears in the footer, including for single-line values, and bracketed
paste can fill it. `/` in Insert mode remains literal text.

## Navigation and request tabs

| Keys | Action |
| --- | --- |
| `h j k l` | Move within the focused pane; arrow keys also work. |
| `Ctrl+h` / `Ctrl+l` | Move between Collections, Parameters, and Response. |
| `Ctrl+k` / `Ctrl+j` | Focus the URL above / Parameters below. |
| `Ctrl+w` then `h/j/k/l` | Focus a neighboring pane from Normal mode. |
| `Ctrl+w` then `w` or `Ctrl+w` | Focus the next pane: Collections, URL, Parameters, Response. |
| `Enter` in Collections | Expand a collection or open its request in a tab. |
| `h` / `l` in Collections | Collapse / expand the tree. |
| `[` / `]` | Previous / next open request, wrapping at either end. |
| `X` outside an editor | Close the active tab, keeping the saved request. |
| `Tab` / `Shift+Tab` | Next / previous panel in the focused Parameters or Response pane. |
| `gg` / `G` | First / last row in collections, parameter tables, environments, and responses. |
| `Ctrl+u` / `Ctrl+d` | Move up / down ten rows. |
| `Esc` in the request workspace | Focus Collections. |
| `q` / `Ctrl+c` in the workspace | Quit the app. |

Opening the same request focuses its existing tab. Tabs preserve panel selection,
row/column selection, view, and response scroll. Requests can complete in the
background, including after their tab is closed. Closing a tab never deletes a
request or cancels it. Open tabs are kept for the current app session.

The URL spans the top of the request workspace; Parameters and Response sit side
by side below it. At an outer pane edge, directional focus stays in that pane.
Focusing a hidden pane restores the split view.
`Ctrl+w` shows a pane hint until the next key; `Esc` cancels it. It is a
one-command prefix, as in Neovim. In Insert mode, `Ctrl+w` deletes a word.

## Editing

In collections and tables, `a` adds an item. To append within an existing cell,
press `Enter` to open its editor, then `a`. Vim search still uses `n` / `N` for
the next / previous match inside an editor.
In collections and tables, `d` deletes the selected item. Inside a text editor,
`d` starts a Vim delete command such as `dd` or `diw`, and `x` deletes a character.

| Keys | Action |
| --- | --- |
| `Enter` on a value | Open its editor in Normal mode, at the beginning of the value. |
| `i` on a value | Open its editor and insert before the cursor. |
| `I` / `A` on a value | Open its editor and insert at first text / append at line end. |
| `Esc` / `Ctrl+[` while editing | Return to Normal mode; keep the draft and editor open. Repeating Esc does nothing. |
| `Ctrl+s` | Save and finish editing in any editor mode. |
| `Enter` in a single-line field | Save and finish, except during a search or command. |
| `Enter` in a multiline field | Insert a newline in Insert mode. |
| `Ctrl+c` | Discard changes since the last save and leave the editor. |
| `i` / `a` / `I` / `A` in Normal | Insert before / after cursor / at first text / at end of line. |
| `o O` | Open a line below/above in multiline editor Normal mode. |
| `h j k l`, `w b e`, `0 ^ $`, `gg G` | Cursor, word, line, and buffer motions in Normal/Visual mode. |
| `v` / `V` | Select characters / the current line. Use `y` to yank, `d` to delete, or `c` to change a selection. |
| `yy` / `p` | Yank a line / paste. |
| `u` / `Ctrl+r` | Undo / redo, including after saving with `:w`. |
| `x` / `X` / `s` | Delete at cursor / before cursor / substitute a character. |
| `dd` / `D` / `d$` | Delete the line / from cursor through line end. |
| `C` / `c$`, `cc` / `S` | Change through line end / change the whole line. |
| `ciw` / `diw` / `yiw` | Change / delete / yank the inner word. |
| `ci"`, `ci'`, `ci(`, `ci{`, `ci[` | Change inside quotes or brackets; `d` and `y` support the same objects. |
| `/`, then `Enter`, then `n` / `N` | Search within the field, accept the search, and move to next/previous match. Search Enter never saves the field. |
| `Ctrl+w` / `Ctrl+u` in Insert | Delete the previous word / back to the first text on the line. |
| `?` / `F1` | Help in Normal mode / help in any editing mode. |

In editor Normal mode, `:` opens a command prompt in the footer. Type a command
and press Enter. Backspace edits it, `Ctrl+u` clears it, and `Esc` cancels the
prompt without changing the field.

| Command | Action |
| --- | --- |
| `:w` / `:write` | Save the field and keep editing, with cursor and undo history intact. |
| `:wq` / `:x` | Save the field and return to its pane. |
| `:q` / `:quit` | Close an unchanged editor; show a message if changes are unsaved. |
| `:q!` / `:quit!` | Discard changes since the last save and return to the pane. |
| `:noh` / `:nohlsearch` | Clear search highlighting. |
| `:help` / `:h` | Open the shortcut modal. |

These commands apply to the current field. They do not quit the application.
`Ctrl+c` retains LazyAPI's discard shortcut; use Esc to keep your draft.

Pane shortcuts work while editing and save the current field before changing
focus. `[` / `]` switch requests after returning to Normal mode and also save
the field. `Tab` / `Shift+Tab` save and switch panels from editor Normal mode.
In Insert mode, `[`, `]`, `?`, `:`, Tab, and application shortcut letters are
text. Vim operators own their complete sequence, so `ci[` edits brackets
without switching tabs. `X` deletes backward inside the editor.

Terminal bracketed paste inserts text without running shortcuts or saving the
field. Multiline editors preserve line breaks; single-line fields replace line
breaks with spaces.

Creation and rename dialogs start in Insert mode. Esc returns to Normal and
keeps the dialog open; Enter, Ctrl+s, or `:w` submits it. `Tab` / `Shift+Tab`
move between fields of the new-request dialog. `Ctrl+c` or `:q!` cancels it.
An invalid value keeps its editor and draft open.

## Collections, requests, and tables

| Keys | Action |
| --- | --- |
| `a` | Create a collection, request, or row in the focused table. |
| `r` | Rename the selected collection or request. |
| `d` | Delete the selected item; collections/requests require confirmation. |
| `Y` | Duplicate the selected item. |
| `J` / `K` | Move the selected collection/request down/up. |
| `t` | Enable/disable a parameter, header, or form row. |
| `u` | Edit URL. |
| `m` | Cycle HTTP method from the URL/Parameters pane. |
| `Space` | Send/cancel HTTP request; connect/disconnect WebSocket. |
| `Ctrl+Enter` | Alternate send; send a WebSocket message from its panel/editor. |
| `s` | Request settings. |
| `v` | Cycle split / Response / Parameters views. |
| `E` | Export request. |
| `Ctrl+a` | Cycle authentication type from Parameters. |
| `Ctrl+b` | Cycle HTTP body type from Parameters. |
| `Ctrl+t` | Cycle WebSocket message type from Parameters. |
| `y` in Response | Copy the focused response panel. |
| `e` / `Ctrl+e` | Next environment / environment editor. |
| `c` / `L` | Cookies / logs. |

Saving a WebSocket draft does not send it. Sending a message requires an explicit
send command. The application never sends HTTP requests merely because a field
is saved or a tab/pane changes.

In help, use `j/k` or arrows to scroll, `Ctrl+u/d` to page, `g/G` for the ends,
and `Esc`, `?`, or `q` to close it. Help also works in small/resized terminals.

## Environments

`Ctrl+e` opens the environment manager even when there are no environments.
The left pane lists environments and marks the one used by requests as `[active]`.
The right pane edits the highlighted environment's variables. Browsing and
editing an environment do not activate it; Enter or Space on its name does.
A newly created environment becomes active. Selection lasts for the app session;
on startup environments are listed by name and the first one is active.

| Keys | Action |
| --- | --- |
| `Ctrl+h` / `Ctrl+k` | Focus the environment list. |
| `Ctrl+l` / `Ctrl+j` | Focus variables. |
| `Tab` / `Shift+Tab` in Normal | Switch between the two panes. |
| `j/k`, `gg/G`, `Ctrl+u/d` | Navigate the focused list or table. |
| `Enter` / `Space` on an environment | Activate it; Enter also focuses its variables. |
| `a` / `r` / `d` in the list | Create / rename / delete an environment. |
| `N` / `R` / `D` in either pane | Create / rename / delete the highlighted environment. |
| `a` in variables | Add a blank variable with a unique `KEY_n` name and select it. |
| `h/l` in variables | Select the name / value column. |
| `Enter` / `i` on a cell | Open Normal mode / start inserting. `cc` replaces a cell in Normal. |
| `r` in variables | Open the variable name in Normal mode. |
| `d` in variables | Delete the selected variable immediately. |
| `Ctrl+s` / `:wq` | Save a cell or submit a name dialog. |
| `Esc` / `q` in the manager | Close the manager and return to the request workspace. |

Name dialogs start in Insert mode. `Esc` keeps the draft in Normal;
`Ctrl+c` cancels the dialog or discards a cell edit. Pane navigation saves a cell
before leaving; validation or write errors keep its draft open with an error.
`?` shows manager shortcuts and `F1` also works while editing.

Deleting an environment asks for confirmation, with **Cancel** selected by
default. Use `h/l` to choose and Enter to confirm; Esc cancels. After deleting
the last environment, the manager stays open so `a` can create a replacement.

Variables are saved in the existing `.env.<name>` format alongside collections.
Use `{{VARIABLE}}` in URLs, headers, authentication or bodies, and
`env.VARIABLE` in scripts.
Names must start with a letter or underscore and contain only letters, digits,
or underscores. Values retain quotes, whitespace and backslashes when saved.
Duplicate names and keys are rejected without overwriting existing data.

## Customization and terminal support

Set `LAZYAPI_KEY_BINDINGS` to a TOML file. The complete default is in
[`default_key_bindings.toml`](../example_resources/key_bindings/default_key_bindings.toml).
The new `[keybindings.workspace]` table configures pane navigation, request tabs,
previous panel, and paging. Older files without that table still load and inherit
its defaults; remove old conflicts such as help on `Ctrl+h`, logs on `l`, or
response scrolling on `Ctrl+h/j/k/l` when adopting this workspace.

The tables above list the supported Vim commands. Alternative Emacs and custom
editor presets remain available;
Enter continues to open those editors in Insert mode. The modal editing and
command prompt rows above describe the default Vim preset.

Terminals must deliver the key combination to LazyAPI. In raw mode, `Ctrl+j`
is distinct from Enter and `Ctrl+h` is distinct from the usual Backspace (DEL).
If a terminal sends Ctrl+h for Backspace or intercepts these chords, adjust its
bindings or override the pane keys in the TOML file. `Ctrl+Enter` support varies;
Space works for HTTP requests and WebSocket connection control.
