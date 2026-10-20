#!/usr/bin/env bash
#
# wizard-flash-3-more.sh — Flash 3 additional ESP32-S3 CSI nodes (IDs 2, 3, 4)
#
# Uses the same firmware binary for all nodes; each node gets a unique
# node_id via its NVS partition (flashed to offset 0x9000).
#
# Australian English spelling throughout.
#
# Prerequisites:
#   - ESP-IDF v5.5.2 installed and exported (`. ~/esp/esp-idf/export.sh`)
#   - Firmware already built (idf.py build in firmware/esp32-s3-csi-node)
#   - One ESP32-S3 connected at a time via USB
#
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()  { printf "${BLUE}[info]${NC}  %s\n" "$*"; }
ok()    { printf "${GREEN}[ok]${NC}   %s\n" "$*"; }
warn()  { printf "${YELLOW}[warn]${NC} %s\n" "$*"; }
err()   { printf "${RED}[error]${NC}%s\n" "$*" >&2; }

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIRMWARE_DIR="$REPO_ROOT/firmware/esp32-s3-csi-node"
EXPERIMENTS_DIR="$REPO_ROOT/docs/research/v2/experiments"
NVS_GEN="$HOME/.espressif/python_env/idf5.5_py3.14_env/lib/python3.14/site-packages/esp_idf_nvs_partition_gen/nvs_partition_gen.py"
LIBUSB_PATH="/nix/store/rv7zdild7njfakhcvl7jgcly91x8s91a-libusb-1.0.30/lib"

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    err "Required command not found: $1"
    return 1
  }
}

press_enter() {
  read -rp "Press Enter when ready..." _
}

detect_ports() {
  local ports=()
  mapfile -t _p < <(compgen -G '/dev/ttyUSB*' 2>/dev/null || true)
  ports+=("${_p[@]}")
  mapfile -t _p < <(compgen -G '/dev/ttyACM*' 2>/dev/null || true)
  ports+=("${_p[@]}")
  mapfile -t _p < <(compgen -G '/dev/cu.usbserial*' 2>/dev/null || true)
  ports+=("${_p[@]}")
  mapfile -t _p < <(compgen -G '/dev/cu.usbmodem*' 2>/dev/null || true)
  ports+=("${_p[@]}")
  local seen=() result=()
  for p in "${ports[@]}"; do
    local found=0
    for s in "${seen[@]}"; do
      [[ "$s" == "$p" ]] && { found=1; break; }
    done
    if [[ $found -eq 0 ]]; then
      seen+=("$p")
      result+=("$p")
    fi
  done
  printf '%s\n' "${result[@]}"
}

generate_nvs() {
  local node_id="$1"
  local csv="$REPO_ROOT/nvs_node_${node_id}.csv"
  local bin="$REPO_ROOT/nvs_node_${node_id}.bin"

  cat > "$csv" <<CSVEOF
key,type,encoding,value
aquila,namespace,,
wifi_ssid,data,string,jupiter.au
wifi_pass,data,string,"!@#iotdevice!@#"
core_host,data,string,10.1.1.3
core_data_port,data,u16,9100
node_id,data,u16,$node_id
pairing_code,data,u32,0
CSVEOF

  export LD_LIBRARY_PATH="$LIBUSB_PATH"
  source ~/.espressif/python_env/idf5.5_py3.14_env/bin/activate
  python3 "$NVS_GEN" "$csv" "$bin" 24576 >/dev/null 2>&1 || {
    err "NVS generation failed for node $node_id"
    return 1
  }
  ok "NVS image for node $node_id: $bin"
}

flash_node() {
  local node_id="$1"

  info "=== Flashing node $node_id ==="

  # Generate NVS for this node
  generate_nvs "$node_id"

  # Prompt to plug in
  info "Plug in ESP32-S3 node $node_id (node ID $node_id)."
  press_enter

  # Detect port
  mapfile -t PORTS < <(detect_ports)
  if [[ ${#PORTS[@]} -eq 0 ]]; then
    err "No serial port detected."
    return 1
  fi

  if [[ ${#PORTS[@]} -eq 1 ]]; then
    PORT="${PORTS[0]}"
  else
    info "Multiple ports detected:"
    local i=1
    for p in "${PORTS[@]}"; do
      printf "  %d) %s\n" "$i" "$p"
      i=$((i + 1))
    done
    read -rp "Select port number [1]: " choice
    choice="${choice:-1}"
    PORT="${PORTS[$((choice - 1))]}"
  fi
  ok "Using port: $PORT"

  # Flash firmware
  info "Flashing firmware to node $node_id..."
  export LD_LIBRARY_PATH="$LIBUSB_PATH"
  source ~/.espressif/python_env/idf5.5_py3.14_env/bin/activate
  . ~/esp/esp-idf/export.sh
  (cd "$FIRMWARE_DIR" && idf.py -p "$PORT" flash) 2>&1 | tail -5
  ok "Firmware flashed to node $node_id."

  # Flash NVS
  info "Flashing NVS to node $node_id (offset 0x9000)..."
  python3 ~/esp/esp-idf/components/esptool_py/esptool/esptool.py \
    --chip esp32s3 -p "$PORT" -b 460800 \
    --before=default_reset --after=hard_reset \
    write_flash --flash_mode dio --flash_freq 80m --flash_size 4MB \
    0x9000 "$REPO_ROOT/nvs_node_${node_id}.bin" 2>&1 | tail -3
  ok "NVS flashed to node $node_id."

  ok "Node $node_id complete."
}

main() {
  echo "=============================================="
  echo "  Aquila ESP32-S3 — Flash 3 More Nodes (2, 3, 4)"
  echo "=============================================="
  echo
  info "This script will flash 3 additional nodes sequentially."
  info "All nodes run the same firmware binary; node IDs come from NVS."
  echo

  require_cmd idf.py || {
    err "idf.py not found. Export ESP-IDF first:"
    err "  . ~/esp/esp-idf/export.sh"
    exit 1
  }

  # Verify firmware is built
  if [[ ! -f "$FIRMWARE_DIR/build/aquila_esp32s3_csi_node.bin" ]]; then
    err "Firmware not built at $FIRMWARE_DIR/build/aquila_esp32s3_csi_node.bin"
    err "Run idf.py build first."
    exit 1
  fi
  ok "Firmware binary found."

  for node_id in 2 3 4; do
    echo
    echo "----------------------------------------------"
    info "Node $node_id of 4 (additional nodes 2, 3, 4)"
    echo "----------------------------------------------"
    flash_node "$node_id"
    if [[ "$node_id" != 4 ]]; then
      info "Node $node_id flashed successfully."
      read -rp "Unplug node $node_id and plug in node $((node_id + 1)), then press Enter..." _
    fi
  done

  echo
  echo "=============================================="
  ok "All 3 additional nodes flashed (IDs 2, 3, 4)."
  echo "=============================================="
  echo
  info "Node configuration:"
  info "  SSID: jupiter.au"
  info "  Core host: 10.1.1.3:9100 (data) / 9101 (control)"
  info "  Node IDs: 1 (already flashed), 2, 3, 4"
  echo
  info "To run the full experiment suite once all nodes are deployed:"
  info "  cd docs/research/v2/experiments && python3 test_protocol_runner.py"
}

main "$@"
