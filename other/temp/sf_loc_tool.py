"""
SpellForce 1 & 2 - CFF Database & Localization Suite
Full Python implementation synchronized with the native Rust core.
Supports CFF containers (SF1 & SF2), automated format detection (Formats A, B, C, Fixed-566),
clean translator-friendly JSON exports with sidecar .meta.json files, and Cyrillic CP1251 heuristics.
"""

import argparse
import contextlib
import json
import os
import re
import shutil
import struct
import sys
import threading
import zlib

# ==============================================================================
# ENCODING HELPERS (Windows-1251 / Windows-1252 / UTF-8)
# ==============================================================================


def decode_windows(bytes_data: bytes) -> str:
    """
    Decodes binary strings safely.
    Checks UTF-8 first, applies Cyrillic (CP1251) frequency analysis,
    and falls back to Windows-1252 to prevent mojibake.
    """
    try:
        return bytes_data.decode("utf-8")
    except UnicodeDecodeError:
        pass

    # Heuristic for Cyrillic: in CP1251, Russian characters fall in 0xC0..0xFF
    cyrillic_hits = sum(1 for b in bytes_data if b >= 0xC0)
    if cyrillic_hits > 0:
        try:
            return bytes_data.decode("cp1251")
        except UnicodeDecodeError:
            pass

    return bytes_data.decode("cp1252", errors="replace")


def encode_windows(text: str) -> bytes:
    """Encodes strings to bytes, picking CP1251 if Cyrillic is present, else CP1252."""
    has_cyrillic = any("\u0400" <= c <= "\u04ff" for c in text)
    if has_cyrillic:
        return text.encode("cp1251", errors="replace")
    try:
        return text.encode("cp1252")
    except UnicodeEncodeError:
        return text.encode("cp1251", errors="replace")


# ==============================================================================
# 1. CFF CONTAINER PACK / UNPACK ENGINE (SF1 & SF2)
# ==============================================================================


