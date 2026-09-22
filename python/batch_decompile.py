#!/usr/bin/env python3
"""
SpellForce 1 — High-Performance Recursive Batch Lua 4.0 Decompiler.

Decompiles compiled Lua 4.0 bytecode (.lua / .luac) using LuaDec while
preserving folder structures and copying plain-text scripts as-is.
"""

import argparse
import os
import shutil
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path


def is_compiled_lua(file_path: Path) -> bool:
    """Check for Lua 4.0 bytecode magic bytes (\\x1bLua)."""
    try:
        with file_path.open("rb") as f:
            header = f.read(4)
            return header.startswith(b"\x1bLua")
    except OSError:
        return False


def extract_error_summary(stdout_text: str, max_tail_lines: int = 5) -> str:
    """Extract key parser errors and tail lines instead of full disassembly dumps."""
    lines = [line.strip() for line in stdout_text.splitlines() if line.strip()]
    if not lines:
        return "No stdout output recorded."

    error_lines = [
        line
        for line in lines
        if "Parser error" in line or "Exit Code:" in line or "error" in line.lower()
    ]
    tail_lines = lines[-max_tail_lines:]

    summary: list[str] = []
    if error_lines:
        summary.extend(error_lines)
    summary.append("--- Last output lines ---")
    summary.extend(tail_lines)
    return "\n".join(summary)


def decompile_single_file(
    src_file: Path,
    src_dir: Path,
    dst_dir: Path,
    luadec_exe: Path,
    timeout: int | None,
    resume: bool,
) -> tuple[Path, str, str]:
    """
    Decompile or copy a single file.

    Returns:
        tuple[Path, str, str]: (relative_path, status, error_details)
    """
    rel_path = src_file.relative_to(src_dir)
    dst_file = dst_dir / rel_path
    dst_file.parent.mkdir(parents=True, exist_ok=True)

    # 1. Skip already processed files if resume mode is enabled
    if resume and dst_file.exists() and dst_file.stat().st_size > 0:
        return rel_path, "SKIPPED", ""

    # 2. Plain text script: copy directly without decompilation
    if not is_compiled_lua(src_file):
        try:
            shutil.copy2(src_file, dst_file)
            return rel_path, "COPIED_TEXT", ""
        except OSError as err:
            return rel_path, "EXCEPTION", f"[COPY ERROR] {rel_path}: {err}\n"

    # 3. Compiled bytecode: run decompiler
    cmd = [str(luadec_exe), str(src_file), str(dst_file)]
    creation_flags = 0x08000000 if sys.platform == "win32" else 0

    try:
        res = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            errors="replace",
            timeout=timeout,
            creationflags=creation_flags,
            check=False,
        )

        if res.returncode == 0 and dst_file.exists():
            return rel_path, "OK", ""

        # Remove broken/empty output file on failure so it won't be skipped later
        if dst_file.exists():
            dst_file.unlink(missing_ok=True)

        err_summary = extract_error_summary(res.stdout)
        stderr_content = res.stderr.strip() if res.stderr else "none"
        err_msg = (
            f"[Exit Code: {res.returncode}] {rel_path.as_posix()}\n"
            f"{err_summary}\n"
            f"STDERR: {stderr_content}\n"
        )
        return rel_path, f"FAILED_{res.returncode}", err_msg

    except subprocess.TimeoutExpired:
        if dst_file.exists():
            dst_file.unlink(missing_ok=True)
        return rel_path, "TIMEOUT", f"[TIMEOUT] {rel_path.as_posix()}\n"

    except (OSError, subprocess.SubprocessError, RuntimeError) as err:
        if dst_file.exists():
            dst_file.unlink(missing_ok=True)
        return (
            rel_path,
            "EXCEPTION",
            f"[EXCEPTION] {rel_path.as_posix()}: {err}\n",
        )


