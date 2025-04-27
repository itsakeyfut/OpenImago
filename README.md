# OpenImago

![Status](https://img.shields.io/badge/status-in--development-yellow?style=flat-square)
[![Rust Version](https://img.shields.io/badge/rust-1.86+-blue.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Overview

OpenImago is an open-source tool for downloading media content (videos and audio) from various platforms. It aims to provide an easy-to-use interface for users to download media for offline use, supporting a wide range of formats and platforms. OpenImago is built using Rust, ensuring a lightweight, fast, and secure experience.

## Features

- **Platform Support**: Download videos and audio from multiple platforms.
- **Flexible Formats**: Choose from a variety of formats (currently available formats: MP3, MP4).
- **Offline Enjoyment**: Fully offline-compatible, making it perfect for enjoying your favorite media on the go.
- **Command-line Interface**: Simple CLI for easy interaction.

## Installation

To install OpenImago, simply build it from source using the following steps:

```sh
git clone https://github.com/itsakeyfut/OpenImago.git
cd OpenImago
cargo build --release
```

## Usage

### Example

Supported formats: MP3 and MP4.

At this time, PowerShell supported only.
```shell
./target/release/openimago -u <url> -f <format>
```

## Future Development

- Support additional formats such as WebM, WAV, OGG, and more.
- Support GUI for a more user-friendly interface.
- Add Offline Music and Video Player functionality.

## Contributing

Contributions are welcome! If you'd like to help improve OpenImago, feel free to fork the repository and submit pull requests. Here's how you can contribute:

Steps for Contributors:

- Fork the repo
- Create a new branch for your changes
- Make your changes and commit them
- Push your changes to your forked repo
- Open a pull request for review

## License

This project is licensed under the MIT License. See the [LICENSE](./LICENSE-MIT) file for more details.

## Author

OpenImago is maintained by itsakeyfut. Feel free to open issues or contribute pull requests if you encounter problems or want to suggest new features.
