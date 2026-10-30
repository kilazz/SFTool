import argparse
import concurrent.futures
import os
import re
import shutil
import subprocess
from pathlib import Path

# Regular expression to match D3D9 shader profile declarations
ASM_PROFILE_REGEX = re.compile(
    r"^\s*([pv]s_[1-3]_[0-9abx])\b", re.IGNORECASE | re.MULTILINE
)


def decode_output(data: bytes) -> str:
    """Decodes subprocess bytes output trying Windows/UTF encodings safely."""
    if not data:
        return ""
    for encoding in ("utf-8", "cp1251", "cp866"):
        try:
            return data.decode(encoding)
        except UnicodeDecodeError:
            continue
    return data.decode("ascii", errors="replace")


def find_fxc() -> Path | None:
    """Locates the Microsoft FXC (DirectX Shader Compiler) executable."""
    which_fxc = shutil.which("fxc") or shutil.which("fxc.exe")
    if which_fxc:
        return Path(which_fxc)

    search_roots = [
        Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")),
        Path(os.environ.get("ProgramFiles", r"C:\Program Files")),
    ]

    patterns = [
        "Windows Kits/10/bin/*/x64/fxc.exe",
        "Windows Kits/10/bin/*/x86/fxc.exe",
        "Windows Kits/8.1/bin/x64/fxc.exe",
        "Windows Kits/8.1/bin/x86/fxc.exe",
        "Microsoft DirectX SDK (June 2010)/Utilities/bin/x64/fxc.exe",
        "Microsoft DirectX SDK (June 2010)/Utilities/bin/x86/fxc.exe",
    ]

    for root in search_roots:
        if not root.exists():
            continue
        for pattern in patterns:
            candidates = sorted(root.glob(pattern), reverse=True)
            for candidate in candidates:
                if candidate.is_file():
                    return candidate

    return None


def find_hlsl_decompiler() -> Path | None:
    """Locates the HlslDecompiler executable in PATH or script directory."""
    which_tool = shutil.which("HlslDecompiler") or shutil.which("HlslDecompiler.exe")
    if which_tool:
        return Path(which_tool)

    script_dir = Path(__file__).resolve().parent
    local_candidates = [
        script_dir / "HlslDecompiler.exe",
        script_dir / "tools" / "HlslDecompiler.exe",
        script_dir / "bin" / "HlslDecompiler.exe",
    ]
    for candidate in local_candidates:
        if candidate.is_file():
            return candidate

    return None


def detect_shader_profile(src_file: Path) -> str:
    """Detects the shader profile from .asm/.hlsl contents or filename prefix."""
    if src_file.suffix.lower() == ".asm":
        try:
            with open(src_file, "r", encoding="utf-8", errors="ignore") as f:
                for _ in range(60):
                    line = f.readline()
                    if not line:
                        break
                    clean_line = line.split("//")[0].split(";")[0].strip()
                    match = ASM_PROFILE_REGEX.match(clean_line)
                    if match:
                        return match.group(1).lower()
        except OSError:
            pass

    # Fallback by name prefix
    name_lower = src_file.name.lower()
    if name_lower.startswith("vs_") or "_vs" in name_lower:
        return "vs_3_0"
    return "ps_3_0"


def disassemble_to_asm(fxc: Path, bin_file: Path, out_asm: Path) -> tuple[bool, str]:
    """Disassembles a .bin shader file into .asm text using FXC."""
    cmd = [str(fxc), "/dumpbin", "/nologo", str(bin_file)]
    proc = subprocess.run(cmd, capture_output=True, check=False)

    if proc.returncode == 0:
        out_asm.write_bytes(proc.stdout)
        return True, ""
    return False, decode_output(proc.stderr or proc.stdout)


def decompile_to_hlsl(
    decompiler: Path, bin_file: Path, out_hlsl: Path
) -> tuple[bool, str]:
    """Decompiles a .bin shader file into .hlsl code using HlslDecompiler."""
    # HlslDecompiler [options] <input> -o <output>
    cmd = [str(decompiler), str(bin_file), "-o", str(out_hlsl)]
    proc = subprocess.run(cmd, capture_output=True, check=False)

    if proc.returncode == 0 and out_hlsl.exists():
        return True, ""
    return False, decode_output(proc.stderr or proc.stdout)


def compile_single(
    fxc: Path,
    src_file: Path,
    out_bin: Path,
    entry_point: str = "main",
    forced_profile: str | None = None,
) -> tuple[bool, str]:
    """Compiles .asm or .hlsl back into bytecode .bin."""
    profile = forced_profile or detect_shader_profile(src_file)
    is_hlsl = src_file.suffix.lower() == ".hlsl"

    cmd = [
        str(fxc),
        "/nologo",
        "/T",
        profile,
        "/Fo",
        str(out_bin),
    ]

    if is_hlsl:
        cmd.extend(["/E", entry_point])

    cmd.append(str(src_file))

    proc = subprocess.run(cmd, capture_output=True, check=False)
    if proc.returncode == 0:
        return True, ""

    error_msg = decode_output(proc.stderr) or decode_output(proc.stdout)
    return False, f"Profile: {profile} | Error:\n{error_msg}"


