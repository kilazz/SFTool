import argparse
import logging
import os
import re
import shutil
import subprocess
import sys
import time
from collections.abc import Callable
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass, field
from pathlib import Path

# Configure default logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    datefmt="%H:%M:%S",
)
logger = logging.getLogger("LuaToolkit")


@dataclass
class LuaFileAnalysisResult:
    """Represents the syntax check result for a single Lua file."""

    relative_path: str
    is_syntax_ok: bool
    message: str
    encoding: str = "utf-8"
    status: str = "ok"


@dataclass
class LuaBatchResult:
    """Represents aggregated results of a batch operation."""

    total_files: int = 0
    passed_files: int = 0
    failed_files: int = 0
    elapsed_seconds: float = 0.0
    failures: list[LuaFileAnalysisResult] = field(default_factory=list)


# Lexer token patterns for Lua 4.0
LUA_TOKEN_REGEX = re.compile(
    r"""
    (?P<COMMENT_MULTI>--\[\[[\s\S]*?\]\])
    |(?P<COMMENT_SINGLE>--[^\n]*)
    |(?P<STRING_MULTI>\[\[[\s\S]*?\]\])
    |(?P<STRING_DOUBLE>"(?:\\.|[^"\\])*")
    |(?P<STRING_SINGLE>'(?:\\.|[^'\\])*')
    |(?P<NEWLINE>\n)
    |(?P<WHITESPACE>[^\S\n]+)
    |(?P<KEYWORD>\b(and|or|not|function|then|do|repeat|else|elseif|end|until|return|local|for|while|if|in|break)\b)
    |(?P<VARARG>\.\.\.)
    |(?P<OP_MULTI>==|~=|<=|>=|\.\.)
    |(?P<IDENT>[a-zA-Z_][a-zA-Z0-9_]*)
    |(?P<NUMBER>\b\d+(\.\d+)?([eE][+-]?\d+)?\b|\.\d+([eE][+-]?\d+)?\b)
    |(?P<SYMBOL>[{}()[\]=+\-*/^,;:.%<>])
    |(?P<MISC>.)
    """,
    re.VERBOSE,
)


def format_lua_source(source_text: str, indent_unit: str = "\t") -> str:
    """
    Robust Lua 4.0 Pretty-Printer & Indenter matching original SpellForce scripts.
    Converts 'Name = function' -> 'function Name', collapses empty tables,
    and formats blocks with tabs.
    """
    # 1. Normalize syntactic sugar: Name = function(...) -> function Name(...)
    source_text = re.sub(
        r"^([ \t]*)([a-zA-Z_][a-zA-Z0-9_.:]*)\s*=\s*function\s*\(",
        r"\1function \2(",
        source_text,
        flags=re.MULTILINE,
    )

    # 2. Collapse empty tables: {\s*\n\s*} -> {}
    source_text = re.sub(r"\{\s*\n\s*\}", r"{}", source_text)

    lines: list[list[tuple[str, str]]] = [[]]

    # Tokenize input text
    for match in LUA_TOKEN_REGEX.finditer(source_text):
        kind = match.lastgroup or "MISC"
        val = match.group()

        if kind == "NEWLINE":
            lines.append([])
        elif kind != "WHITESPACE":
            lines[-1].append((kind, val))

    formatted_lines: list[str] = []
    current_indent = 0

    open_keywords = {"then", "do", "repeat"}
    close_keywords = {"end", "until"}
    middle_keywords = {"else", "elseif"}

    for token_list in lines:
        if not token_list:
            if formatted_lines and formatted_lines[-1] != "":
                formatted_lines.append("")
            continue

        code_tokens = [
            (kind, val)
            for kind, val in token_list
            if kind
            not in (
                "COMMENT_SINGLE",
                "COMMENT_MULTI",
                "STRING_DOUBLE",
                "STRING_SINGLE",
                "STRING_MULTI",
            )
        ]

        open_count = 0
        close_count = 0
        starts_with_unindent = False
        first_val = code_tokens[0][1] if code_tokens else ""

        if (
            first_val in close_keywords
            or first_val in middle_keywords
            or first_val == "}"
        ):
            starts_with_unindent = True

        for _, val in code_tokens:
            if val in open_keywords or val == "{":
                open_count += 1
            elif val in close_keywords or val == "}":
                close_count += 1
            elif val == "function":
                open_count += 1

        # 'elseif ... then' continues an existing if-branch and must not add indentation
        if first_val == "elseif" and open_count > 0:
            open_count -= 1

        effective_indent = current_indent
        if starts_with_unindent:
            effective_indent = max(0, current_indent - 1)

        line_str = _render_line_tokens(token_list)
        formatted_lines.append((indent_unit * effective_indent) + line_str)

        current_indent = max(0, current_indent + open_count - close_count)

    while formatted_lines and not formatted_lines[-1]:
        formatted_lines.pop()

    return "\n".join(formatted_lines) + "\n"


