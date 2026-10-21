"""Resolve the ESP32-S3's ROM PHY function table against the ROM's symbols.

The table was dumped from a running node, because it lives in the ROM's RAM
and is filled at boot: the published ELF cannot show it. This resolves every
entry against the ROM's symbol index, so the table becomes a list of named
functions rather than a list of addresses.

The result is the answer to "what binary blobs are in use": the ESP32-S3's
WiFi MAC, PHY and baseband are in the ROM's `.text`, and the ROM's PHY
dispatch table is the list of hardware controls the chip exposes. Anything in
the table that names a channel estimator, a pilot, a scale or a phase is
where the CSI's sign ambiguity would live.

Australian English spelling throughout.
"""

from __future__ import annotations

import bisect
import json
import os
import sys
from typing import Dict, List, Optional, Tuple

EXTRACT_DIR = "/tmp/opencode/rea/rom_extract"


def load_index(path: str) -> Tuple[List[int], List[Dict]]:
    """The ROM's function symbols, sorted by address."""
    with open(path) as handle:
        raw = json.load(handle)
    entries = raw["symbols"]
    entries.sort(key=lambda entry: entry["address"])
    return ([entry["address"] for entry in entries], entries)


def resolve(addresses: List[int], entries: List[Dict], address: int) -> Tuple[str, bool]:
    """The name of the function containing an address, and whether it is exact.

    The ROM's symbol table is truncated: most of its internals are unnamed, so
    a name that is not an exact match is a nearest-symbol fallback and is
    marked as such rather than presented as the function's name.
    """
    if address == 0:
        return ("(null)", True)
    index = bisect.bisect_right(addresses, address) - 1
    if index < 0:
        return (f"0x{address:08X}", True)
    entry = entries[index]
    delta = address - entry["address"]
    if delta == 0:
        return (entry["name"], True)
    return (f"{entry['name']}+0x{delta:X}", False)


def in_rom(address: int) -> bool:
    """Whether an address is in the ROM's code or its RAM."""
    return 0x3F000000 <= address < 0x40200000


def main() -> int:
    """Resolve the table and write the result out."""
    table_path = os.path.join(EXTRACT_DIR, "rom_phy_table.txt")
    index_path = os.path.join(EXTRACT_DIR, "rom_symbols.json")
    if not os.path.exists(table_path) or not os.path.exists(index_path):
        print("the table dump or the symbol index is missing")
        return 1

    addresses, entries = load_index(index_path)
    rows: List[Tuple[int, int]] = []
    with open(table_path) as handle:
        for line in handle:
            offset, value = line.split()
            rows.append((int(offset, 16), int(value, 16)))

    resolved: List[Dict] = []
    for offset, value in rows:
        symbol, exact = resolve(addresses, entries, value)
        resolved.append({
            "offset": offset,
            "address": value,
            "symbol": symbol,
            "name_is_exact": exact,
            "region": (
                "rom" if in_rom(value)
                else "app" if 0x40000000 <= value < 0x44000000
                else "data"
            ),
        })

    out_path = os.path.join(EXTRACT_DIR, "rom_phy_table_resolved.json")
    with open(out_path, "w") as handle:
        json.dump({"entries": resolved}, handle, indent=1)

    rom_rows = [row for row in resolved if row["region"] == "rom"]
    app_rows = [row for row in resolved if row["region"] == "app"]
    other = [row for row in resolved if row["region"] == "data"]
    named = [row for row in rom_rows if row["name_is_exact"]]
    print(f"resolved {len(resolved)} entries: {len(rom_rows)} in the ROM "
          f"({len(named)} of them named), {len(app_rows)} in the application, "
          f"{len(other)} that are not addresses")
    print()
    print("=== the ROM's own PHY entry points ===")
    for row in resolved:
        if row["region"] == "rom":
            print(f"  {row['offset']:>4}  0x{row['address']:08X}  {row['symbol']}")
    print()
    print("=== the application's replacements, patched over the ROM's ===")
    for row in resolved:
        if row["region"] == "app":
            print(f"  {row['offset']:>4}  0x{row['address']:08X}  {row['symbol']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
