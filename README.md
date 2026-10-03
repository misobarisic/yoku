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

## Keyboard shortcuts

- Move between files and lists with the arrow keys, WASD, or HJKL. Move through tasks with Up/Down.
- Press `Enter`, `Space`, `x`, `+`, or `-` to change the selected task state.
- Press `e` to edit the selected file name, list title, or task. Press `Ctrl+E` to edit a list description.
- Press `u` to create a file, `i` to create a list, and `o` to create a task.
- Press `r` to delete the selected task or list. Deleting a file asks for confirmation. `Ctrl+Z` undoes edits (including deletions, creations, renames, and task states); `Ctrl+Y` redoes them. The last 100 edits remain undoable after saving. Reloading external changes clears the history.
- Press `/` to search file names, list titles, descriptions, and task text. Search ignores case; `n` and `N` move to the next and previous matches.
- Press `?` or `F1` for the in-app help.
- Press `Ctrl+S` to save and keep working; changed files have a `*` beside their name. Press `q` to save and quit. If saving fails, the app stays open with your edits available for retry. `Ctrl+Q` or `Ctrl+C` asks before discarding unsaved changes.
- In an editor, use Left/Right, Home/End, Backspace, and Delete to move and edit text. Cursor movement treats accented letters and emoji as whole graphemes.

---

## License
This project is licensed under [GPLv3](https://choosealicense.com/licenses/gpl-3.0/).