def main():
    parser = argparse.ArgumentParser(
        description="DXVK Shader Automation Tool (D3D9 / SpellForce 2)"
    )
    parser.add_argument(
        "--threads",
        type=int,
        default=os.cpu_count() or 4,
        help="Number of concurrent worker threads (default: CPU cores)",
    )

    subparsers = parser.add_subparsers(dest="command", required=True)

    # Disassembly / Decompilation subcommand
    p_disasm = subparsers.add_parser(
        "disasm", help="Convert dumped .bin shaders into readable HLSL or ASM"
    )
    p_disasm.add_argument(
        "--dump-dir",
        type=Path,
        default=Path("dxvk_shaders/dump"),
        help="Directory containing dumped binary shaders",
    )
    p_disasm.add_argument(
        "--out-dir",
        type=Path,
        default=None,
        help="Output directory (defaults to dxvk_shaders/hlsl or dxvk_shaders/asm)",
    )
    p_disasm.add_argument(
        "--format",
        choices=["hlsl", "asm"],
        default="hlsl",
        help="Target output format: 'hlsl' (via HlslDecompiler) or 'asm' (via FXC). Default: hlsl",
    )
    p_disasm.add_argument(
        "--decompiler-path",
        type=Path,
        default=None,
        help="Explicit path to HlslDecompiler.exe",
    )

    # Compilation subcommand
    p_compile = subparsers.add_parser(
        "compile", help="Compile modified .asm/.hlsl files into override .bin shaders"
    )
    p_compile.add_argument(
        "--src-dir",
        type=Path,
        default=None,
        help="Source directory containing .asm or .hlsl files (default: auto-detected)",
    )
    p_compile.add_argument(
        "--override-dir",
        type=Path,
        default=Path("dxvk_shaders/override"),
        help="Output directory for compiled override shaders",
    )
    p_compile.add_argument(
        "--entry-point",
        type=str,
        default="main",
        help="Entry point function for .hlsl shaders (default: main)",
    )
    p_compile.add_argument(
        "--profile",
        type=str,
        default=None,
        help="Force specific shader profile (e.g. ps_3_0, vs_3_0). Auto-detected if omitted.",
    )

    args = parser.parse_args()
    fxc = find_fxc()

    if not fxc:
        print(
            "[-] Error: fxc.exe not found! Please install the Windows SDK or DirectX SDK."
        )
        return

    print(f"[*] FXC compiler: {fxc}")

    # ==========================
    # DISASM / DECOMPILE COMMAND
    # ==========================
    if args.command == "disasm":
        if not args.dump_dir.exists():
            print(
                f"[-] Directory '{args.dump_dir}' not found. Run the game with DXVK shader dumping enabled."
            )
            return

        bin_files = list(args.dump_dir.glob("*.bin"))
        if not bin_files:
            print("[-] No .bin files found in the dump directory.")
            return

        target_format = args.format
        decompiler = None

        if target_format == "hlsl":
            decompiler = args.decompiler_path or find_hlsl_decompiler()
            if not decompiler:
                print(
                    "[-] Warning: HlslDecompiler.exe was not found!\n"
                    "    Download it from: https://github.com/AndresTraks/HlslDecompiler/releases\n"
                    "    and place 'HlslDecompiler.exe' next to this script or in PATH.\n"
                    "[*] Falling back to ASM disassembly via FXC..."
                )
                target_format = "asm"

        # Determine output folder
        out_dir = args.out_dir or Path(f"dxvk_shaders/{target_format}")
        out_dir.mkdir(parents=True, exist_ok=True)

        ext = f".{target_format}"
        print(
            f"[*] Processing {len(bin_files)} shaders to [{target_format.upper()}] using {args.threads} threads..."
        )

        success, failed = 0, 0
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.threads) as pool:
            futures = {}
            for bf in bin_files:
                out_file = out_dir / bf.with_suffix(ext).name
                if target_format == "hlsl" and decompiler:
                    futures[
                        pool.submit(decompile_to_hlsl, decompiler, bf, out_file)
                    ] = bf
                else:
                    futures[pool.submit(disassemble_to_asm, fxc, bf, out_file)] = bf

            for future in concurrent.futures.as_completed(futures):
                ok, err = future.result()
                if ok:
                    success += 1
                else:
                    failed += 1
                    bf = futures[future]
                    if err:
                        print(f"[-] Error processing {bf.name}:\n{err}")

        print(f"[+] Done! Succeeded: {success}, Failed: {failed}")
        print(f"[+] Output files saved to: {out_dir}")

    # ==========================
    # COMPILE COMMAND
    # ==========================
    elif args.command == "compile":
        # Resolve source folder if not provided
        src_dir = args.src_dir
        if not src_dir:
            if Path("dxvk_shaders/hlsl").exists():
                src_dir = Path("dxvk_shaders/hlsl")
            else:
                src_dir = Path("dxvk_shaders/asm")

        if not src_dir.exists():
            print(f"[-] Source directory '{src_dir}' not found.")
            return

        src_files = [
            f
            for f in src_dir.iterdir()
            if f.is_file() and f.suffix.lower() in (".asm", ".hlsl")
        ]

        if not src_files:
            print(f"[-] No .asm or .hlsl files found in '{src_dir}'.")
            return

        args.override_dir.mkdir(parents=True, exist_ok=True)
        print(
            f"[*] Compiling {len(src_files)} shaders from '{src_dir}' with {args.threads} workers..."
        )

        success, failed = 0, 0
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.threads) as pool:
            futures = {}
            for sf in src_files:
                out_bin = args.override_dir / sf.with_suffix(".bin").name
                futures[
                    pool.submit(
                        compile_single,
                        fxc,
                        sf,
                        out_bin,
                        args.entry_point,
                        args.profile,
                    )
                ] = sf

            for future in concurrent.futures.as_completed(futures):
                sf = futures[future]
                ok, err = future.result()
                if ok:
                    success += 1
                else:
                    failed += 1
                    print(f"[-] Compilation error for {sf.name}:\n{err}\n")

        print(f"[+] Done! Succeeded: {success}, Failed: {failed}")
        print(f"[+] Overrides saved to: {args.override_dir}")


if __name__ == "__main__":
    main()
