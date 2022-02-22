class RadicleTools < Formula
  version "0.0.0"
  desc "Radicle CLI tools"
  homepage "https://app.radicle.network/alt-clients.radicle.eth/radicle-client-tools"

  url "https://github.com/radicle-dev/radicle-client-tools/releases/download/v#{version}/radicle-tools-x86_64-apple-darwin.tar.gz"
  sha256 "30dc8a5627f41b0ab4b71fb7260b01167cba1c0f25dd875829ee0ebdc83dc7dd"


  def install
    bin.install "rad-self"
    bin.install "rad-inspect"
    bin.install "rad-account"
    bin.install "rad-sync"
    bin.install "rad-help"
    bin.install "rad-ens"
    bin.install "rad-push"
    bin.install "rad-pull"
    bin.install "rad"
    bin.install "rad-auth"
    bin.install "rad-track"
    bin.install "rad-ls"
    bin.install "rad-init"
    bin.install "rad-checkout"
    bin.install "rad-untrack"
    bin.install "git-remote-rad"
    man1.install "radicle-tools.1.gz"
  end

  test do
    system "rad"
  end
end
