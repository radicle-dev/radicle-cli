# Radicle Client Tools

> ✨ Command-line client tooling for Radicle.

## Installation
### By building from source code

You can install the radicle tools by running the following command from inside
this repository:

    cargo install --force --locked --path .

Or directly from our seed node:

    cargo install --force --locked --git https://seed.alt-clients.radicle.xyz/radicle-client-tools.git radicle-tools

### From an APT repository on Debian/Ubuntu

```
curl https://europe-north1.pkg.dev/doc/repo-signing-key.gpg | sudo apt-key add -
echo 'deb https://us-central1-apt.pkg.dev/projects/radicle-services radicle-tools main' | sudo tee -a  /etc/apt/sources.list.d/artifact-registry.list
sudo apt update
sudo apt install radicle-tools
```

### From Homebrew on macOS

```
brew tap radicle/client-tools https://github.com/radicle-dev/homebrew-client-tools
brew install radicle/client-tools/core
```
