#!/bin/sh

mkdir "$RAD_DEV/tmp/radicle/"

export PATH="$RAD_DEV/radicle-link/bins/target/debug/:$PATH"
export PATH="$RAD_DEV/radicle-link/target/debug/:$PATH"

export RAD_HOME="$RAD_DEV/tmp/radicle/"
alias rad="python $RAD_DEV/radicle-client-tools/rad-prototype/rad.py"