def _render_line_tokens(tokens: list[tuple[str, str]]) -> str:
    """Reassembles tokens on a single line with standard spacing around operators and keywords."""
    if not tokens:
        return ""

    out: list[str] = []
    prev_kind = ""
    prev_val = ""

    spaced_ops = {"=", "==", "~=", "<=", ">=", "+", "-", "*", "/", "^", ".."}
    word_kinds = {"IDENT", "KEYWORD", "NUMBER", "VARARG"}
    string_kinds = {"STRING_DOUBLE", "STRING_SINGLE", "STRING_MULTI"}

    for kind, val in tokens:
        if val in (",", ";", ")", "]", "}") or prev_val in ("(", "[", "{"):
            pass
        elif (
            val in spaced_ops
            or prev_val in spaced_ops
            or prev_val in (",", ";")
            or (prev_kind in word_kinds and kind in word_kinds)
            or (prev_val in (")", "]", "}") and kind in ("KEYWORD", "IDENT"))
            or (prev_kind in string_kinds and kind in ("KEYWORD", "IDENT"))
            or (prev_kind in word_kinds and kind in string_kinds)
            or prev_kind == "COMMENT_SINGLE"
        ):
            out.append(" ")

        out.append(val)
        prev_kind = kind
        prev_val = val

    return "".join(out).strip()


class LuaToolkit:
    """Comprehensive toolkit for syntax validation, batch compilation, and formatting of Lua 4.0 scripts."""

    def __init__(
        self,
        root: Path,
        luac_path: Path | None = None,
        progress_callback: Callable[[int, int], None] | None = None,
    ) -> None:
        self.root = root.resolve()
        self.progress_callback = progress_callback
        self.luac = self._resolve_luac(luac_path)

    @staticmethod
    def _resolve_luac(custom_path: Path | None) -> Path:
        """Find the official Lua 4.0 compiler executable."""
        candidates = [
            custom_path,
            Path(__file__).parent / "luac4.exe",
            Path("luac4.exe"),
            Path("luac.exe"),
        ]
        for candidate in candidates:
            if candidate and candidate.is_file():
                return candidate.resolve()

        which_path = shutil.which("luac4") or shutil.which("luac")
        if which_path:
            return Path(which_path).resolve()

        return Path("luac4.exe")

    def _run_cmd(self, cmd: list[str], timeout: int = 30) -> tuple[bool, str]:
        """Execute a sub-command with window suppression on Windows."""
        creation_flags = 0x08000000 if sys.platform == "win32" else 0

        try:
            p = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                errors="replace",
                timeout=timeout,
                creationflags=creation_flags,
                check=False,
            )
            output = (p.stderr or p.stdout or "").strip()
            return p.returncode == 0, output

        except subprocess.TimeoutExpired:
            return False, "Command timed out."
        except (OSError, subprocess.SubprocessError) as err:
            return False, f"Execution error: {err}"

    def check_file_syntax(self, file_path: Path) -> LuaFileAnalysisResult:
        """Check syntax of a single Lua file using 'luac4 -p'."""
        try:
            rel_path = file_path.relative_to(self.root).as_posix()
        except ValueError:
            rel_path = file_path.name

        is_ok, msg = self._run_cmd([str(self.luac), "-p", str(file_path)])
        status = "ok" if is_ok else "syntax_error"

        return LuaFileAnalysisResult(
            relative_path=rel_path,
            is_syntax_ok=is_ok,
            message=msg,
            encoding="utf-8",
            status=status,
        )

    def run_diagnostics(self, max_workers: int | None = None) -> LuaBatchResult:
        """Perform multi-threaded syntax validation across all Lua scripts in root."""
        if not self.luac.is_file():
            logger.error("Lua 4 compiler not found at: %s", self.luac)
            return LuaBatchResult()

        files = [f for f in self.root.rglob("*.lua") if f.is_file()]
        total_files = len(files)
        if total_files == 0:
            logger.warning("No .lua files found in %s", self.root)
            return LuaBatchResult()

        workers = max_workers or min(32, (os.cpu_count() or 1) * 4)
        logger.info(
            "Running syntax diagnostics on %d files using %d worker threads...",
            total_files,
            workers,
        )

        batch_res = LuaBatchResult(total_files=total_files)
        start_time = time.time()

        with ThreadPoolExecutor(max_workers=workers) as executor:
            future_map = {executor.submit(self.check_file_syntax, f): f for f in files}

            for idx, future in enumerate(as_completed(future_map), 1):
                if self.progress_callback and (idx % 100 == 0 or idx == total_files):
                    self.progress_callback(idx, total_files)

                try:
                    res = future.result()
                    if res.is_syntax_ok:
                        batch_res.passed_files += 1
                    else:
                        batch_res.failed_files += 1
                        batch_res.failures.append(res)
                except (OSError, RuntimeError) as err:
                    batch_res.failed_files += 1
                    failed_file = future_map[future]
                    batch_res.failures.append(
                        LuaFileAnalysisResult(
                            relative_path=str(failed_file),
                            is_syntax_ok=False,
                            message=f"Thread execution error: {err}",
                            status="error",
                        )
                    )

        batch_res.elapsed_seconds = time.time() - start_time
        return batch_res

    def run_formatting(
        self, indent_unit: str = "\t", max_workers: int | None = None
    ) -> int:
        """Safely formats all Lua files under root directory matching original SpellForce style (multi-threaded)."""
        files = [f for f in self.root.rglob("*.lua") if f.is_file()]
        total_files = len(files)
        if total_files == 0:
            return 0

        workers = max_workers or min(32, (os.cpu_count() or 1) * 4)
        logger.info(
            "Formatting %d files using %d worker threads (unit=%r)...",
            total_files,
            workers,
            indent_unit,
        )

        formatted_count = 0

        def _format_task(file_path: Path) -> bool:
            try:
                content = file_path.read_text(encoding="utf-8", errors="replace")
                formatted = format_lua_source(content, indent_unit=indent_unit)
                if formatted != content:
                    file_path.write_text(formatted, encoding="utf-8")
                    return True
            except OSError as err:
                logger.warning("Failed to format %s: %s", file_path.name, err)
            return False

        with ThreadPoolExecutor(max_workers=workers) as executor:
            futures = [executor.submit(_format_task, f) for f in files]
            for idx, future in enumerate(as_completed(futures), 1):
                if self.progress_callback and (idx % 200 == 0 or idx == total_files):
                    self.progress_callback(idx, total_files)
                if future.result():
                    formatted_count += 1

        print()
        logger.info("Formatted %d of %d files.", formatted_count, total_files)
        return formatted_count


