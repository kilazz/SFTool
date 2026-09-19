import argparse
import json
import os
import re
import shutil
import struct
import sys
import threading
import zlib


def unpack_cff(input_file, out_dir):
    """Unpacks compressed CFF container into separate binary chunk files."""
    print(f"[*] Unpacking CFF: {input_file} -> {out_dir}")
    os.makedirs(out_dir, exist_ok=True)

    with open(input_file, "rb") as f:
        data = f.read()

    if len(data) < 20 or data[0:4] != b"\x12\xdd\x72\xdd":
        print("[!] Error: Invalid CFF signature or corrupted header!")
        return False

    with open(os.path.join(out_dir, "header.bin"), "wb") as f:
        f.write(data[0:20])

    manifest = {"chunks": []}
    offset = 20
    chunk_idx = 0

    while offset + 16 <= len(data):
        c_id, flag1, comp_size, flag2, _uncomp_size = struct.unpack_from(
            "<IHIHI", data, offset
        )
        offset += 16

        if offset + comp_size > len(data):
            print(f"[!] Error: Chunk {chunk_idx} data exceeds file boundaries!")
            break

        comp_data = data[offset : offset + comp_size]
        offset += comp_size

        try:
            uncomp_data = zlib.decompress(comp_data)
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
        except (zlib.error, OSError) as e:
            print(f"[!] Error decompressing chunk {chunk_idx}: {e}")

        chunk_idx += 1

    with open(os.path.join(out_dir, "manifest.json"), "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=4)

    print(f"[+] Unpacking completed! Extracted {chunk_idx} chunks.")
    return True


def pack_cff(input_dir, out_file, comp_level=6):
    """Packs binary chunks back into a compressed CFF container."""
    print(
        f"[*] Packing CFF (compression level: {comp_level}): {input_dir} -> {out_file}"
    )
    manifest_path = os.path.join(input_dir, "manifest.json")
    header_path = os.path.join(input_dir, "header.bin")

    if not os.path.exists(manifest_path) or not os.path.exists(header_path):
        print("[!] Error: manifest.json or header.bin not found!")
        return False

    with open(manifest_path, "r", encoding="utf-8") as f:
        manifest = json.load(f)

    out_dir = os.path.dirname(os.path.abspath(out_file))
    if out_dir:
        os.makedirs(out_dir, exist_ok=True)

    with open(out_file, "wb") as out:
        with open(header_path, "rb") as hf:
            out.write(hf.read())

        for chunk in manifest.get("chunks", []):
            chunk_path = os.path.join(input_dir, chunk["file"])
            if not os.path.exists(chunk_path):
                print(f"[!] Error: Chunk file {chunk_path} missing!")
                return False

            with open(chunk_path, "rb") as cf:
                uncomp_data = cf.read()

            comp_data = zlib.compress(uncomp_data, level=comp_level)
            out.write(
                struct.pack(
                    "<IHIHI",
                    chunk["id"],
                    chunk["flag1"],
                    len(comp_data),
                    chunk["flag2"],
                    len(uncomp_data),
                )
            )
            out.write(comp_data)

    print("[+] Packing completed successfully!")
    return True


def detect_format(data):
    """Detects binary chunk format and its internal parameters."""
    if len(data) < 4:
        return "unknown", 0, 0

    count = struct.unpack_from("<I", data, 0)[0]
    if count == 0:
        return ("empty", 0, 0) if len(data) == 4 else ("unknown", 0, 0)

    # 1. Format A (string_table): Flag(1) + KeyLen(4) + Key + TextLen(4) + Text(UTF-16)
    try:
        offset = 4
        success = True
        for _ in range(count):
            if offset + 5 > len(data):
                success = False
                break
            offset += 1  # 0x01 flag
            key_len = struct.unpack_from("<I", data, offset)[0]
            offset += 4 + key_len
            if offset + 4 > len(data):
                success = False
                break
            text_len = struct.unpack_from("<I", data, offset)[0]
            offset += 4 + text_len * 2
            if offset > len(data):
                success = False
                break

        if success and offset == len(data):
            return "string_table", 0, 0
    except struct.error:
        pass

    # 2. Format B (table_based): ID(4) + E extra bytes + N strings(UTF-16)
    for E in range(33):
        for N in range(1, 11):
            offset = 4
            success = True
            for _ in range(count):
                if offset + 4 + E > len(data):
                    success = False
                    break
                offset += 4 + E
                for _ in range(N):
                    if offset + 4 > len(data):
                        success = False
                        break
                    str_len = struct.unpack_from("<I", data, offset)[0]
                    offset += 4 + str_len * 2
                    if offset > len(data):
                        success = False
                        break

            if success and offset == len(data):
                return "table_based", N, E

    return "unknown", 0, 0


def export_text(chunk_path, json_path):
    """Exports texts from a binary chunk into JSON."""
    if not os.path.exists(chunk_path):
        print(f"[!] Error: Chunk file not found: {chunk_path}")
        return False

    with open(chunk_path, "rb") as f:
        data = f.read()

    if len(data) < 4:
        print(f"[!] Error: Chunk {chunk_path} is too small!")
        return False

    count = struct.unpack_from("<I", data, 0)[0]
    fmt, num_strings, extra_bytes = detect_format(data)

    if fmt == "unknown":
        print(
            f"[-] Skipping {os.path.basename(chunk_path)}: non-text or unsupported format."
        )
        return False

    if fmt == "empty":
        print(f"[*] Chunk {os.path.basename(chunk_path)} is empty.")
        with open(json_path, "w", encoding="utf-8") as f:
            json.dump({"_metadata": {"format": "empty"}}, f, indent=4)
        return True

    print(
        f"[*] Exporting text: {os.path.basename(chunk_path)} -> {os.path.basename(json_path)}"
    )
    print(
        f"    [Format: {fmt} | Entries: {count} | Strings/Entry: {num_strings} | ExtraBytes: {extra_bytes}]"
    )

    offset = 4
    texts = {}

    if fmt == "string_table":
        out_dict = {"_metadata": {"format": "string_table"}}
        for _ in range(count):
            if offset >= len(data):
                break
            offset += 1  # Hidden flag
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
    else:  # table_based
        out_dict = {
            "_metadata": {
                "format": "table_based",
                "num_strings": num_strings,
                "extra_bytes": extra_bytes,
            }
        }
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

                texts[f"{i}_{id_val}_{extra_hex}_str{s}"] = text

    out_dict.update(texts)
    with open(json_path, "w", encoding="utf-8") as f:
        json.dump(out_dict, f, indent=4, ensure_ascii=False)

    print(f"[+] Exported {len(texts)} entries!")
    return True


def import_text(json_path, chunk_path):
    """Imports texts from JSON back into binary chunk."""
    if not os.path.exists(json_path):
        print(f"[!] Error: JSON file not found: {json_path}")
        return False

    with open(json_path, "r", encoding="utf-8") as f:
        texts = json.load(f)

    meta = texts.pop("_metadata", None)
    is_table_based = False
    num_strings = 0

    if meta and isinstance(meta, dict):
        if meta.get("format") == "table_based":
            is_table_based = True
            num_strings = meta.get("num_strings", 0)
    else:
        # Fallback detection for legacy JSON files
        first_key = next(iter(texts.keys()), None)
        if first_key is not None and "_" in first_key:
            parts = first_key.split("_", 3)
            if (
                len(parts) == 4
                and parts[0].isdigit()
                and parts[1].isdigit()
                and parts[3].startswith("str")
            ):
                is_table_based = True

    if is_table_based and num_strings == 0:
        max_str_idx = 0
        for key in texts:
            k_parts = key.split("_", 3)
            if len(k_parts) == 4 and k_parts[3].startswith("str"):
                try:
                    str_idx = int(k_parts[3][3:])
                    max_str_idx = max(max_str_idx, str_idx)
                except ValueError:
                    pass
        num_strings = max_str_idx + 1

    # Preserve original binary chunk before overwriting
    if os.path.exists(chunk_path):
        bak_path = chunk_path + ".bak"
        if not os.path.exists(bak_path):
            shutil.copy2(chunk_path, bak_path)

    with open(chunk_path, "wb") as f:
        if not is_table_based:
            # String table import
            f.write(struct.pack("<I", len(texts)))
            for key, text in texts.items():
                f.write(b"\x01")  # Flag

                kb = key.encode("utf-8")
                f.write(struct.pack("<I", len(kb)))
                f.write(kb)

                tb = text.encode("utf-16-le")
                f.write(struct.pack("<I", len(tb) // 2))
                f.write(tb)
        else:
            # Table-based import
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

            sorted_indices = sorted(entries.keys())
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


def unpack_all(cff_path, work_dir):
    """Full unpack: unpacks CFF and exports all detected text tables to JSON."""
    print(f"[*] Starting full unpack cycle: {cff_path} -> {work_dir}")
    if not unpack_cff(cff_path, work_dir):
        return False

    json_dir = os.path.join(work_dir, "texts_json")
    os.makedirs(json_dir, exist_ok=True)

    exported_count = 0
    for file in sorted(os.listdir(work_dir)):
        match = re.match(r"^chunk_(\d+)\.dat$", file)
        if not match:
            continue

        chunk_idx = int(match.group(1))
        chunk_path = os.path.join(work_dir, file)
        desc_name = f"chunk_{chunk_idx}_strings.json"
        json_path = os.path.join(json_dir, desc_name)

        if export_text(chunk_path, json_path):
            exported_count += 1

    print(
        f"[+] All done! Successfully exported {exported_count} text tables to {json_dir}"
    )
    return True


def pack_all(work_dir, cff_path, comp_level=6):
    """Full pack: imports all JSON texts back and builds the CFF archive."""
    print(
        f"[*] Starting full pack cycle (compression: {comp_level}): {work_dir} -> {cff_path}"
    )
    json_dir = os.path.join(work_dir, "texts_json")

    if not os.path.exists(json_dir):
        print("[!] Error: Directory texts_json not found!")
        return False

    imported_count = 0
    for file in sorted(os.listdir(json_dir)):
        if file.endswith(".json"):
            match = re.match(r"^chunk_(\d+)", file)
            if not match:
                continue
            chunk_idx = int(match.group(1))
            chunk_file = f"chunk_{chunk_idx}.dat"
            chunk_path = os.path.join(work_dir, chunk_file)
            json_path = os.path.join(json_dir, file)

            if import_text(json_path, chunk_path):
                imported_count += 1

    print(f"[+] Re-imported {imported_count} JSON tables.")
    if pack_cff(work_dir, cff_path, comp_level):
        print("[+] Localized CFF archive successfully created!")
        return True
    return False


def launch_gui():
    """Launches the graphical user interface."""
    import tkinter as tk
    from tkinter import filedialog, messagebox

    root = tk.Tk()
    root.title("SpellForce 2 CFF Localization Tool")
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
        text="SpellForce 2 - CFF Localization Tool",
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
            except Exception as e:  # noqa: BLE001
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


def main():
    if len(sys.argv) < 2:
        launch_gui()
        return

    parser = argparse.ArgumentParser(
        description="SpellForce 2 CFF Localization and Modding Tool"
    )
    subparsers = parser.add_subparsers(dest="command", help="Available commands")

    # export_all
    p_exp_all = subparsers.add_parser(
        "export_all", help="Unpack CFF and export all text tables"
    )
    p_exp_all.add_argument("cff", help="Path to input .cff file")
    p_exp_all.add_argument("work_dir", help="Target working directory")

    # pack_all
    p_pack_all = subparsers.add_parser(
        "pack_all", help="Import all text tables and pack CFF"
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
        "unpack", help="Unpack CFF into binary chunks only"
    )
    p_unpack.add_argument("cff", help="Path to input .cff file")
    p_unpack.add_argument("out_dir", help="Output directory")

    # pack
    p_pack = subparsers.add_parser("pack", help="Pack CFF from binary chunks")
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
        "export", help="Export text from a single chunk to JSON"
    )
    p_exp.add_argument("chunk", help="Path to .dat chunk file")
    p_exp.add_argument("json", help="Path to output .json file")

    # import
    p_imp = subparsers.add_parser("import", help="Import text from JSON into a chunk")
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
