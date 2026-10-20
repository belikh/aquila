#!/usr/bin/env bash
#
# sweep-csi-shift.sh — measure the CSI phase precision at each manual scale
#
# The ESP32-S3's automatic CSI scale leaves the samples in a handful of int8
# levels, which costs phase precision. The manual scale (manu_scale + shift)
# moves the samples into the rest of the int8 range. This sweeps the shift
# count and reports how many distinct levels the samples occupy.
#
# Australian English spelling throughout.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PORT="${PORT:-/dev/ttyACM0}"
NODE_ID="${NODE_ID:-3}"

export LD_LIBRARY_PATH="/nix/store/rv7zdild7njfakhcvl7jgcly91x8s91a-libusb-1.0.30/lib:${LD_LIBRARY_PATH:-}"
source ~/.espressif/python_env/idf5.5_py3.14_env/bin/activate
NVS_GEN="$HOME/.espressif/python_env/idf5.5_py3.14_env/lib/python3.14/site-packages/esp_idf_nvs_partition_gen/nvs_partition_gen.py"

build_nvs() {
  local shift="$1"
  local csv="$REPO_ROOT/nvs_shift_${shift}.csv"
  local bin="$REPO_ROOT/nvs_shift_${shift}.bin"
  cat > "$csv" <<CSVEOF
key,type,encoding,value
ota,namespace,,
fw_pubkey,data,hex2bin,fd7906ffc2dd85fdda500fc6e0fc1cc027271991a82da738ef415eb6bb41f046
aquila,namespace,,
ap_bssid,data,string,f4:92:bf:a1:99:d9
ap_channel,data,u8,5
csi_shift,data,u8,$shift
wifi_ssid,data,string,jupiter.au
wifi_pass,data,string,lolcats66
core_host,data,string,10.1.1.3
core_data_port,data,u16,9100
node_id,data,u16,$NODE_ID
pairing_code,data,u32,4242
CSVEOF
  python3 "$NVS_GEN" generate "$csv" "$bin" 24576 >/dev/null 2>&1
  python3 ~/esp/esp-idf/components/esptool_py/esptool/esptool.py \
    --chip esp32s3 -p "$PORT" -b 460800 \
    --before=default_reset --after=hard_reset write_flash \
    --flash_mode dio --flash_freq 80m --flash_size 4MB \
    0x9000 "$bin" 2>&1 | tail -1
}

measure() {
  local shift="$1"
  echo "--- shift=$shift ---"
  sleep 6
  python3 - "$shift" <<'PYEOF'
import sys, statistics
sys.path.insert(0, "docs/research/v2/experiments")
from hardware import CsiCollector, frames_by_node
import time
c = CsiCollector(bind_port=9100)
c.start()
time.sleep(1)
frames = c.collect_for(8.0)
c.stop()
g = frames_by_node(frames)
for nid, fs in sorted(g.items()):
    vals = [int(round(x*128)) for f in fs for x in (f.csi[10].real, f.csi[10].imag)]
    mags = [abs(x) for f in fs for x in f.csi]
    print(f"  node {nid}: {len(fs)} frames | sc10 distinct levels={len(set(vals))} "
          f"| max|csi|={max(mags):.3f} mean={statistics.mean(mags):.3f} "
          f"| raw range=[{min(vals)},{max(vals)}]")
PYEOF
}

for shift in "$@"; do
  build_nvs "$shift"
  measure "$shift"
done
