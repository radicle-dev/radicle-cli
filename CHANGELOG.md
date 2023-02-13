# Changelog

## [Unreleased]

### Added

- Add Dockerfile for `radicle-cli`
- Add build metadata to version output
- `comment`: Comment on issues or patches

### Changed

- `issue`: Close and re-open issues
- `patch`: Automatically push on creation, Show reviews in list, support `--message` on creation, show only local branches on list
- `rm`: Implement profile removal

### Removed

- `auth`: Remove passphrase option, read passphrase from environment variable `RAD_PASSPHRASE` or from stdin

## [0.6.1] - 2022-08-11

### Added

- Introduce `Radicle.toml` file for config
- `review`: Review radicle patches

### Changed

- `clone`: Correct the default project name
- `track`: When showing peer information, show all branches other than master
- `patch`: Various updates on patch listing, automatically push on creation 

## [0.6.0] - 2022-06-09

### Added

- Add support for environment variable `RAD_HOME`
- `issue`: Manage radicle issues
- `patch`: Manage patches for radicle projects
- `merge`: Merge radicle patches
- `path`: Show radicle paths
- `init`: Add `--no-confirm` option for scripting
- `inspect`: Add `--id` option
- `reward:`Reward contributors of a repository
- `rm`: Add `-i` flag
- `self`: Add storage info

### Changed

- Make ssh-agent optional
- `auth`: Warn and initialize on non-active existing profile(s), disallow whitespace(s) in name
- `ens`: Only update local identity for mainnet
- `init`: Use ssh keys for gitsigners 
- `sync`: Do not fetch own identity

### Removed

- Remove `git-repository` dependency

### Fixed

- Verify signed refs on fetch
- `sync`: Correct verification order
- `auth`: Profile switching

## [0.5.1] - 2022-04-14

### Added

- `inspect`: Add `--history` option

## [0.5.0] - 2022-04-13

### Added

- `gov`: With `vote`, `propose`, `queue` and `execute`
- `auth`: Add username & password to argument list
- `rens`: Add support for WalletConnect

### Changed

- `push`: `--all` affects git command

### Removed

- Remove `openssl-sys` dependency

## [0.4.0] - 2022-03-21

### Added

- Push and pull current branches
- `rad`: Add `--version` flag
- `push`: Option to set upstream

### Changed

- `init`: Verify `.gitsigners` file
- `push`: Add verbose option to git

### Fixed

- `clone`: Fix argument parsing

## [0.3.1] - 2022-03-03

### Fixed

- `auth`: Use correct profile for storage

## [0.3.0] - 2022-03-03

### Added

- `inspect`: Add `--refs` and `--payload`

## [0.2.1] - 2022-03-02

### Added

- `sync`: Sync tags and use peer seeds if available
- `track`: Save per seed configuration

### Changed

- `track`: Display peers with no id or head

### Fixed

- `sync`: Properly use `--seed` and fix verbose output on push

## [0.2.0] - 2022-02-28

### Added

- `init`: Allow custom project name and initialization path
- `clone`: Support `rad://` URLs

### Changed

- `common`: Default HTTP for IPs
- `sync`: Collapse synching progress messages

### Fixed

- Don't overflow when rendering tables

## [0.1.2] - 2022-02-23

### Changed

- `pull`: Add to core tools

## [0.1.1] - 2022-02-23

### Changed

- Rename project to `radicle-cli`

## [0.1.0] - 2022-02-23

### Added

- `pull`: Pull radicle projects
- `inspect`: Inspect a directory for information relating to radicle
- `self`: Show information about your radicle identity and device

### Changed

- `rad`: Add `--help` option
- `sync`: Add `--identity` option
- Replace `tree` with `track`

### Removed

- `show`: Replace by `self`

## [0.0.1] - 2022-02-17

### Added

- `remote`: Manage radicle project remotes
- `tree`: View radicle project source trees

### Changed

- Update guide with contributor flow

## [0.0.0] - 2022-02-11

### Added

- `account`: Manage radicle ethereum accounts
- `auth`: Manage radicle identities and profiles
- `checkout`: Checkout a radicle project working copy
- `clone`: Clone radicle projects
- `ens`: Manage radicle ENS records
- `help`: Radicle tools help
- `init`: Initialize radicle projects from git repositories
- `ls`: List radicle projects and other objects
- `rm`: Remove radicle projects and other objects
- `push`: Publish radicle projects to the network
- `sync`: Synchronize radicle projects with seeds
- `show`: Show contextual information pertaining to radicle
- `track`: Track radicle project peers
- `untrack`: Untrack radicle project peers
- Add git remote helper for radicle