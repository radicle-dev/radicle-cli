##################################################
#
# Radicle alpine CLI Dockerfile - Native builds
#
# $ docker build . -t rad
# $ docker -ti --rm rad --help
#
##################################################
#
# We are using alpine not rust image
#
# - Stage 1 - Build   - Large musl builder ~ 3 GB
# - Stage 2 - Runtime - Minimal musl rt ~ 362 MB
#

##################################################
#
# Radicle alpine CLI - Stage 1 - Build
#
FROM alpine:3.15.0 as rad-build

RUN /bin/ash -c "set -ex && \
    echo \"Preparing Radicle (Build) Container apk Dependencies\" && \
    apk update && \
    apk add gcc g++ automake cmake make autoconf pkgconfig openssl openssl-dev rustup"

# rustup-init installs stable by default
RUN /bin/ash -c "set -ex && \
    echo \"Preparing Rust (Stable) Build environment\" && \
    rustup-init -q -y"

# TODO: Make it arg to install either from local volume e.g. env=dev|etc.
RUN /bin/ash -c "set -ex && \
    source $HOME/.cargo/env && \
    echo \"Preparing Radicle (Binary) Build from seed\" && \
    cargo install radicle-cli --force --locked --git https://seed.alt-clients.radicle.xyz/radicle-cli.git"

##################################################
#
# Radicle alpine CLI - Stage 2 - Runtime
#
FROM alpine:3.15.0 as radicle

# TODO: Shrink image around what gets copied.
COPY --from=rad-build /root/.cargo /root/.cargo

RUN apk update && \
    apk add openssl grep git && \
    apk upgrade

ENTRYPOINT ["/root/.cargo/bin/rad"]
CMD ["--help"]
