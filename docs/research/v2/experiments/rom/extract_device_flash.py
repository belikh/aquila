"""Carve the node's flash into the blobs that are actually on the device.

The flash image is read from a running node with esptool. This parses the
partition table, carves each region out, and records its digest, so the
inventory of what the device actually holds is a checked artefact rather than
a claim.

It also answers a question the build files could not: the partition table
declares a `phy_init` region that `idf.py flash` never writes, so whether
that region holds anything on the device is worth looking at.

Australian English spelling throughout.
"""

from __future__ import annotations

import hashlib
import json
import os
import struct
import sys
from typing import Dict, List, Optional, Tuple

# The ESP32 image header: a magic byte, then the segment count and the
# flash-mode and size bytes.
IMAGE_MAGIC = 0xE9


def digest(data: bytes) -> str:
    """The SHA-256 of a byte string."""
    return hashlib.sha256(data).hexdigest()


def parse_partitions(image: bytes, table_offset: int) -> List[Dict]:
    """The partition table's entries."""
    entries: List[Dict] = []
    names = {0: "app", 1: "data"}
    # The subtype means different things for an app and for a data region.
    subtypes = {
        ("app", 0x10): "ota_0", ("app", 0x11): "ota_1",
        ("data", 0x00): "otadata", ("data", 0x01): "phy",
        ("data", 0x02): "nvs", ("data", 0x04): "nvs_keys",
    }
    offset = table_offset
    while offset + 32 <= len(image):
        entry = image[offset : offset + 32]
        kind, subtype = entry[2], entry[3]
        if kind == 0x01 and subtype in (0x10, 0x20):
            offset += 32
            continue
        label = entry[12:28].split(b"\x00")[0].decode("ascii", "replace")
        start, size = struct.unpack_from("<II", entry, 4)
        if start == 0xFFFFFFFF or (start == 0 and size == 0):
            break
        type_name = names.get(kind, kind)
        entries.append({
            "label": label,
            "type": type_name,
            "subtype": subtypes.get((type_name, subtype), subtype),
            "offset": start,
            "size": size,
        })
        offset += 32
    return entries


def describe_image(data: bytes) -> Optional[Dict]:
    """Describe an ESP32 application image."""
    if len(data) < 24 or data[0] != IMAGE_MAGIC:
        return None
    segments = data[1]
    flash_mode = data[2]
    # The fourth byte packs the flash size and the SPI speed: the high
    # nibble is the log2 of the flash size in bytes, the low nibble is the
    # speed code.
    # The size nibble is log2 of the flash size in megabytes.
    flash_size = (1 << (data[3] >> 4)) << 20
    flash_speed = data[3] & 0x0F
    entry = data[24:32]
    return {
        "esp32_image": True,
        "segments": segments,
        "flash_mode": flash_mode,
        "flash_size": flash_size,
        "flash_speed_code": flash_speed,
        "entry_point": struct.unpack_from("<I", entry, 0)[0],
    }


def main() -> int:
    """Carve the flash image."""
    if len(sys.argv) < 2:
        print("usage: extract_device_flash.py <flash.bin> [out_dir]")
        return 2
    flash_path = sys.argv[1]
    out_dir = sys.argv[2] if len(sys.argv) > 2 else "/tmp/opencode/rea/device"
    os.makedirs(out_dir, exist_ok=True)

    with open(flash_path, "rb") as handle:
        image = handle.read()

    partitions = parse_partitions(image, 0x8000)
    print(f"flash image: {flash_path} ({len(image)} bytes)")
    print(f"partition table at 0x8000: {len(partitions)} entries")
    print()

    manifest: List[Dict] = []
    for entry in partitions:
        start = entry["offset"]
        size = entry["size"]
        region = image[start : start + size]
        blank = all(byte == 0xFF for byte in region) if region else True
        record = {
            "label": entry["label"],
            "type": entry["type"],
            "subtype": entry["subtype"],
            "offset": start,
            "size": size,
            "sha256": digest(region),
            "all_0xff": blank,
            "non_zero_bytes": sum(1 for byte in region if byte != 0xFF),
        }
        description = describe_image(region)
        if description is not None:
            record.update(description)
        manifest.append(record)

        used = record["non_zero_bytes"]
        state = "UNWRITTEN" if blank else f"{used} bytes used"
        print(f"  {entry['label']:<12} {entry['type']}/{entry['subtype']:<8} "
              f"0x{start:06x} +0x{size:x}  {state}")
        if description is not None:
            print(f"                 an ESP32 image: {description['segments']} "
                  f"segments, entry 0x{description['entry_point']:08x}, "
                  f"{description['flash_size'] // 1024} KiB flash")

    out_path = os.path.join(out_dir, "device_flash_manifest.json")
    with open(out_path, "w") as handle:
        json.dump({
            "source": {
                "path": flash_path,
                "sha256": digest(image),
                "bytes": len(image),
            },
            "partitions": manifest,
        }, handle, indent=1)
    print()
    print(f"manifest: {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
