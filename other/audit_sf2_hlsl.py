import argparse
import concurrent.futures
import os
import re
import shutil
import subprocess
import time
from collections import defaultdict
from pathlib import Path

# DirectX 9 Shader Model 3.0 profiles for SpellForce 2
STAGE_PROFILES = {
    "VS": "vs_3_0",
    "PS": "ps_3_0",
}


def decode_output(data: bytes) -> str:
    """Safely decodes compiler output handling common Windows encodings."""
    if not data:
        return ""
    for encoding in ("utf-8", "cp1251", "cp866"):
        try:
            return data.decode(encoding)
        except (UnicodeDecodeError, LookupError):
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


def get_shader_stage_and_profile(file_path: Path) -> tuple[str, str]:
    """
    Determines shader stage (vertex or pixel) and target profile
    by DXVK filename prefix (ps_ / vs_) or parent folder name.
    """
    name_lower = file_path.name.lower()
    parent_upper = file_path.parent.name.upper()

    # Check by parent directory name
    if parent_upper in ("VS", "VERTEX"):
        return "VS", "vs_3_0"
    if parent_upper in ("PS", "PIXEL"):
        return "PS", "ps_3_0"

    # Check by DXVK filename prefix (ps_* / vs_*)
    if name_lower.startswith("vs_") or "_vs" in name_lower:
        return "VS", "vs_3_0"
    if name_lower.startswith("ps_") or "_ps" in name_lower:
        return "PS", "ps_3_0"

    # Default to pixel shader for SF2
    return "PS", "ps_3_0"


def compile_single_shader(
    fxc_path: Path,
    hlsl_file: Path,
    entry_point: str = "main",
    forced_profile: str | None = None,
) -> tuple[Path, str, bool, str, list[str]]:
    """Compiles a single HLSL file to NUL to validate syntax and extract error codes."""
    stage, profile = get_shader_stage_and_profile(hlsl_file)
    target_profile = forced_profile or profile

    cmd = [
        str(fxc_path),
        "/nologo",
        "/T",
        target_profile,
        "/E",
        entry_point,
        "/Fo",
        "NUL",
        str(hlsl_file),
    ]

    proc = subprocess.run(
        cmd,
        capture_output=True,
        check=False,
    )

    output = decode_output(proc.stderr) + decode_output(proc.stdout)

    if proc.returncode == 0:
        return hlsl_file, stage, True, "", []

    error_codes = re.findall(r"error\s+(X\d+)", output)
    return hlsl_file, stage, False, output.strip(), error_codes


