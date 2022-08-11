# Radicle CLI

> ✨ Command-line tooling for Radicle.

## Installation

### 📦 From source

You can install the Radicle CLI and supporting tools by running the
following command from inside this repository:

    cargo install --path cli --force --locked

Or directly from our seed node:

    cargo install radicle-cli --force --locked --git https://seed.alt-clients.radicle.xyz/radicle-cli.git

### 🐧 From APT (Debian/Ubuntu)

First, download the package signing key:

    curl https://europe-west6-apt.pkg.dev/doc/repo-signing-key.gpg | sudo apt-key add -

Then update your sources list with the radicle repository by creating a registry file:

    echo deb https://europe-west6-apt.pkg.dev/projects/radicle-services radicle-cli main | sudo tee -a /etc/apt/sources.list.d/radicle-registry.list

Then update the package list and install `radicle-cli`:

    sudo apt update
    sudo apt install radicle-cli

### 🍺 From Homebrew (x86_64)

    brew tap radicle/cli https://seed.alt-clients.radicle.xyz/radicle-cli-homebrew.git
    brew install radicle-cli
