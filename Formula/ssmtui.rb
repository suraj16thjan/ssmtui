class Ssmtui < Formula
  desc "Terminal UI for AWS SSM Parameter Store"
  homepage "https://github.com/suraj16thjan/ssmtui"
  url "https://github.com/suraj16thjan/ssmtui/archive/refs/tags/v1.0.4.tar.gz"
  sha256 "5de6211945e7bda1063469ba561da6d4510c591f48d31d33c4112ee58f348be6"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "ssmtui", shell_output("#{bin}/ssmtui --help 2>&1", 2)
  end
end
