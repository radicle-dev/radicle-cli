# Radicle CLI

> ✨ Command-line tooling for Radicle.

## Installation

### From source

You can install the Radicle CLI by running the following command from inside
this repository:

    cargo install --force --locked --path .

Or directly from our seed node:

    cargo install --force --locked --git https://seed.alt-clients.radicle.xyz/radicle-cli.git radicle-cli

### From an APT repository on Debian/Ubuntu

First, download the package signing key:

    curl https://europe-west6.pkg.dev/doc/repo-signing-key.gpg | sudo apt-key add -

Then update your sources list by creating a registry file for the Radicle APT repository:

    # /etc/apt/sources.list.d/radicle-registry.list
    deb https://europe-west6-apt.pkg.dev/projects/radicle-services radicle-cli main

Then update the package list and install `radicle-cli`:

    sudo apt update
    sudo apt install radicle-cli

### From Homebrew (x86_64)

    brew tap radicle/cli https://seed.alt-clients.radicle.xyz/radicle-cli-homebrew.git
    brew install radicle/cli/core
