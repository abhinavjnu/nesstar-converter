class NesstarConverter < Formula
  desc "High-performance parser and converter for proprietary Nesstar survey files"
  homepage "https://github.com/abhinavjnu/nesstar-converter"
  url "https://github.com/abhinavjnu/nesstar-converter/archive/refs/tags/v1.0.7.tar.gz"
  sha256 "aef3a09f05f00dc446641f0c208553508f15186d121b2b684e01e8229f444252"
  license "MIT"
  head "https://github.com/abhinavjnu/nesstar-converter.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "build", "--release", "--bin", "nesstar-cli"
    bin.install "target/release/nesstar-cli" => "nesstar"
  end

  test do
    assert_match "Usage:", shell_output("#{bin}/nesstar --help 2>&1", 2)
  end
end
