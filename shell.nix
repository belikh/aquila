# Delegates to flake.nix so `nix-shell` and `nix develop` provide the same
# toolchain: Rust 1.98.1 with the wasm32-unknown-unknown target, plus trunk,
# wasm-bindgen-cli and binaryen, and gcc/make/pkg-config.
#
#   nix-shell                     # Rust + wasm toolchain
#   nix develop .#ha              # Python + Home Assistant (HACS component tests)
#
# The plain nixpkgs `rustc` has no wasm std; that is why this defers to the flake.
let
  flake = builtins.getFlake (toString ./.);
in
flake.devShells.${builtins.currentSystem}.default
