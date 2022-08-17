# Radicle CLI

> ✨ Command-line tooling for Radicle.

## Documentation

See the [getting started guide](https://radicle.xyz/get-started.html) for setup and usage instructions.

## Contributing

The Radicle CLI is still under development. Contributions are always welcome! Please check the [contributing page](https://app.radicle.network/seeds/seed.alt-clients.radicle.xyz/rad:git:hnrkmg77m8tfzj4gi4pa4mbhgysfgzwntjpao/tree/cc80d84ea5be6466647777224c1131b2e0ad11c8/CONTRIBUTING.md) if you want to help.

## Installation

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

### 🐧 From APT (Debian/Ubuntu)

First, download the package signing key:

    curl -fsSL https://europe-west6-apt.pkg.dev/doc/repo-signing-key.gpg | gpg --dearmor | sudo tee /usr/share/keyrings/radicle-archive-keyring.gpg > /dev/null

Then update your sources list with the radicle repository by creating a registry file:

    echo "deb [arch=amd64 signed-by=/usr/share/keyrings/radicle-archive-keyring.gpg] https://europe-west6-apt.pkg.dev/projects/radicle-services radicle-cli main" | sudo tee -a /etc/apt/sources.list.d/radicle-registry.list

Then update the package list and install `radicle-cli`:

    sudo apt update
    sudo apt install radicle-cli

### 🍺 From Homebrew (x86_64)

    brew tap --force-auto-update radicle/cli https://seed.alt-clients.radicle.xyz/radicle-cli-homebrew.git
    brew install radicle-cli
