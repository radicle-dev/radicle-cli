# Radicle CLI (Prototype)

This is a CLI prototype build with Python and [click](https://github.com/pallets/click). It assumes that you have the build folders, containing radicle (development) binaries added to your PATH. It relies on the radicle plumbing binaries returning proper JSON output (still WIP).

# Usage

## Setup

Install dependencies:
```
pip install -r requirements.txt
```

Make radicle (development) binaries accessible and create `rad` alias for this prototype:
```
$ export RAD_DEV=<absolute_path_to_radicle-dev>
$ source init.sh
```

Build radicle development binaries (with JSON output patched in):
```
$ cd radicle-link
$ git checkout feature/bins-json-output
$ cargo build
```

## User-flow

### Create new radicle profile and identity
```
$ rad auth
```
Available options:
```
$ rad auth --verbose
$ rad auth --add
```

### Initialize project
```
$ rad init
```
Note: Only possible in an exisiting Git repository.

### Run user node
```
$ rad node
```
Available options:
```
$ rad node --setup
```

### Publish to Monorepo
```
$ rad publish
```

## Subcommands

```
$ rad profile
```
Available options:
```
$ rad profile --init
$ rad profile --list
```

```
$ rad project
```
Available options:
```
$ rad project --init
$ rad project --list
```

# Discussion

- Should we use `cargo run --quiet --bin <bin>` instead of calling the pre-built binaries directly?