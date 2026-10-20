#!/usr/bin/env bash
#
# wizard-flash-test.sh — Aquila ESP32-S3 CSI node flashing & test wizard
#
# Guides you through flashing a single ESP32-S3 node and running the
# M2 / CSI capture test. Designed to be run after cloning the repo:
#
#   git clone https://github.com/belikh/aquila-v2-spec.git
#   cd aquila-v2-spec
#   bash scripts/wizard-flash-test.sh
#
# Australian English spelling throughout.
#
# Prerequisites:
#   - ESP-IDF v5.5.2 installed and exported (`. ~/esp/esp-idf/export.sh`)
#   - One ESP32-S3 node connected via USB
#   - Python 3 with pip packages (aiohttp not needed for this wizard)
#
set -euo pipefail

# ---------------------------------------------------------------------------
# Colours
# ---------------------------------------------------------------------------
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Colour

info()  { printf "${BLUE}[info]${NC}  %s\n" "$*"; }
ok()    { printf "${GREEN}[ok]${NC}   %s\n" "$*"; }
warn()  { printf "${YELLOW}[warn]${NC} %s\n" "$*"; }
err()   { printf "${RED}[error]${NC}%s\n" "$*" >&2; }

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    err "Required command not found: $1"
    return 1
  fi
}

press_enter() {
  printf "\n"
  read -rp "Press Enter when ready..." _
}

# ---------------------------------------------------------------------------
# Step 0: Check ESP-IDF
# ---------------------------------------------------------------------------
step0_check_idf() {
  info "Step 0 — Checking ESP-IDF toolchain"
  require_cmd idf.py || {
    err "idf.py not found on PATH."
    info "Export ESP-IDF first:"
    info "  . ~/esp/esp-idf/export.sh"
    err "Then re-run this wizard."
    exit 1
  }
  ok "idf.py found: $(command -v idf.py)"
  idf.py --version 2>/dev/null | head -1 || true
  require_cmd python3
  ok "python3 found: $(command -v python3)"
}

