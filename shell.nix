{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  packages = with pkgs; [
    cargo
    rustc
    gcc
    gnumake
    clippy
    rustfmt
    pkg-config
  ];

  RUST_BACKTRACE = "1";
}
