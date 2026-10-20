{
  description = "Aquila — reproducible dev environments (host + wasm32 + HA + v2 tooling)";

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

        # v2 tooling shell: `nix develop .#sim`.
        #
        # Deliberately carries NO Python packages. This flake's `nixpkgs` is a
        # *path* input, so the binary cache does not apply and every Python
        # package would compile from source — and several nixpkgs test-suites
        # (e.g. anyio's TLS tests) fail in this environment, which cascades and
        # blocks the whole shell. Python numerics are provided instead by the
        # pip virtualenv at `.venv/` (see docs/research/v2/EXPERIMENT-ENV.md).
        sim = pkgs.mkShell {
          packages = [
            rust
            pkgs.jq
            pkgs.unzip
            pkgs.gnuplot
            pkgs.graphviz
            pkgs.gnumake
            pkgs.which
          ];
          shellHook = ''
            echo "Aquila sim shell — tooling only (rust/jq/gnuplot/graphviz)."
            echo "  Python numerics:  source .venv/bin/activate    (see docs/research/v2/EXPERIMENT-ENV.md)"
            echo "  In-repo CSI simulator: cargo run -p aquila-capture -- synth"
            echo "  Wolfram: use the Wolfram MCP tools (no local install)."
          '';
        };

        # ESP32-S3 firmware shell: `nix develop .#esp`.
        #
        # ESP-IDF and its xtensa toolchain are installed by Espressif's own
        # installer (into ~/.espressif), NOT by Nix: this flake's nixpkgs is a
        # path input (no binary cache), and a source-built IDF toolchain would
        # be enormous. This shell provides the host prerequisites, locates the
        # IDF checkout, and wires up Espressif's QEMU fork for
        # firmware-in-the-loop testing without hardware (see docs/plan/v2/map.md).
        esp = pkgs.mkShell {
          packages = [
            pkgs.python3
            pkgs.git
            pkgs.wget
            pkgs.curl
            pkgs.gnumake
            pkgs.cmake
            pkgs.ninja
            pkgs.flex
            pkgs.bison
            pkgs.gperf
            pkgs.ccache
            pkgs.dfu-util
            pkgs.unzip
            pkgs.which
          ];
          shellHook = ''
            echo "Aquila esp shell — ESP-IDF host prerequisites (python3/git/cmake/ninja/flex/bison/gperf/ccache/dfu-util)."
            idf_found=""
            for d in "''${ESP_IDF:-}" "$HOME/esp/v5.5.2/esp-idf" "$HOME/esp/esp-idf"; do
              if [ -n "$d" ] && [ -f "$d/export.sh" ]; then idf_found="$d"; break; fi
            done
            if [ -n "$idf_found" ]; then
              echo "  ESP-IDF checkout: $idf_found"
              echo "  Activate with:   . $idf_found/export.sh"
              if ls "$HOME"/.espressif/tools/qemu-xtensa/*/qemu/bin/qemu-system-xtensa >/dev/null 2>&1; then
                echo "  QEMU (espressif fork): installed"
                echo "  Firmware-in-the-loop:  idf.py qemu monitor   (no hardware needed)"
              else
                echo "  QEMU not installed yet — one-time:"
                echo "    python3 $idf_found/tools/idf_tools.py install qemu-xtensa"
                echo "  then: idf.py qemu monitor   (also: idf.py qemu gdb, qemu efuse-burn / efuse-summary)"
              fi
            else
              echo "  No ESP-IDF checkout found (checked \$ESP_IDF, ~/esp/v5.5.2/esp-idf, ~/esp/esp-idf)."
              echo "  One-time bootstrap:"
              echo "    mkdir -p ~/esp && cd ~/esp"
              echo "    git clone -b v5.5.2 --recursive https://github.com/espressif/esp-idf.git"
              echo "    cd esp-idf && ./install.sh esp32s3"
              echo "  then activate:  . ~/esp/esp-idf/export.sh"
            fi
            echo "  Note: QEMU emulates CPU/memory/digital peripherals, eFuses and secure"
            echo "        boot v2 — it does NOT emulate the radio. Sim-only results stay"
            echo "        Hardware-gated (evaluation.md §5)."
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
