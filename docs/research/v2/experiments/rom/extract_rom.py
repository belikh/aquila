"""Extract the ESP32-S3's ROM into analysable artefacts.

The ROM is the chip's mask-programmed firmware: the WiFi MAC, the PHY and the
baseband, including the channel estimator that produces the CSI. It is not a
flash blob — the application cannot read it out of the chip — so the only
copy is the ELF Espressif publishes, and the only way to analyse it is with
the toolchain that matches the architecture.

This script produces three artefacts from that ELF:

* ``rom_code.bin`` — every executable section, concatenated, with the load
  addresses preserved in the accompanying manifest. This is the ROM's code as
  one image, which is what a disassembly wants.
* ``rom_symbols.json`` — the symbol index: every function with its address,
  its size and the section it lives in, so a byte offset in the image can be
  resolved back to a name.
* ``rom_manifest.json`` — the provenance: the source ELF's digest, the
  section map, and a digest of every artefact.

It also extracts the ROM's **PHY function table**, which is the interesting
one: the application reaches the ROM's PHY through ``g_phyFuns``, and the
table is the list of hardware controls the chip exposes. Enumerating it says
what the hardware can be asked to do, including anything the driver does not
surface.

Australian English spelling throughout.
"""

from __future__ import annotations

import bisect
import hashlib
import json
import os
import struct
import sys
from typing import Dict, List, Optional, Tuple

from elftools.elf.elffile import ELFFile

# The section types that hold executable code.
EXECUTABLE_TYPES = {"SHT_PROGBITS"}


def digest(path: str) -> str:
    """The SHA-256 of a file."""
    hasher = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def load_symbols(elf: ELFFile) -> Tuple[List[int], List[Dict], Dict[str, int]]:
    """Every function symbol, sorted by address, plus the data symbols.

    The data symbols matter: the ROM's PHY function table is reached through
    a pointer, and it is the list of hardware controls the chip exposes.
    """
    table = elf.get_section_by_name(".symtab")
    entries: List[Dict] = []
    data: Dict[str, int] = {}
    if table is None:
        return ([], entries, data)
    for symbol in table.iter_symbols():
        info = symbol["st_info"]
        if not symbol["st_value"]:
            continue
        if info["type"] == "STT_FUNC":
            entries.append({
                "name": symbol.name,
                "address": symbol["st_value"],
                "size": symbol["st_size"],
            })
        else:
            data.setdefault(symbol.name, symbol["st_value"])
    entries.sort(key=lambda entry: entry["address"])
    return ([entry["address"] for entry in entries], entries, data)


def resolve(addresses: List[int], entries: List[Dict], address: int) -> str:
    """The name of the function containing an address, if any."""
    if not address:
        return "(null)"
    index = bisect.bisect_right(addresses, address) - 1
    if index < 0:
        return f"0x{address:08X}"
    entry = entries[index]
    delta = address - entry["address"]
    if delta == 0:
        return entry["name"]
    return f"{entry['name']}+0x{delta:X}"


def read_virtual(elf: ELFFile, address: int, length: int) -> Optional[bytes]:
    """Read raw bytes at a virtual address."""
    for segment in elf.iter_segments():
        if segment["p_type"] != "PT_LOAD":
            continue
        start = segment["p_vaddr"]
        end = start + segment["p_filesz"]
        if start <= address and address + length <= end:
            data = segment.data()
            offset = address - start
            return data[offset : offset + length]
    return None


def extract_code(elf: ELFFile, out_path: str) -> List[Dict]:
    """Concatenate the executable sections into one image."""
    image = bytearray()
    layout: List[Dict] = []
    for section in elf.iter_sections():
        if section["sh_type"] not in EXECUTABLE_TYPES:
            continue
        if not section["sh_size"]:
            continue
        if not section["sh_flags"] & 0x4:  # SHF_EXECINSTR
            continue
        data = section.data()
        layout.append({
            "name": section.name,
            "address": section["sh_addr"],
            "size": section["sh_size"],
            "offset_in_image": len(image),
        })
        image.extend(data)
    with open(out_path, "wb") as handle:
        handle.write(image)
    return layout


