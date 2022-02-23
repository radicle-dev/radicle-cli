# Radicle CLI

> ✨ Command-line client tooling for Radicle.

## Installation

### From source

You can install the Radicle CLI by running the following command from inside
this repository:

    cargo install --force --locked --path .

Or directly from our seed node:

    cargo install --force --locked --git https://seed.alt-clients.radicle.xyz/radicle-cli.git radicle-cli

### From an APT repository on Debian/Ubuntu

    curl https://europe-north1.pkg.dev/doc/repo-signing-key.gpg | sudo apt-key add -
    echo 'deb https://us-central1-apt.pkg.dev/projects/radicle-services radicle-cli main' | sudo tee -a  /etc/apt/sources.list.d/artifact-registry.list
    sudo apt update
    sudo apt install radicle-cli


### From Homebrew

    brew tap radicle/cli https://seed.alt-clients.radicle.xyz/radicle-cli-homebrew.git
    brew install radicle/cli/core
