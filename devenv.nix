{ pkgs, ... }:
{
  languages.rust = {
    enable = true;
  };
  packages = with pkgs; [
    pkg-config
    wayland
    wayland-protocols
    wayland-scanner
    libxkbcommon
  ];
  env.RUST_BACKTRACE = "1";
  enterShell = ''
    echo "🦀 Rust Environment initialized."
    echo "📦 Cargo: $(cargo --version)"
    echo "🔍 Analyzer: $(rust-analyzer --version)"
  '';
}
