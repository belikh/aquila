{
  description = "Aquila — reproducible dev environments (host + wasm32 + HA)";

  inputs = {
    nixpkgs.url = "nixpkgs";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ (import rust-overlay) ];
      };

      # Rust with the wasm32 target that `dashboard.md` requires; the plain
      # nixpkgs `rustc` ships no wasm std, which stalled the dashboard builder.
      rust = pkgs.rust-bin.stable."1.98.1".default.override {
        targets = [ "wasm32-unknown-unknown" ];
        extensions = [ "rust-src" "rustfmt" "clippy" ];
      };

      # Python for the HACS component: HA is a Python integration, so its
      # tests need aiohttp (HA ships it), voluptuous, pytest(+asyncio) and yaml.
      haPython = pkgs.python3.withPackages (ps: with ps; [
        aiohttp
        voluptuous
        pytest
        pytest-asyncio
        pyyaml
      ]);
    in
    {
      devShells.${system} = {
        # Default: build and test the Rust workspace, including wasm.
        default = pkgs.mkShell {
          packages = [
            rust
            pkgs.trunk
            pkgs.wasm-bindgen-cli
            pkgs.binaryen
            pkgs.wasm-tools
            pkgs.gcc
            pkgs.gnumake
            pkgs.pkg-config
            pkgs.git
          ];
          RUST_BACKTRACE = "1";
          shellHook = ''
            echo "Aquila rust shell — $(rustc --version | cut -d' ' -f2); wasm32-unknown-unknown + trunk + wasm-bindgen available"
          '';
        };

        # Home Assistant component: Python with HA dependencies (+ HA itself for
        # integration tests). `nix develop .#ha`.
        ha = pkgs.mkShell {
          packages = [
            haPython
            pkgs.home-assistant
          ];
          shellHook = ''
            echo "Aquila HA shell — python $(python3 --version | cut -d' ' -f2); aiohttp/voluptuous/pytest/pyyaml + homeassistant"
          '';
        };
      };

      # `nix run .#test` / `nix run .#dashboard`.
      apps.${system} = {
        test = {
          type = "app";
          program = toString (pkgs.writeShellScript "aquila-test" ''
            exec cargo test --workspace "$@"
          '');
        };
        dashboard = {
          type = "app";
          program = toString (pkgs.writeShellScript "aquila-dashboard" ''
            cd crates/aquila-dashboard && exec trunk build --release "$@"
          '');
        };
      };
    };
}