def unpack_cff(input_file: str, out_dir: str) -> bool:
    """Unpacks a .cff archive container into separate .dat chunks."""
    print(f"[*] Unpacking CFF: {input_file} -> {out_dir}")
    os.makedirs(out_dir, exist_ok=True)

    try:
        with open(input_file, "rb") as f:
            data = f.read()
    except OSError as e:
        print(f"[!] Error reading file: {e}")
        return False

    if len(data) < 20:
        print("[!] Error: File too small to be a CFF container!")
        return False

    sig = data[0:4]
    if sig == b"\x02\xc5r\xdd":
        fmt_type = "sf1"
        print("[*] Detected SpellForce 1 CFF container (Original).")
    elif sig == b"\x12\xdd\x72\xdd":
        h2, h3, h4, h5 = struct.unpack_from("<IIII", data, 4)
        if h2 == 2 and h3 == 2 and h4 == 1 and h5 == 0:
            fmt_type = "sf1"
            print("[*] Detected SpellForce 1 CFF container (Platinum Edition).")
        else:
            if len(data) > 36:
                _, _, cs_2, _, us_2 = struct.unpack_from("<IHIHI", data, 20)
                if cs_2 == 0 and us_2 > len(data):
                    fmt_type = "sf1"
                    print("[*] Detected SpellForce 1 CFF container (Heuristic Match).")
                else:
                    fmt_type = "sf2"
                    print("[*] Detected SpellForce 2 CFF container.")
            else:
                fmt_type = "sf2"
                print("[*] Detected SpellForce 2 CFF container.")
    else:
        print(f"[!] Error: Invalid CFF signature! (Found: {sig.hex()})")
        return False

    with open(os.path.join(out_dir, "header.bin"), "wb") as f:
        f.write(data[0:20])

    manifest = {"format": fmt_type, "chunks": []}
    offset = 20
    chunk_idx = 0

    while offset + 12 <= len(data):
        if fmt_type == "sf1":
            c_id, occurrence, comp_flag, comp_size, c_type = struct.unpack_from(
                "<hhhih", data, offset
            )
            offset += 12

            if comp_flag == 0:
                uncomp_data = data[offset : offset + comp_size]
                offset += comp_size
            else:
                if offset + 4 > len(data):
                    break
                _uncomp_size = struct.unpack_from("<i", data, offset)[0]
                offset += 4
                comp_data = data[offset : offset + comp_size]
                offset += comp_size
                try:
                    uncomp_data = zlib.decompress(comp_data)
                except zlib.error as e:
                    print(f"[!] Error decompressing SF1 chunk {chunk_idx}: {e}")
                    uncomp_data = comp_data

            chunk_name = f"chunk_{chunk_idx}.dat"
            with open(os.path.join(out_dir, chunk_name), "wb") as f:
                f.write(uncomp_data)

            manifest["chunks"].append(
                {
                    "file": chunk_name,
                    "id": c_id,
                    "occurrence": occurrence,
                    "comp_flag": comp_flag,
                    "type": c_type,
                }
            )
        else:  # sf2
            if offset + 16 > len(data):
                break
            c_id, flag1, comp_size, flag2, _uncomp_size = struct.unpack_from(
                "<IHIHI", data, offset
            )
            offset += 16
            comp_data = data[offset : offset + comp_size]
            offset += comp_size

            try:
                uncomp_data = zlib.decompress(comp_data)
            except zlib.error as e:
                print(f"[!] Error decompressing SF2 chunk {chunk_idx}: {e}")
                uncomp_data = comp_data

            chunk_name = f"chunk_{chunk_idx}.dat"
            with open(os.path.join(out_dir, chunk_name), "wb") as f:
                f.write(uncomp_data)

            manifest["chunks"].append(
                {
                    "file": chunk_name,
                    "id": c_id,
                    "flag1": flag1,
                    "flag2": flag2,
                }
            )
        chunk_idx += 1

    with open(os.path.join(out_dir, "manifest.json"), "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=4)

    print(f"[+] Container unpacking completed! Extracted {chunk_idx} chunks.")
    return True


def pack_cff(input_dir: str, out_file: str, comp_level: int = 6) -> bool:
    """Packs raw .dat chunks back into a unified .cff container."""
    manifest_path = os.path.join(input_dir, "manifest.json")
    header_path = os.path.join(input_dir, "header.bin")

    if not os.path.exists(manifest_path) or not os.path.exists(header_path):
        print("[!] Error: manifest.json or header.bin not found!")
        return False

    if os.path.exists(out_file):
        bak_file = out_file + ".bak"
        if not os.path.exists(bak_file):
            shutil.copy2(out_file, bak_file)

    with open(manifest_path, "r", encoding="utf-8") as f:
        manifest = json.load(f)

    fmt_type = manifest.get("format", "sf2")
    print(
        f"[*] Packing CFF ({fmt_type.upper()}) (compression level: {comp_level}): {input_dir} -> {out_file}"
    )

    out_dir = os.path.dirname(os.path.abspath(out_file))
    if out_dir:
        os.makedirs(out_dir, exist_ok=True)

    with open(out_file, "wb") as out:
        with open(header_path, "rb") as hf:
            out.write(hf.read())

        for chunk in manifest.get("chunks", []):
            chunk_file_path = os.path.join(input_dir, chunk["file"])
            if not os.path.exists(chunk_file_path):
                print(f"[!] Warning: Chunk file '{chunk['file']}' not found! Skipping.")
                continue

            with open(chunk_file_path, "rb") as cf:
                uncomp_data = cf.read()

            if fmt_type == "sf1":
                comp_flag = chunk.get("comp_flag", 1)
                occurrence = chunk.get("occurrence", 0)
                c_type = chunk.get("type", 0)

                if comp_flag == 0:
                    out.write(
                        struct.pack(
                            "<hhhih",
                            chunk["id"],
                            occurrence,
                            0,
                            len(uncomp_data),
                            c_type,
                        )
                    )
                    out.write(uncomp_data)
                else:
                    comp_data = zlib.compress(uncomp_data, level=comp_level)
                    out.write(
                        struct.pack(
                            "<hhhih",
                            chunk["id"],
                            occurrence,
                            comp_flag,
                            len(comp_data),
                            c_type,
                        )
                    )
                    out.write(struct.pack("<i", len(uncomp_data)))
                    out.write(comp_data)
            else:
                flag1 = chunk.get("flag1", 0)
                flag2 = chunk.get("flag2", 0)
                comp_data = zlib.compress(uncomp_data, level=comp_level)
                out.write(
                    struct.pack(
                        "<IHIHI",
                        chunk["id"],
                        flag1,
                        len(comp_data),
                        flag2,
                        len(uncomp_data),
                    )
                )
                out.write(comp_data)

    print("[+] CFF container packing completed successfully!")
    return True


# ==============================================================================
# 2. FORMAT DETECTION & TEXT TRANSLATION EXPORTER / IMPORTER
# ==============================================================================


def detect_format(data: bytes):
    """Detects binary chunk format (Format A, B, C, Fixed-566, or unknown)."""
    if len(data) < 8:
        return "unknown", 0, 0

    if len(data) >= 566 and len(data) % 566 == 0:
        is_f566 = True
        for i in range(min(5, len(data) // 566)):
            if data[i * 566 + 565] != 0:
                is_f566 = False
                break
        if is_f566:
            return "fixed_566", 0, 0

    count = struct.unpack_from("<I", data, 0)[0]
    if count == 0 or count > 200000:
        return "unknown", 0, 0

    # Format C: Developer Table (0x02)
    with contextlib.suppress(struct.error, IndexError):
        offset = 4
        is_c = True
        for _ in range(count):
            if offset + 6 > len(data) or data[offset] != 0x02:
                is_c = False
                break
            offset += 6
            if offset + 4 > len(data):
                is_c = False
                break
            name_len = struct.unpack_from("<I", data, offset)[0]
            if name_len > 100000 or offset + 4 + name_len > len(data):
                is_c = False
                break
            offset += 4 + name_len
            if offset + 4 > len(data):
                is_c = False
                break
            key_len = struct.unpack_from("<I", data, offset)[0]
            if key_len > 1000 or offset + 4 + key_len > len(data):
                is_c = False
                break
            offset += 4 + key_len
        if is_c and offset == len(data):
            return "developer_table", 0, 0

    # Format A: String Table (0x01)
    with contextlib.suppress(struct.error, IndexError):
        offset = 4
        is_a = True
        for _ in range(count):
            if offset + 5 > len(data) or data[offset] != 0x01:
                is_a = False
                break
            offset += 1
            key_len = struct.unpack_from("<I", data, offset)[0]
            if key_len > 1000 or offset + 4 + key_len > len(data):
                is_a = False
                break
            offset += 4 + key_len
            if offset + 4 > len(data):
                is_a = False
                break
            text_len = struct.unpack_from("<I", data, offset)[0]
            if text_len > 100000 or offset + 4 + text_len * 2 > len(data):
                is_a = False
                break
            offset += 4 + text_len * 2
        if is_a and offset == len(data):
            return "string_table", 0, 0

    # Format B: Table Based (ID + E extra bytes + N strings)
    for E in range(33):
        for N in range(1, 11):
            with contextlib.suppress(struct.error, IndexError):
                offset = 4
                is_b = True
                for _ in range(count):
                    if offset + 4 + E > len(data):
                        is_b = False
                        break
                    offset += 4 + E
                    for _ in range(N):
                        if offset + 4 > len(data):
                            is_b = False
                            break
                        str_len = struct.unpack_from("<I", data, offset)[0]
                        if str_len > 100000 or offset + 4 + str_len * 2 > len(data):
                            is_b = False
                            break
                        offset += 4 + str_len * 2
                if is_b and offset == len(data):
                    return "table_based", N, E

    return "unknown", 0, 0


def export_text(chunk_path: str, json_path: str) -> bool:
    """Exports texts from a binary chunk into JSON with separate .meta.json sidecar."""
    if not os.path.exists(chunk_path):
        print(f"[!] Error: Chunk file not found: {chunk_path}")
        return False

    with open(chunk_path, "rb") as f:
        data = f.read()

    if len(data) < 4:
        return False

    fmt, num_strings, extra_bytes = detect_format(data)
    if fmt == "unknown":
        return False

    print(
        f"[*] Exporting text: {os.path.basename(chunk_path)} -> {os.path.basename(json_path)} | format: {fmt}"
    )
    texts = {}

    if fmt == "fixed_566":
        offset = 0
        while offset + 566 <= len(data):
            block = data[offset : offset + 566]
            str_id = struct.unpack_from("<I", block, 0)[0]
            text_bytes = block[54:566]
            null_idx = text_bytes.find(b"\x00")
            if null_idx != -1:
                text_bytes = text_bytes[:null_idx]

            text = decode_windows(text_bytes)
            texts[f"f566_{offset:08d}_{str_id}"] = text
            offset += 566

    else:
        count = struct.unpack_from("<I", data, 0)[0]
        offset = 4

        if fmt == "string_table":
            for _ in range(count):
                if offset >= len(data):
                    break
                offset += 1
                key_len = struct.unpack_from("<I", data, offset)[0]
                offset += 4
                key = data[offset : offset + key_len].decode("utf-8", errors="replace")
                offset += key_len
                text_len = struct.unpack_from("<I", data, offset)[0]
                offset += 4
                text = data[offset : offset + (text_len * 2)].decode(
                    "utf-16-le", errors="replace"
                )
                offset += text_len * 2
                texts[key] = text

        elif fmt == "developer_table":
            for i in range(count):
                if offset >= len(data):
                    break
                offset += 1
                id_val = struct.unpack_from("<I", data, offset)[0]
                offset += 4
                flag = data[offset]
                offset += 1
                name_len = struct.unpack_from("<I", data, offset)[0]
                offset += 4

                name = decode_windows(data[offset : offset + name_len])
                offset += name_len
                key_len = struct.unpack_from("<I", data, offset)[0]
                offset += 4
                key = decode_windows(data[offset : offset + key_len])
                offset += key_len
                texts[f"{i:05d}_{id_val}_{flag}_{key}"] = name

        else:  # table_based
            for i in range(count):
                if offset >= len(data):
                    break
                id_val = struct.unpack_from("<I", data, offset)[0]
                offset += 4
                param_bytes = b""
                if extra_bytes > 0:
                    param_bytes = data[offset : offset + extra_bytes]
                    offset += extra_bytes
                extra_hex = param_bytes.hex()
                for s in range(num_strings):
                    if offset + 4 > len(data):
                        break
                    str_len = struct.unpack_from("<I", data, offset)[0]
                    offset += 4
                    text = data[offset : offset + str_len * 2].decode(
                        "utf-16-le", errors="replace"
                    )
                    offset += str_len * 2
                    texts[f"{i:05d}_{id_val}_{extra_hex}_str{s}"] = text

    with open(json_path, "w", encoding="utf-8") as f:
        json.dump(texts, f, indent=4, ensure_ascii=False)

    # Save format metadata as sidecar file to keep translation text clean
    meta_path = os.path.splitext(json_path)[0] + ".meta.json"
    with open(meta_path, "w", encoding="utf-8") as mf:
        json.dump(
            {
                "format": fmt,
                "num_strings": num_strings,
                "extra_bytes": extra_bytes,
            },
            mf,
            indent=4,
        )

    return True


def import_text(json_path: str, chunk_path: str) -> bool:
    """Imports edited JSON text back into binary CFF chunk."""
    if not os.path.exists(json_path):
        print(f"[!] Error: JSON file not found: {json_path}")
        return False

    with open(json_path, "r", encoding="utf-8") as f:
        texts = json.load(f)

    if not texts:
        return False

    is_table_based = False
    is_developer_table = False
    is_fixed_566 = False
    num_strings = 0

    # 1. Read format from sidecar metadata if present
    meta_path = os.path.splitext(json_path)[0] + ".meta.json"
    if os.path.exists(meta_path):
        try:
            with open(meta_path, "r", encoding="utf-8") as mf:
                meta = json.load(mf)
            fmt = meta.get("format")
            if fmt == "fixed_566":
                is_fixed_566 = True
            elif fmt == "developer_table":
                is_developer_table = True
            elif fmt == "table_based":
                is_table_based = True
                num_strings = meta.get("num_strings", 0)
        except OSError:
            pass

    # 2. Fallback to in-json metadata tag or legacy key heuristics
    if not (is_table_based or is_developer_table or is_fixed_566):
        meta_tag = texts.pop("_metadata", None)
        if meta_tag and isinstance(meta_tag, dict):
            fmt = meta_tag.get("format")
            if fmt == "fixed_566":
                is_fixed_566 = True
            elif fmt == "developer_table":
                is_developer_table = True
            elif fmt == "table_based":
                is_table_based = True
                num_strings = meta_tag.get("num_strings", 0)
        else:
            first_key = next(iter(texts), None)
            if first_key is not None:
                if first_key.startswith("f566_"):
                    is_fixed_566 = True
                elif "_" in first_key:
                    parts = first_key.split("_", 3)
                    if len(parts) == 4 and parts[0].isdigit() and parts[1].isdigit():
                        if parts[3].startswith("str"):
                            is_table_based = True
                        else:
                            is_developer_table = True

    if is_table_based and num_strings == 0:
        max_str_idx = 0
        for key in texts:
            k_parts = key.split("_", 3)
            if len(k_parts) == 4 and k_parts[3].startswith("str"):
                with contextlib.suppress(ValueError):
                    str_idx = int(k_parts[3][3:])
                    max_str_idx = max(max_str_idx, str_idx)
        num_strings = max_str_idx + 1

    # Preserve original binary chunk with .bak backup
    if os.path.exists(chunk_path):
        bak_file = chunk_path + ".bak"
        if not os.path.exists(bak_file):
            shutil.copy2(chunk_path, bak_file)

    if is_fixed_566:
        with open(chunk_path, "rb") as f:
            orig_data = bytearray(f.read())

        for key, val in texts.items():
            if not key.startswith("f566_"):
                continue
            parts = key.split("_")
            offset = int(parts[1])

            text_bytes = encode_windows(val)
            if len(text_bytes) > 511:
                text_bytes = text_bytes[:511]

            padded = text_bytes.ljust(512, b"\x00")
            if offset + 566 <= len(orig_data):
                orig_data[offset + 54 : offset + 566] = padded

        with open(chunk_path, "wb") as f:
            f.write(orig_data)
        print(
            f"[+] Fixed-566 text successfully compiled to {os.path.basename(chunk_path)}"
        )
        return True

    with open(chunk_path, "wb") as f:
        if not is_table_based and not is_developer_table:
            f.write(struct.pack("<I", len(texts)))
            for key, text in texts.items():
                f.write(b"\x01")
                kb = key.encode("utf-8")
                f.write(struct.pack("<I", len(kb)))
                f.write(kb)
                tb = text.encode("utf-16-le")
                f.write(struct.pack("<I", len(tb) // 2))
                f.write(tb)

        elif is_developer_table:
            entries = {}
            for key, val in texts.items():
                parts = key.split("_", 3)
                idx = int(parts[0])
                id_val = int(parts[1])
                flag = int(parts[2])
                dev_key = parts[3]
                entries[idx] = {
                    "id": id_val,
                    "flag": flag,
                    "name": val,
                    "key": dev_key,
                }
            sorted_indices = sorted(entries)
            f.write(struct.pack("<I", len(sorted_indices)))
            for idx in sorted_indices:
                entry = entries[idx]
                f.write(struct.pack("<B", 0x02))
                f.write(struct.pack("<I", entry["id"]))
                f.write(struct.pack("<B", entry["flag"]))

                name_bytes = encode_windows(entry["name"])
                f.write(struct.pack("<I", len(name_bytes)))
                f.write(name_bytes)
                key_bytes = encode_windows(entry["key"])
                f.write(struct.pack("<I", len(key_bytes)))
                f.write(key_bytes)
        else:
            entries = {}
            for key, val in texts.items():
                parts = key.split("_", 3)
                if len(parts) != 4 or not parts[3].startswith("str"):
                    continue
                try:
                    idx = int(parts[0])
                    id_val = int(parts[1])
                    extra_hex = parts[2]
                    str_idx = int(parts[3][3:])
                except (ValueError, IndexError):
                    continue

                if idx not in entries:
                    try:
                        extra_bytes = bytes.fromhex(extra_hex) if extra_hex else b""
                    except ValueError:
                        extra_bytes = b""
                    entries[idx] = {
                        "id": id_val,
                        "extra_bytes": extra_bytes,
                        "strings": {},
                    }
                entries[idx]["strings"][str_idx] = val

            sorted_indices = sorted(entries)
            f.write(struct.pack("<I", len(sorted_indices)))
            for idx in sorted_indices:
                entry = entries[idx]
                f.write(struct.pack("<I", entry["id"]))
                if entry["extra_bytes"]:
                    f.write(entry["extra_bytes"])
                for s in range(num_strings):
                    text_val = entry["strings"].get(s, "")
                    text_bytes = text_val.encode("utf-16-le")
                    f.write(struct.pack("<I", len(text_bytes) // 2))
                    f.write(text_bytes)

    print(f"[+] Text successfully imported into {os.path.basename(chunk_path)}")
    return True


# ==============================================================================
# 3. BATCH WORKFLOW ENGINE
# ==============================================================================


def unpack_all(cff_path: str, work_dir: str) -> bool:
    """Full batch unpack: unpacks CFF container and exports all text tables to JSON."""
    print(f"[*] Starting full unpack cycle: {cff_path} -> {work_dir}")
    if not unpack_cff(cff_path, work_dir):
        return False

    json_dir = os.path.join(work_dir, "texts_json")
    os.makedirs(json_dir, exist_ok=True)

    exported_count = 0
    skipped_count = 0

    for file in sorted(os.listdir(work_dir)):
        match = re.match(r"^chunk_(\d+)\.dat$", file)
        if not match:
            continue
        chunk_idx = int(match.group(1))
        chunk_path = os.path.join(work_dir, file)

        with open(chunk_path, "rb") as f:
            data = f.read()

        if len(data) < 8:
            skipped_count += 1
            continue

        fmt, _, _ = detect_format(data)
        if fmt == "unknown":
            skipped_count += 1
            continue

        desc_name = f"chunk_{chunk_idx}_strings.json"
        json_path = os.path.join(json_dir, desc_name)

        if export_text(chunk_path, json_path):
            exported_count += 1
        else:
            skipped_count += 1

    print(
        f"[+] Unpack completed! Exported: {exported_count} text tables, Skipped: {skipped_count} binary chunks."
    )
    return True


def pack_all(work_dir: str, cff_path: str, comp_level: int = 6) -> bool:
    """Full batch pack: validates, compiles edited JSONs back, and rebuilds the CFF archive."""
    print(
        f"[*] Starting full pack cycle (compression: {comp_level}): {work_dir} -> {cff_path}"
    )
    json_dir = os.path.join(work_dir, "texts_json")

    if not os.path.exists(json_dir):
        print("[!] Error: texts_json directory not found!")
        return False

    # Pre-flight syntax validation
    has_errors = False
    for file in sorted(os.listdir(json_dir)):
        if file.endswith("_strings.json"):
            json_path = os.path.join(json_dir, file)
            try:
                with open(json_path, "r", encoding="utf-8") as f:
                    json.load(f)
            except json.JSONDecodeError as je:
                print(f"[!] JSON syntax error in '{file}': {je}")
                has_errors = True

    if has_errors:
        print("[!] Packing aborted due to JSON errors!")
        return False

    imported_count = 0
    for file in sorted(os.listdir(json_dir)):
        if file.endswith("_strings.json"):
            json_path = os.path.join(json_dir, file)
            match = re.match(r"^chunk_(\d+)", file)
            if not match:
                continue
            chunk_idx = int(match.group(1))
            chunk_file = f"chunk_{chunk_idx}.dat"
            chunk_path = os.path.join(work_dir, chunk_file)

            if os.path.exists(chunk_path) and import_text(json_path, chunk_path):
                imported_count += 1

    print(f"[+] Re-imported {imported_count} JSON tables.")
    if pack_cff(work_dir, cff_path, comp_level):
        print("[+] Localized CFF archive compiled successfully!")
        return True
    return False


# ==============================================================================
# 4. GRAPHICAL USER INTERFACE (Tkinter)
# ==============================================================================


def launch_gui():
    """Launches the desktop GUI for CFF localization."""
    import tkinter as tk
    from tkinter import filedialog, messagebox

    root = tk.Tk()
    root.title("SpellForce 1 & 2 - CFF Localization Tool")
    root.geometry("740x580")
    root.configure(bg="#212121")

    fg_color = "#ffffff"
    bg_color = "#212121"
    btn_color = "#333333"
    entry_color = "#2d2d2d"

    class GuiLogger:
        def __init__(self, text_widget, tk_root):
            self.text_widget = text_widget
            self.root = tk_root

        def write(self, message):
            self.root.after(0, self._append, message)

        def _append(self, message):
            try:
                self.text_widget.insert(tk.END, message)
                self.text_widget.see(tk.END)
            except tk.TclError:
                pass

        def flush(self):
            pass

    cff_var = tk.StringVar(value="")
    work_dir_var = tk.StringVar(value=os.path.abspath("work_folder"))
    comp_level_var = tk.IntVar(value=6)

    tk.Label(
        root,
        text="SpellForce 1 & 2 - CFF Localization Tool",
        font=("Arial", 16, "bold"),
        fg="#e0a96d",
        bg=bg_color,
    ).pack(pady=10)

    file_frame = tk.LabelFrame(
        root,
        text=" Path & Compression Settings ",
        font=("Arial", 10, "bold"),
        fg=fg_color,
        bg=bg_color,
        padx=10,
        pady=10,
    )
    file_frame.pack(fill="x", padx=20, pady=5)

    tk.Label(file_frame, text="Localization .cff file:", fg=fg_color, bg=bg_color).grid(
        row=0, column=0, sticky="w"
    )
    tk.Entry(
        file_frame,
        textvariable=cff_var,
        width=52,
        fg=fg_color,
        bg=entry_color,
        insertbackground="white",
    ).grid(row=0, column=1, padx=5)

    def select_cff():
        path = filedialog.askopenfilename(
            filetypes=[("CFF files", "*.cff"), ("All files", "*.*")]
        )
        if path:
            cff_var.set(path)

    tk.Button(
        file_frame,
        text="Browse...",
        command=select_cff,
        fg=fg_color,
        bg=btn_color,
        activebackground="#555555",
    ).grid(row=0, column=2)

    tk.Label(
        file_frame,
        text="Working directory (work_folder):",
        fg=fg_color,
        bg=bg_color,
    ).grid(row=1, column=0, sticky="w", pady=5)
    tk.Entry(
        file_frame,
        textvariable=work_dir_var,
        width=52,
        fg=fg_color,
        bg=entry_color,
        insertbackground="white",
    ).grid(row=1, column=1, padx=5, pady=5)

    def select_work_dir():
        path = filedialog.askdirectory()
        if path:
            work_dir_var.set(path)

    tk.Button(
        file_frame,
        text="Browse...",
        command=select_work_dir,
        fg=fg_color,
        bg=btn_color,
        activebackground="#555555",
    ).grid(row=1, column=2, pady=5)

    tk.Label(
        file_frame,
        text="Zlib Compression Level (0-9):",
        fg=fg_color,
        bg=bg_color,
    ).grid(row=2, column=0, sticky="w", pady=5)
    tk.Scale(
        file_frame,
        from_=0,
        to=9,
        variable=comp_level_var,
        orient="horizontal",
        fg=fg_color,
        bg=bg_color,
        highlightthickness=0,
        showvalue=True,
    ).grid(row=2, column=1, padx=5, sticky="ew")

    log_frame = tk.LabelFrame(
        root,
        text=" Execution Log ",
        font=("Arial", 10, "bold"),
        fg=fg_color,
        bg=bg_color,
        padx=5,
        pady=5,
    )
    log_frame.pack(fill="both", expand=True, padx=20, pady=5)

    text_log = tk.Text(
        log_frame,
        wrap="word",
        height=12,
        fg="#00ff00",
        bg="#121212",
        font=("Consolas", 10),
    )
    text_log.pack(fill="both", expand=True)

    logger = GuiLogger(text_log, root)
    orig_stdout = sys.stdout
    orig_stderr = sys.stderr
    sys.stdout = logger
    sys.stderr = logger

    def on_close():
        sys.stdout = orig_stdout
        sys.stderr = orig_stderr
        root.destroy()

    root.protocol("WM_DELETE_WINDOW", on_close)

    action_frame = tk.Frame(root, bg=bg_color)
    action_frame.pack(pady=10)

    btn_unpack = tk.Button(
        action_frame,
        text="1. Unpack CFF and export TEXT",
        fg=fg_color,
        bg="#1e5f1e",
        font=("Arial", 11, "bold"),
        padx=10,
        pady=5,
        activebackground="#2e7f2e",
    )
    btn_unpack.grid(row=0, column=0, padx=10)

    btn_pack = tk.Button(
        action_frame,
        text="2. Import TEXT and pack CFF",
        fg=fg_color,
        bg="#5f1e1e",
        font=("Arial", 11, "bold"),
        padx=10,
        pady=5,
        activebackground="#7f2e2e",
    )
    btn_pack.grid(row=0, column=1, padx=10)

    def set_buttons_state(state):
        btn_unpack.config(state=state)
        btn_pack.config(state=state)

    def run_thread(target, success_msg):
        set_buttons_state(tk.DISABLED)

        def worker():
            try:
                res = target()
                if res and success_msg:
                    root.after(0, lambda: messagebox.showinfo("Success", success_msg))
            except (OSError, ValueError, struct.error, zlib.error) as e:
                err_msg = str(e)
                print(f"[!] Unhandled exception: {err_msg}")
                root.after(0, lambda msg=err_msg: messagebox.showerror("Error", msg))
            finally:
                root.after(0, lambda: set_buttons_state(tk.NORMAL))

        threading.Thread(target=worker, daemon=True).start()

    def gui_unpack():
        cff = cff_var.get().strip()
        work = work_dir_var.get().strip()
        if not cff or not os.path.isfile(cff):
            messagebox.showerror("Error", "Please select a valid .cff file!")
            return
        if not work:
            messagebox.showerror("Error", "Please select a working directory!")
            return
        text_log.delete("1.0", tk.END)
        run_thread(
            lambda: unpack_all(cff, work),
            "Unpacking and text export completed successfully!",
        )

    def gui_pack():
        work = work_dir_var.get().strip()
        level = comp_level_var.get()
        if not work or not os.path.isdir(work):
            messagebox.showerror("Error", "Working directory does not exist!")
            return
        cff = filedialog.asksaveasfilename(
            defaultextension=".cff",
            filetypes=[("CFF files", "*.cff"), ("All files", "*.*")],
        )
        if not cff:
            return
        text_log.delete("1.0", tk.END)
        run_thread(
            lambda: pack_all(work, cff, level),
            "Text import and CFF packing completed successfully!",
        )

    btn_unpack.config(command=gui_unpack)
    btn_pack.config(command=gui_pack)

    root.mainloop()


# ==============================================================================
# 5. COMMAND LINE INTERFACE (CLI)
# ==============================================================================


def main():
    if len(sys.argv) < 2:
        launch_gui()
        return

    parser = argparse.ArgumentParser(
        description="SpellForce 1 & 2 - CFF Localization Tool"
    )
    subparsers = parser.add_subparsers(dest="command", help="Available commands")

    # export_all
    p_exp_all = subparsers.add_parser(
        "export_all", help="Unpack CFF container and export all text tables"
    )
    p_exp_all.add_argument("cff", help="Path to input .cff file")
    p_exp_all.add_argument("work_dir", help="Target working directory")

    # pack_all
    p_pack_all = subparsers.add_parser(
        "pack_all", help="Import all text tables and pack CFF container"
    )
    p_pack_all.add_argument("work_dir", help="Working directory containing texts_json")
    p_pack_all.add_argument("cff", help="Path to output .cff file")
    p_pack_all.add_argument(
        "-c",
        "--compression",
        type=int,
        default=6,
        choices=range(10),
        help="Zlib compression level (0-9, default: 6)",
    )

    # unpack
    p_unpack = subparsers.add_parser(
        "unpack", help="Unpack CFF container into raw binary chunks only"
    )
    p_unpack.add_argument("cff", help="Path to input .cff file")
    p_unpack.add_argument("out_dir", help="Output directory")

    # pack
    p_pack = subparsers.add_parser("pack", help="Pack CFF container from binary chunks")
    p_pack.add_argument("input_dir", help="Directory with chunks and manifest.json")
    p_pack.add_argument("out_cff", help="Path to output .cff file")
    p_pack.add_argument(
        "-c",
        "--compression",
        type=int,
        default=6,
        choices=range(10),
        help="Zlib compression level (0-9, default: 6)",
    )

    # export
    p_exp = subparsers.add_parser(
        "export", help="Export text from a single binary chunk to JSON"
    )
    p_exp.add_argument("chunk", help="Path to .dat chunk file")
    p_exp.add_argument("json", help="Path to output .json file")

    # import
    p_imp = subparsers.add_parser(
        "import", help="Import text from JSON into a binary chunk"
    )
    p_imp.add_argument("json", help="Path to input .json file")
    p_imp.add_argument("chunk", help="Path to target .dat chunk file")

    args = parser.parse_args()

    if args.command == "export_all":
        unpack_all(args.cff, args.work_dir)
    elif args.command == "pack_all":
        pack_all(args.work_dir, args.cff, args.compression)
    elif args.command == "unpack":
        unpack_cff(args.cff, args.out_dir)
    elif args.command == "pack":
        pack_cff(args.input_dir, args.out_cff, args.compression)
    elif args.command == "export":
        export_text(args.chunk, args.json)
    elif args.command == "import":
        import_text(args.json, args.chunk)
    else:
        parser.print_help()


if __name__ == "__main__":
    main()
