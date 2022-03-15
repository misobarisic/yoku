## yoku

[![Continuous Integration](https://github.com/misobarisic/yoku/actions/workflows/ci.yml/badge.svg)](https://github.com/misobarisic/yoku/actions/workflows/ci.yml)
[![Continuous Deployment](https://github.com/misobarisic/yoku/actions/workflows/cd.yml/badge.svg)](https://github.com/misobarisic/yoku/actions/workflows/cd.yml)

yoku is a markdown based todo app allowing for easy portability

---

## Installation

### Latest release

Binary releases are available [here](https://github.com/misobarisic/yoku/releases).

### Build from source (latest)

Requires `rust` and `cargo` to be installed:

1. Clone the repository with `git clone https://github.com/misobarisic/yoku.git` and cd into it
2. Run `cargo build --release`
3. Move the binary to your place of choice `mv target/release/yoku $destination`

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

Default data location depends on the platform you're using. You can check it by passing the `-d/--data-path` flag such as `yoku -d`.

---

## License
This project is licensed under [GPLv3](https://choosealicense.com/licenses/gpl-3.0/).
