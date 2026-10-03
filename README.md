## yoku

[![Continuous Integration](https://github.com/misobarisic/yoku/actions/workflows/ci.yml/badge.svg)](https://github.com/misobarisic/yoku/actions/workflows/ci.yml)
[![Continuous Deployment](https://github.com/misobarisic/yoku/actions/workflows/cd.yml/badge.svg)](https://github.com/misobarisic/yoku/actions/workflows/cd.yml)
[![License](https://img.shields.io/github/license/misobarisic/yoku?color=blue)](./COPYING.md)
[![GitHub release (latest by date)](https://img.shields.io/github/v/release/misobarisic/yoku)](https://github.com/misobarisic/yoku/releases/latest)
![GitHub code size in bytes](https://img.shields.io/github/languages/code-size/misobarisic/yoku)

yoku is a Markdown based todo app allowing for easy portability.

---

## Installation

### Latest release

Binary releases are available [here](https://github.com/misobarisic/yoku/releases).

### Build from source (latest)

Requires Rust 1.88 or newer and Cargo:

1. Clone the repository with `git clone https://github.com/misobarisic/yoku.git` and cd into it
2. Run `cargo build --locked --release`
3. Move the binary to your place of choice `mv target/release/yoku $destination`

You can also install it directly from the checkout with `cargo install --path . --locked`.

### Arch Linux

3 different packages are available in the Arch Linux User Repository:
- `yoku-bin` (latest binary release)
- `yoku` (latest release, built locally)
- `yoku-git` (latest commit, built locally)

---

## Contributing
Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

Please make sure to update tests as appropriate.

---

## Data

The default data directory depends on your platform. Print it with `yoku --data-path`, or choose a different directory with `yoku --main-path ./todos`. Yoku reads Markdown (`.md`) files from that directory and preserves Markdown it does not edit.

Lists can use headings from `#` through `######`. Checklists without a heading appear as an implicit Inbox. Tasks support `-`, `*`, `+`, and numbered bullets, uppercase or lowercase checked markers, and indented subtasks. Checkbox edits preserve the original bullet, indentation, and spacing. Code examples are excluded from tasks. Deleting a parent task includes its subtasks and continuation text; deleting a heading includes its subheadings. Both are undoable.

Capture and inspect tasks from the shell without opening the TUI:

```sh
yoku add "Buy milk"
yoku add "Review changes" --file work --list Inbox
yoku list --open
yoku list --state done --file work --json
yoku --main-path ./todos add "Local project task"
```

Capture defaults to `inbox.md` and an Inbox heading, creating them as needed. Listing supports `--file` and `--list` and does not create files. JSON includes each task's file, list, index within the list, state, text, and indentation depth. New files created in the TUI start with an empty Inbox.

Metadata stays in task text, for example `- [ ] Review changes #work priority:high due:2026-10-03`. Tags can contain letters, numbers, `-`, `_`, and `/`; priorities are `high`, `normal` (the default), or `low`. Dates use `YYYY-MM-DD`. Invalid tokens remain ordinary text, and tokens inside code spans are excluded from metadata.

```sh
yoku add "Review changes" --tag work --priority high --due 2026-10-03
yoku list --open --tag work --due overdue --sort due --json
```

Capture accepts `--tag` multiple times, `--priority`, and `--due`. Listing supports the same filters; its due filter accepts `today`, `overdue`, `none`, or a date. Sorting uses `document`, `priority`, or `due`. JSON also includes tags, priority, due date, and recurrence when present. Sorting and filtering change the view without rewriting Markdown.

## Keyboard shortcuts

- Move between files and lists with the arrow keys, WASD, or HJKL. Move through tasks with Up/Down.
- Press `Enter`, `Space`, `x`, `+`, or `-` to change the selected task state.
- Press `e` to edit the selected file name, list title, or task. Press `Ctrl+E` to edit a list description.
- Press `J`/`K` to move a task down/up among its siblings, `m` to pick another list or file, and `Tab`/`Shift+Tab` to indent/outdent it. Subtasks and continuation Markdown move with their parent.
- Press `u` to create a file, `i` to create a list, and `o` to create a task.
- Press `r` to delete the selected task or list. Deleting a file asks for confirmation. `Ctrl+Z` undoes edits (including deletions, creations, renames, and task states); `Ctrl+Y` redoes them. The last 100 edits remain undoable after saving. Reloading external changes clears the history.
- Press `/` to search file names, list titles, descriptions, and task text. Search ignores case; `n` and `N` move to the next and previous matches.
- Press `f` to cycle All/Open/Done/Rejected tasks and `g` to toggle a task view across every file. Tabs show done/total progress, and the task panel shows state counts. Search returns to the matching list with all states visible.
- Press `F` to combine filters such as `tag:work priority:high due:today` and ordinary search words; submit an empty filter to clear it. Press `S` to cycle document/priority/due sorting, `t` for open tasks due Today, and `v` for open Overdue tasks across all files. Relative dates use your local calendar. Overdue tasks appear red and high-priority tasks yellow. `J`/`K` switch back to document order when reordering tasks.
- Press `?` or `F1` for the in-app help.
- Press `Ctrl+S` to save and keep working; changed files have a `*` beside their name. Press `q` to save and quit. If saving fails, the app stays open with your edits available for retry. `Ctrl+Q` or `Ctrl+C` asks before discarding unsaved changes.
- In an editor, use Left/Right, Home/End, Backspace, and Delete to move and edit text. Cursor movement treats accented letters and emoji as whole graphemes.

---

## License
This project is licensed under [GPLv3](https://choosealicense.com/licenses/gpl-3.0/).