def _print_progress(done: int, total: int) -> None:
    pct = (done / total) * 100
    print(f"\rProgress: [{done}/{total}] ({pct:.1f}%)", end="", flush=True)


def main() -> None:
    """CLI entry point for lua_toolkit."""
    parser = argparse.ArgumentParser(
        description="SpellForce 1 (Lua 4.0.1) Diagnostics, Compilation & Formatting Toolkit"
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    # Subcommand: check
    parser_check = subparsers.add_parser(
        "check", help="Run multi-threaded syntax diagnostics"
    )
    parser_check.add_argument(
        "directory", type=Path, help="Directory containing .lua scripts"
    )
    parser_check.add_argument(
        "--luac", type=Path, default=None, help="Path to luac4.exe"
    )

    # Subcommand: format
    parser_format = subparsers.add_parser(
        "format",
        help="Normalize formatting and indentation to match original game scripts",
    )
    parser_format.add_argument(
        "directory", type=Path, help="Directory to format in-place"
    )
    parser_format.add_argument(
        "--spaces",
        type=int,
        default=None,
        help="Use spaces instead of tabs (e.g. --spaces 2)",
    )

    args = parser.parse_args()

    if args.command == "check":
        toolkit = LuaToolkit(
            args.directory, luac_path=args.luac, progress_callback=_print_progress
        )
        res = toolkit.run_diagnostics()
        print()
        print("=" * 60)
        print(f"Total files checked: {res.total_files}")
        print(f"Syntax OK:           {res.passed_files}")
        print(f"Syntax Errors:       {res.failed_files}")
        print(f"Elapsed Time:        {res.elapsed_seconds:.2f} s")
        print("=" * 60)

        if res.failures:
            print(f"\nALL SYNTAX ERRORS ({len(res.failures)} files):")
            for fail in res.failures:
                print(f"  [{fail.relative_path}]:\n    {fail.message}\n")

    elif args.command == "format":
        unit = (" " * args.spaces) if args.spaces else "\t"
        toolkit = LuaToolkit(args.directory, progress_callback=_print_progress)
        toolkit.run_formatting(indent_unit=unit)


if __name__ == "__main__":
    main()
