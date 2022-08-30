#!/bin/sh
set -e

# NOTE: When using this script for testing, it's sometimes useful to
# drop into the shell and run some additional commands. For this to work,
# make sure `RAD_HOME` is set to the same value as bellow, otherwise things
# won't work. Also make sure that you are using the *debug* builds, since
# the crypto used for generating keys is different in the release build.
#
# You'll also want to run a local seed node (HTTP + Git) that uses a *different*
# radicle home than this one.
#
# Switching between the maintainer and contributor can be done with `rad auth`
# and then changing into the `contributor/acme` or `maintainer/acme` directories.
#
# Example, let's say we run our seed using `/tmp/seed` as the radicle home
# (`RAD_HOME`). First we would create an identity for the seed:
#
#   RAD_HOME=/tmp/seed rad auth --init --name seed --passphrase seed
#
# Then we would run the services:
#
#   radicle-git-server --root /tmp/seed --passphrase seed --git-receive-pack --allow-unauthorized-keys
#   radicle-http-api   --root /tmp/seed --passphrase seed
#
# Nb. Make sure the git hooks are copied into the seed's `git/hooks` folder in the monorepo,
#     otherwise identities won't be created properly.
#
#     Eg. cp target/release/{post,pre}-receive /tmp/seed/.../git/hooks
#
# Then we would run this script with a different `RAD_HOME`.
#
export RAD_HOME="$(pwd)/tmp/root"

abort() {
  echo $1 >&2
  exit 1
}

SEED_API=127.0.0.1:8777
SEED_ID=$(curl --silent http://$SEED_API | jq --raw-output .peer.id)
SEED_ADDR="$SEED_ID@127.0.0.1:8776"

[ ! -z "$SEED_ID" ] || abort "Couldn't get peer id from $SEED_API"

rad() {
  cmd=$1; shift

  echo                   >&2
  echo "▒ rad $cmd $@" >&2
  cargo run --quiet --package radicle-cli --bin rad -- $cmd "$@"
}

banner() {
  echo
  echo "▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒ $1 ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒"
  echo
}

BASE=$(pwd)

rm --recursive --force tmp/
mkdir --parents tmp/root

###################
banner "MAINTAINER"
###################

echo cloudhead | rad auth --init --name cloudhead --stdin
MAINTAINER=$(rad self --profile)

# Create git repo
mkdir --parents $BASE/tmp/maintainer/acme
cd $BASE/tmp/maintainer/acme
# Setup project seed config.
echo "ACME" > README
cat << EOF > Radicle.toml
[[seed]]
  name = "radicle.local"
  p2p = "rad://$SEED_ADDR"
  git = "http://127.0.0.1:8778"
  api = "http://127.0.0.1:8777"
EOF

# Create repo and initial commit.
git init --initial-branch master
git add .
git commit --message "Initial commit" --no-gpg-sign

# Initialize
rad init --name acme --description 'Acme Monorepo' --no-confirm
rad push

PROJECT=$(rad inspect)

####################
banner "CONTRIBUTOR"
####################

mkdir --parents $BASE/tmp/contributor
cd $BASE/tmp/contributor

echo scooby | rad auth --init --name scooby --stdin
rad clone $PROJECT --seed $SEED_ADDR --no-confirm

CONTRIBUTOR=$(rad self --profile)
CONTRIBUTOR_PEER=$(rad self --peer)

# Change into project directory
cd acme

# Create change
echo >> README
echo "Acme is such a great company!" >> README
git add README
git commit --message "Update README" --no-gpg-sign

# Push commit to monorepo
# (rad-push)
git push rad
# Create patch
rad patch --sync --message "Update README" --message "Reflect the recent positive news"

###################
banner "MAINTAINER"
###################

cd $BASE/tmp/maintainer/acme

rad auth $MAINTAINER
rad track $CONTRIBUTOR_PEER
rad patch --list

rm .gitignore
rad review --accept hnrk --message "LGTM." # Will match the only patch
rad merge hnrk
rad push

###################
banner "CONTRIBUTOR"
###################

cd $BASE/tmp/contributor/acme

rad auth $CONTRIBUTOR

# Checkout the branch of the maintainer
git log
git checkout peers/cloudhead/master
# Pull the changes (the patch merge)
rad pull

# Compare and test that the branches are the same
MINE=$(git rev-parse master)
THEIRS=$(git rev-parse peers/cloudhead/master)

[[ $MINE = $THEIRS ]] || {
  echo "fatal: commit hashes do not match: $MINE vs. $THEIRS" >&2
  exit 1
}

echo "█ ok"