def main() -> None:
    """CLI entry point for batch decompilation."""
    base_dir = Path(__file__).parent.resolve()

    parser = argparse.ArgumentParser(
        description="SpellForce 1 — Parallel Batch Lua 4.0 Decompiler"
    )
    parser.add_argument(
        "-i",
        "--input",
        type=Path,
        default=base_dir / "lua_original",
        help="Source directory containing .lua / compiled files (default: ./lua_original)",
    )
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=base_dir / "lua_test",
        help="Destination directory for decompiled scripts (default: ./lua_test)",
    )
    parser.add_argument(
        "--luadec",
        type=Path,
        default=base_dir / "luadec_32_deb.exe",
        help="Path to luadec executable (default: ./luadec_32_deb.exe)",
    )
    parser.add_argument(
        "-j",
        "--threads",
        type=int,
        default=min(16, (os.cpu_count() or 1) * 2),
        help="Number of worker threads (default: 2x CPU cores, max 16)",
    )
    parser.add_argument(
        "--no-resume",
        action="store_true",
        help="Overwrite already decompiled files instead of skipping them",
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=None,
        help="Timeout per file in seconds (default: None / infinite)",
    )
    parser.add_argument(
        "--log",
        type=Path,
        default=base_dir / "decompile_log.txt",
        help="Path to save report log (default: ./decompile_log.txt)",
    )

    args = parser.parse_args()

    # Validate paths
    if not args.luadec.is_file():
        print(f"[ERROR] Decompiler executable not found at: {args.luadec}")
        sys.exit(1)

    if not args.input.is_dir():
        print(f"[ERROR] Input directory not found: {args.input}")
        sys.exit(1)

    args.output.mkdir(parents=True, exist_ok=True)
    all_files = [p for p in args.input.rglob("*") if p.is_file()]
    total_files = len(all_files)

    if total_files == 0:
        print(f"[WARNING] No files found in source directory: {args.input}")
        sys.exit(0)

    print("=" * 65)
    print("SpellForce 1 — Parallel Lua 4.0 Decompiler")
    print("=" * 65)
    print(f"Total files found:    {total_files}")
    print(f"Input directory:      {args.input}")
    print(f"Output directory:     {args.output}")
    print(f"Decompiler binary:    {args.luadec.name}")
    print(f"Worker threads:       {args.threads}")
    print(f"Resume existing:      {not args.no_resume}")
    print(
        f"Timeout setting:      {args.timeout}s"
        if args.timeout
        else "Timeout setting:      Infinite"
    )
    print(f"Log file path:        {args.log}")
    print("-" * 65)

    stats: dict[str, int] = {
        "OK": 0,
        "SKIPPED": 0,
        "COPIED_TEXT": 0,
        "TIMEOUT": 0,
        "FAILED": 0,
        "EXCEPTION": 0,
    }
    error_types: dict[str, int] = {}
    log_entries: list[str] = []
    start_time = time.time()

    with ThreadPoolExecutor(max_workers=args.threads) as executor:
        futures = [
            executor.submit(
                decompile_single_file,
                file_path,
                args.input,
                args.output,
                args.luadec,
                args.timeout,
                not args.no_resume,
            )
            for file_path in all_files
        ]

        for idx, future in enumerate(as_completed(futures), 1):
            _rel_path, status, msg = future.result()

            if status in stats:
                stats[status] += 1
            elif status.startswith("FAILED_"):
                stats["FAILED"] += 1
                exit_code = status.split("_", 1)[1]
                error_types[exit_code] = error_types.get(exit_code, 0) + 1

            if msg:
                log_entries.append(msg + "-" * 60 + "\n")

            if idx % 25 == 0 or idx == total_files:
                pct = (idx / total_files) * 100
                success_count = stats["OK"] + stats["SKIPPED"] + stats["COPIED_TEXT"]
                print(
                    f"\rProgress: [{idx}/{total_files}] ({pct:5.1f}%) | "
                    f"Success: {success_count} | Failed: {stats['FAILED'] + stats['TIMEOUT'] + stats['EXCEPTION']}",
                    end="",
                    flush=True,
                )

    elapsed_time = time.time() - start_time
    total_successful = stats["OK"] + stats["SKIPPED"] + stats["COPIED_TEXT"]
    success_pct = (total_successful / total_files * 100) if total_files else 0.0

    # Write report log
    with args.log.open("w", encoding="utf-8") as f_log:
        f_log.write("=" * 65 + "\n")
        f_log.write(f"DECOMPILATION REPORT: {time.strftime('%Y-%m-%d %H:%M:%S')}\n")
        f_log.write(f"Total files processed:    {total_files}\n")
        f_log.write(
            f"Total successful:         {total_successful} ({success_pct:.1f}%)\n"
        )
        f_log.write(f"  - Decompiled bytecode:  {stats['OK']}\n")
        f_log.write(f"  - Skipped (existing):   {stats['SKIPPED']}\n")
        f_log.write(f"  - Plain text (copied):  {stats['COPIED_TEXT']}\n")
        f_log.write(f"Total failed:             {stats['FAILED']}\n")
        f_log.write(f"Total timeouts:           {stats['TIMEOUT']}\n")
        f_log.write(f"Total exceptions:         {stats['EXCEPTION']}\n")
        f_log.write(f"Elapsed time:             {elapsed_time:.2f} s\n")
        f_log.write("=" * 65 + "\n")

        if error_types:
            f_log.write("FAILURE BREAKDOWN BY EXIT CODE:\n")
            for code, count in sorted(error_types.items()):
                f_log.write(f"  Exit Code {code}: {count} files\n")
            f_log.write("=" * 65 + "\n\n")

        if log_entries:
            f_log.write("ERROR DETAILS (COMPACT):\n\n")
            f_log.writelines(log_entries)
        else:
            f_log.write("All files decompiled successfully without errors!\n")

    print("\n" + "=" * 65)
    print("DECOMPILATION SUMMARY:")
    print(f"  Total Processed: {total_files}")
    print(f"  Success:         {total_successful} ({success_pct:.1f}%)")
    print(f"    - Decompiled:  {stats['OK']}")
    print(f"    - Skipped:     {stats['SKIPPED']}")
    print(f"    - Plain Text:  {stats['COPIED_TEXT']}")
    print(f"  Failed:          {stats['FAILED']}")
    print(f"  Timeouts:        {stats['TIMEOUT']}")
    print(f"  Exceptions:      {stats['EXCEPTION']}")
    print(f"  Elapsed Time:    {elapsed_time:.2f} s")
    if error_types:
        print("  Exit Code Breakdown:")
        for code, count in sorted(error_types.items()):
            print(f"    Code {code:>3}: {count} files")
    print(f"Detailed log report saved to: {args.log.resolve()}")
    print("=" * 65)


if __name__ == "__main__":
    main()