# ---------------------------------------------------------------------------
# Step 1: Find serial ports
# ---------------------------------------------------------------------------
step1_find_ports() {
  info "Step 1 — Finding ESP32-S3 serial ports"

  local ports=()
  # Linux
  mapfile -t ports < <(compgen -G '/dev/ttyUSB*' 2>/dev/null \
                      || true)
  mapfile -t _extra < <(compgen -G '/dev/ttyACM*' 2>/dev/null || true)
  ports+=("${_extra[@]}")
  # macOS
  mapfile -t _extra < <(compgen -G '/dev/cu.usbserial*' 2>/dev/null || true)
  ports+=("${_extra[@]}")
  mapfile -t _extra < <(compgen -G '/dev/cu.usbmodem*' 2>/dev/null || true)
  ports+=("${_extra[@]}")

  if [[ ${#ports[@]} -eq 0 ]]; then
    err "No serial ports found."
    info "Connect your ESP32-S3 via USB and re-run this wizard."
    info "If you just plugged it in, wait a few seconds for the OS to enumerate it."
    exit 1
  fi

  info "Detected serial port(s):"
  local i=1
  for p in "${ports[@]}"; do
    printf "  %d) %s\n" "$i" "$p"
    i=$((i + 1))
  done

  local choice
  read -rp "Select port number [1]: " choice
  choice="${choice:-1}"

  if [[ ! "$choice" =~ ^[0-9]+$ ]] || [[ "$choice" -lt 1 ]] || [[ "$choice" -gt ${#ports[@]} ]]; then
    err "Invalid selection."
    exit 1
  fi

  PORT="${ports[$((choice - 1))]}"
  ok "Selected port: $PORT"
}

# ---------------------------------------------------------------------------
# Step 2: Collect configuration
# ---------------------------------------------------------------------------
step2_collect_config() {
  info "Step 2 — Node configuration"

  read -rp "WiFi SSID: " WIFI_SSID
  read -rsp "WiFi password: " WIFI_PASSWORD
  printf "\n"
  read -rp "Core host IP (your laptop's LAN IP, e.g. 192.168.1.42): " CORE_HOST
  read -rp "Core data port [9100]: " CORE_DATA_PORT
  CORE_DATA_PORT="${CORE_DATA_PORT:-9100}"
  read -rp "Core control port [9101]: " CORE_CONTROL_PORT
  CORE_CONTROL_PORT="${CORE_CONTROL_PORT:-9101}"
  read -rp "Node ID [0]: " NODE_ID
  NODE_ID="${NODE_ID:-0}"
  read -rp "Channel [6]: " CHANNEL
  CHANNEL="${CHANNEL:-6}"
  read -rp "Bandwidth MHz [20]: " BANDWIDTH_MHZ
  BANDWIDTH_MHZ="${BANDWIDTH_MHZ:-20}"

  ok "Configuration collected."
}

# ---------------------------------------------------------------------------
# Step 3: Configure sdkconfig
# ---------------------------------------------------------------------------
step3_configure() {
  info "Step 3 — Configuring firmware (menuconfig values)"

  local firmware_dir
  firmware_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/firmware/esp32-s3-csi-node"

  if [[ ! -f "$firmware_dir/sdkconfig.defaults" ]]; then
    err "firmware directory not found at $firmware_dir"
    err "Are you running this from the aquila-v2-spec repo root?"
    exit 1
  fi

  # Back up existing sdkconfig.defaults
  cp "$firmware_dir/sdkconfig.defaults" "$firmware_dir/sdkconfig.defaults.bak.$(date +%s)"
  ok "Backed up sdkconfig.defaults"

  cat > "$firmware_dir/sdkconfig.defaults" <<EOF
# Aquila ESP32-S3 CSI node — generated by wizard-flash-test.sh
# Do not edit by hand; re-run the wizard to change.

CONFIG_AQUILA_WIFI_SSID="$WIFI_SSID"
CONFIG_AQUILA_WIFI_PASSWORD="$WIFI_PASSWORD"
CONFIG_AQUILA_CORE_HOST="$CORE_HOST"
CONFIG_AQUILA_CORE_DATA_PORT=$CORE_DATA_PORT
CONFIG_AQUILA_CORE_CONTROL_PORT=$CORE_CONTROL_PORT
CONFIG_AQUILA_NODE_ID=$NODE_ID
CONFIG_AQUILA_PAIRING_CODE=0
CONFIG_AQUILA_CHANNEL=$CHANNEL
CONFIG_AQUILA_BANDWIDTH_MHZ=$BANDWIDTH_MHZ
EOF

  ok "sdkconfig.defaults written."
}

# ---------------------------------------------------------------------------
# Step 4: Build
# ---------------------------------------------------------------------------
step4_build() {
  info "Step 4 — Building firmware"
  local firmware_dir
  firmware_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/firmware/esp32-s3-csi-node"

  (cd "$firmware_dir" && idf.py set-target esp32s3)
  (cd "$firmware_dir" && idf.py build)

  ok "Firmware built successfully."
}

# ---------------------------------------------------------------------------
# Step 5: Flash
# ---------------------------------------------------------------------------
step5_flash() {
  info "Step 5 — Flashing firmware to $PORT"
  local firmware_dir
  firmware_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/firmware/esp32-s3-csi-node"

  info "If the flash fails with a connection error, put the node in bootloader mode:"
  info "  1. Hold GPIO0 (BOOT button)"
  info "  2. Press and release RESET (EN button)"
  info "  3. Release GPIO0"
  info "  4. Then re-run this wizard."
  press_enter

  (cd "$firmware_dir" && idf.py -p "$PORT" flash)

  ok "Firmware flashed to $PORT."
}

# ---------------------------------------------------------------------------
# Step 6: Monitor (optional)
# ---------------------------------------------------------------------------
step6_monitor() {
  info "Step 6 — Monitor node output (optional)"
  read -rp "Watch live serial output? (y/n) [y]: " do_monitor
  do_monitor="${do_monitor:-y}"
  if [[ "$do_monitor" =~ ^[Yy]$ ]]; then
    local firmware_dir
    firmware_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/firmware/esp32-s3-csi-node"
    info "Opening monitor on $PORT. Press Ctrl-] to exit."
    (cd "$firmware_dir" && idf.py -p "$PORT" monitor) || true
  fi
}

# ---------------------------------------------------------------------------
# Step 7: Run CSI receiver test
# ---------------------------------------------------------------------------
step7_test_receiver() {
  info "Step 7 — Run CSI receiver test"

  read -rp "Run the CSI receiver test now? (y/n) [y]: " do_test
  do_test="${do_test:-y}"
  if [[ ! "$do_test" =~ ^[Yy]$ ]]; then
    info "Skipping receiver test. You can run it later:"
    info "  cd docs/research/v2/experiments && python3 hardware.py"
    return 0
  fi

  local repo_root
  repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  (cd "$repo_root/docs/research/v2/experiments" && python3 hardware.py) || {
    err "hardware.py failed. Check that the node is streaming CSI to UDP 9100."
    return 1
  }
}

# ---------------------------------------------------------------------------
# Step 8: Run M2 gate (optional, needs multiple nodes)
# ---------------------------------------------------------------------------
step8_m2_gate() {
  info "Step 8 — M2 characterisation (cross-node phase residual)"
  info "Note: M2 requires multiple nodes. With a single node this will show the"
  info "single-node baseline; run again when you have more nodes."
  read -rp "Run M2 characterisation now? (y/n) [n]: " do_m2
  do_m2="${do_m2:-n}"
  if [[ "$do_m2" =~ ^[Yy]$ ]]; then
    local repo_root
    repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    (cd "$repo_root/docs/research/v2/experiments" && python3 m2_characterization.py) || {
      warn "M2 characterisation did not complete. Check the output above."
    }
  fi
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
main() {
  clear 2>/dev/null || true
  echo "=============================================="
  echo "  Aquila ESP32-S3 CSI Node Wizard"
  echo "  Flash & test script"
  echo "=============================================="
  echo

  step0_check_idf
  step1_find_ports
  step2_collect_config
  step3_configure
  step4_build
  step5_flash
  step6_monitor
  step7_test_receiver
  step8_m2_gate

  echo
  echo "=============================================="
  ok "Wizard complete."
  echo "=============================================="
  echo
  info "Next steps:"
  info "  1. If hardware.py showed CSI frames, the node is streaming."
  info "  2. Run the full experiment suite:"
  info "       cd docs/research/v2/experiments && python3 test_protocol_runner.py"
  info "  3. Review results in data/ledger/ (witness-graded output)."
  echo
}

main "$@"