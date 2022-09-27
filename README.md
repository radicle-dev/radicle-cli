# Radicle CLI

> ✨ Command-line tooling for Radicle.

## Documentation

See the [getting started guide](https://radicle.xyz/get-started.html) for setup and usage instructions.

## Contributing

The Radicle CLI is still under development. Contributions are always welcome! Please check the [contributing page](https://app.radicle.network/seeds/seed.alt-clients.radicle.xyz/rad:git:hnrkmg77m8tfzj4gi4pa4mbhgysfgzwntjpao/tree/cc80d84ea5be6466647777224c1131b2e0ad11c8/CONTRIBUTING.md) if you want to help.

## Installation

### 💡 Recommended

The [getting started guide](https://radicle.xyz/get-started.html) provides instructions for installing binaries. These are the recommended installation methods for most users.

### 📦 From source

You can install the Radicle CLI and supporting tools by running the
following command from inside this repository:

    cargo install --path cli --force --locked

Or directly from our seed node:

    cargo install radicle-cli --force --locked --git https://seed.alt-clients.radicle.xyz/radicle-cli.git

> **Note**
> It's recommended to install release builds since development builds may be incompatible with the network.

### 🐳 From Dockerfile

Alternatively you can build the Radicle CLI docker `rad` by running the
following command from inside this repository:

    docker build . -t rad

And then to use it:

    docker run -it --rm rad --help

> **Note**
> It's recommended to install release builds since development builds may be incompatible with the network.

