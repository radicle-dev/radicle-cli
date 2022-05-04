#!/bin/bash

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace


die () {
    >&2 echo "$@"
    exit 1
}


main () {
    if [[ $GITHUB_REF_TYPE != tag ]]; then
        die "$GITHUB_REF_TYPE: Expected 'tag' ref type"
    fi

    if [[ $GITHUB_REF_NAME != v* ]]; then
        die "$GITHUB_REF_NAME: Expected v* ref name"
    fi

    export VERSION=$(echo $GITHUB_REF_NAME | cut -c2-)
    export SHA256=$(shasum --algorithm=256 "${staging}.tar.gz" | cut -d' ' -f1)

    echo "$RADICLE_AUTOMATION_PRIVATE_KEY" >id_automation
    age --decrypt --identity id_automation --output 2510389c-eaac-4466-aaa6-c1564c1d89f7.tar.gz .github/2510389c-eaac-4466-aaa6-c1564c1d89f7.tar.gz.age
    mkdir -p ~/Library/Application\ Support/xyz.radicle.radicle-link/
    tar zxvf 2510389c-eaac-4466-aaa6-c1564c1d89f7.tar.gz -C ~/Library/Application\ Support/xyz.radicle.radicle-link/

    echo $RADICLE_AUTH_PASSWORD | rad-auth
    cargo-run --bin rad-clone -- --batch rad:git:hnrkjybkt1knwwq64ig7cxkt19xtcf6bpeugy --seed seed.alt-clients.radicle.xyz

    cd radicle-cli-homebrew
    echo >Formula/core.rb <<EOF
class Core < Formula
  version "${VERSION}"
  desc "Radicle CLI"
  homepage "https://app.radicle.network/alt-clients.radicle.eth/radicle-cli"

  url "https://github.com/radicle-dev/radicle-cli/releases/download/v#{version}/radicle-cli-x86_64-apple-darwin.tar.gz"
  sha256 "${SHA256}"

  depends_on "libusb"
  depends_on "openssl@1.1"
  depends_on "git"

  def install
    bin.install "rad-self"
    bin.install "rad-account"
    bin.install "rad-sync"
    bin.install "rad-help"
    bin.install "rad-ens"
    bin.install "rad-push"
    bin.install "rad-pull"
    bin.install "rad-clone"
    bin.install "rad-inspect"
    bin.install "rad"
    bin.install "rad-auth"
    bin.install "rad-ls"
    bin.install "rad-init"
    bin.install "rad-checkout"
    bin.install "rad-track"
    bin.install "rad-untrack"
    bin.install "git-remote-rad"

    man1.install "rad.1.gz"
    man1.install "rad-checkout.1.gz"
    man1.install "rad-sync.1.gz"
  end

  test do
    system "rad"
  end
end
EOF

    git config --global user.name automation
    git config --global user.email dummy@radicle.xyz
    git add Formula/core.rb
    git commit -m "Automatic release version bump"
    rad-push
}

main "$@"