def find_symbol(entries: List[Dict], name: str) -> Optional[Dict]:
    """A symbol by exact name."""
    for entry in entries:
        if entry["name"] == name:
            return entry
    return None


def dump_function_table(
    elf: ELFFile,
    addresses: List[int],
    entries: List[Dict],
    table_address: int,
    table_name: str,
    limit: int = 400,
) -> List[Dict]:
    """Read a function-pointer table out of the ROM and resolve its entries."""
    raw = read_virtual(elf, table_address, limit * 4)
    if raw is None:
        return []
    pointers = struct.unpack(f"<{len(raw) // 4}I", raw)
    table: List[Dict] = []
    for index, pointer in enumerate(pointers):
        if pointer == 0:
            continue
        table.append({
            "index": index,
            "offset": index * 4,
            "address": pointer,
            "symbol": resolve(addresses, entries, pointer),
        })
    return table


def main() -> int:
    """Extract the ROM."""
    if len(sys.argv) < 2:
        print("usage: extract_rom.py <rom.elf> [out_dir]")
        return 2
    rom_path = sys.argv[1]
    out_dir = sys.argv[2] if len(sys.argv) > 2 else "/tmp/opencode/rea/rom_extract"
    os.makedirs(out_dir, exist_ok=True)

    code_path = os.path.join(out_dir, "rom_code.bin")
    symbols_path = os.path.join(out_dir, "rom_symbols.json")
    manifest_path = os.path.join(out_dir, "rom_manifest.json")

    with open(rom_path, "rb") as handle:
        elf = ELFFile(handle)
        addresses, entries, data_symbols = load_symbols(elf)
        layout = extract_code(elf, code_path)

        with open(symbols_path, "w") as handle:
            json.dump(
                {
                    "source": rom_path,
                    "symbols": entries,
                },
                handle,
                indent=1,
            )

        # The ROM's PHY function table: the app reaches the ROM's PHY through
        # it, and it is the list of hardware controls the chip exposes.
        phy_table = []
        for name in ("rom_phyFuns", "g_phyFuns_instance"):
            address = data_symbols.get(name)
            if address is None:
                continue
            phy_table.append({
                "symbol": name,
                "address": address,
                "entries": dump_function_table(
                    elf, addresses, entries, address, name
                ),
            })

        manifest = {
            "artefact": "esp32s3-rom",
            "description": (
                "the ESP32-S3's mask-programmed ROM: the WiFi MAC, the PHY "
                "and the baseband, including the channel estimator that "
                "produces the CSI"
            ),
            "source": {
                "path": rom_path,
                "sha256": digest(rom_path),
            },
            "architecture": {
                "machine": "Tensilica Xtensa",
                "bits": 32,
                "byte_order": "little",
                "elf_class": "ELF32",
            },
            "code_image": {
                "path": os.path.basename(code_path),
                "sha256": digest(code_path),
                "bytes": os.path.getsize(code_path),
                "layout": layout,
            },
            "symbol_index": {
                "path": os.path.basename(symbols_path),
                "sha256": digest(symbols_path),
                "functions": len(entries),
            },
            "phy_function_tables": phy_table,
        }
        with open(manifest_path, "w") as handle:
            json.dump(manifest, handle, indent=1)

    print(f"code image:   {code_path} "
          f"({os.path.getsize(code_path)} bytes, {len(layout)} sections)")
    for item in layout:
        print(f"    {item['name']:<24} at 0x{item['address']:08X}, "
              f"{item['size']} bytes, image offset 0x{item['offset_in_image']:X}")
    print(f"symbols:      {symbols_path} ({len(entries)} functions)")
    print(f"manifest:     {manifest_path}")
    print()
    for table in phy_table:
        print(f"  {table['symbol']} at 0x{table['address']:08X}")
        for entry in table.get("entries", [])[:24]:
            print(f"    [{entry['index']:>4}] 0x{entry['address']:08X}  "
                  f"{entry['symbol']}")
        if len(table.get("entries", [])) > 24:
            print(f"    ... {len(table['entries'])} entries in all")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