def normalize_error(output: str) -> str:
    """Strips line numbers and file paths for accurate error deduplication."""
    lines = []
    for line in output.splitlines():
        m = re.search(r"error\s+(X\d+:\s*.+)", line)
        if m:
            lines.append(m.group(1).strip())
    return "\n".join(lines) if lines else output.strip()


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Audit and validate decompiled HLSL shaders for SpellForce 2 (DX9 SM 3.0)"
    )
    parser.add_argument(
        "shaders_dir",
        type=Path,
        nargs="?",
        default=Path("dxvk_shaders/hlsl"),
        help="Path to directory containing .hlsl files (default: dxvk_shaders/hlsl)",
    )
    parser.add_argument(
        "--stage",
        choices=list(STAGE_PROFILES.keys()),
        default=None,
        help="Filter validation to a specific stage (VS or PS)",
    )
    parser.add_argument(
        "--entry-point",
        type=str,
        default="main",
        help="Shader entry point function (default: main)",
    )
    parser.add_argument(
        "--profile",
        type=str,
        default=None,
        help="Force a specific profile (e.g. ps_3_0 or vs_3_0)",
    )
    args = parser.parse_args()

    shaders_dir = args.shaders_dir
    if not shaders_dir.exists():
        print(f"[-] Directory not found: {shaders_dir}")
        return

    fxc = find_fxc()
    if not fxc:
        print(
            "[-] Error: fxc.exe not found! Please install the Windows SDK or DirectX SDK."
        )
        return

    all_files = sorted(shaders_dir.rglob("*.hlsl"))
    if args.stage:
        hlsl_files = [
            f for f in all_files if get_shader_stage_and_profile(f)[0] == args.stage
        ]
    else:
        hlsl_files = all_files

    total_files = len(hlsl_files)
    if total_files == 0:
        print(f"[-] No .hlsl files found to audit in '{shaders_dir}'!")
        return

    num_workers = os.cpu_count() or 4
    print(f"[*] FXC Compiler:     {fxc}")
    print(f"[*] Target Directory: {shaders_dir}")
    if args.stage:
        print(f"[*] Stage Filter:     {args.stage}")
    print(
        f"[*] Found {total_files:,} .hlsl shader(s). Running audit across {num_workers} CPU workers..."
    )

    start_time = time.time()
    passed_files: list[Path] = []
    failed_files: list[tuple[Path, str, list[str]]] = []
    error_clusters: dict[str, list[tuple[Path, str]]] = defaultdict(list)

    stage_stats = {s: {"total": 0, "passed": 0, "failed": 0} for s in STAGE_PROFILES}

    with concurrent.futures.ThreadPoolExecutor(max_workers=num_workers) as executor:
        futures = {
            executor.submit(
                compile_single_shader, fxc, f, args.entry_point, args.profile
            ): f
            for f in hlsl_files
        }

        for completed, future in enumerate(concurrent.futures.as_completed(futures), 1):
            hlsl_path, stage, is_success, err_msg, err_codes = future.result()

            if stage in stage_stats:
                stage_stats[stage]["total"] += 1

            if is_success:
                passed_files.append(hlsl_path)
                if stage in stage_stats:
                    stage_stats[stage]["passed"] += 1
            else:
                failed_files.append((hlsl_path, err_msg, err_codes))
                if stage in stage_stats:
                    stage_stats[stage]["failed"] += 1
                sig = normalize_error(err_msg)
                error_clusters[sig].append((hlsl_path, err_msg))

            if completed % 100 == 0 or completed == total_files:
                pct = (completed / total_files) * 100
                print(
                    f"    Progress: {completed:,}/{total_files:,} ({pct:.1f}%) | "
                    f"Passed: {len(passed_files):,} | Failed: {len(failed_files):,}"
                )

    elapsed = time.time() - start_time
    pass_rate = (len(passed_files) / total_files) * 100 if total_files else 0

    print("\n" + "=" * 65)
    print("           HLSL AUDIT REPORT (SPELLFORCE 2)")
    print("=" * 65)
    print(f"Total checked:         {total_files:,}")
    print(f"Passed cleanly:        {len(passed_files):,} ({pass_rate:.1f}%)")
    print(f"Failed compilation:    {len(failed_files):,} ({100 - pass_rate:.1f}%)")
    print(f"Unique Error Patterns: {len(error_clusters):,}")
    print(f"Elapsed time:          {elapsed:.2f} seconds")
    print("-" * 65)
    print("STAGE BREAKDOWN (DX9 SM 3.0):")
    print("  Stage |  Total  |  Passed |  Failed |  Pass Rate")
    print("  ------+---------+---------+---------+-----------")
    for st, stats in stage_stats.items():
        if stats["total"] > 0:
            st_rate = (stats["passed"] / stats["total"]) * 100
            print(
                f"   {st:<4} | {stats['total']:>7,} | {stats['passed']:>7,} | "
                f"{stats['failed']:>7,} | {st_rate:>8.1f}%"
            )
    print("=" * 65)

    log_file = Path("hlsl_audit_errors.log")
    with open(log_file, "w", encoding="utf-8") as f:
        f.write("=== DEDUPLICATED HLSL COMPILATION ERROR LOG (SF2) ===\n")
        f.write(f"Total Files: {total_files:,} | Failed: {len(failed_files):,} | ")
        f.write(f"Unique Patterns: {len(error_clusters)}\n\n")

        sorted_clusters = sorted(
            error_clusters.items(), key=lambda item: len(item[1]), reverse=True
        )

        for idx, (sig, occurrences) in enumerate(sorted_clusters, 1):
            count = len(occurrences)
            f.write("=" * 70 + "\n")
            f.write(
                f"ERROR PATTERN #{idx}: {count:,} file(s) affected ({count / total_files * 100:.1f}%)\n"
            )
            f.write("-" * 70 + "\n")
            f.write("ERROR MESSAGE:\n")
            f.write(f"{sig}\n\n")
            f.write("AFFECTED FILES (first 10 samples):\n")
            f.writelines(f"  - {p.as_posix()}\n" for p, _ in occurrences[:10])
            if count > 10:
                f.write(f"  ... and {count - 10} more files.\n")
            f.write("\n")

    print(f"\n[+] Detailed error log saved to: {log_file.resolve()}")

    if sorted_clusters:
        print("\n--- Top Error Patterns in Console (Top 3) ---")
        for idx, (sig, occurrences) in enumerate(sorted_clusters[:3], 1):
            first_line = sig.splitlines()[0] if sig else "Unknown error"
            print(f"  [#{idx}] ({len(occurrences):,} files): {first_line}")


if __name__ == "__main__":
    main()
