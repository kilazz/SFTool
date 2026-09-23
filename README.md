# SFTool

Modding, localization, and scripting suite for **SpellForce 1** and **SpellForce 2**. Written in Rust with a Slint GUI.

## Features

### 📦 PAK Archive & VFS
* **Formats:** SpellForce 1 (`MASSIVE PAKFILE V 4.0`) and SpellForce 2 (`PAK\x01`).
* **Engine Accuracy (SF1):** In-engine 16-bit K&R path hashing (`FUN_004a4180`), binary search sorting comparator, and 4-byte DWORD alignment fix preventing DirectX `D3DERR_INVALIDCALL` crashes.
* **Operations:** Single/batch unpack, single/batch pack, interactive directory tree view.

### 🗄️ CFF Database & Diff Engine
* **Container Packaging:** Unpack `.cff` to `.dat` chunks and repack with zlib compression. Enforces the 8,192 chunk TOC limit and mandatory `c_type == 3` for chunk `0x0800`.
* **Diff & Patch Engine:** Generate non-destructive JSON diff patches between database states and merge multiple mod patches without overwriting entire `GameData.cff` files.
* **Schema Detection:** Auto-detects and exports/imports Fixed 566, Format A (UTF-16LE String Table), Format B (Table-Based), and Format C (Developer Table) to/from JSON.

### ⚖️ Balance & Entity Editor
* **Direct PAK Asset Streaming:** Decodes `.dds`, `.tga`, and `.png` icons directly from `.pak` files in memory without extracting archives to disk.
* **Inspectors:**
  * 2D Gfx Items (`0x07DC`) with live icon preview.
  * Spells Mapping (`0x07E2`) with side-by-side Dual Cards cross-referencing Combat Spell and Inventory Scroll.
  * Localized text editor with session Undo (`↶`) and Redo (`↷`).
* **Localization Suite:** One-click language slot clone wizard (`Campaign << 24 | Lang << 16 | BaseID`) and isolated JSON phrase export/import.

### 📜 Lua Scripting Suite
* **SpellForce 1 (Lua 4.0.1):**
  * Multi-threaded bytecode decompilation (`luadec_32_deb.exe`).
  * Syntax checking (`luac4.exe -p`).
  * Tab-indentation source code formatter.
* **SpellForce 2 (Lua 5.1 GDS):**
  * Searchable 560+ function GDS API database with parameter signatures and 1-click clipboard copy.
  * 1-click map project scaffolding (`dialog/cutscenetext.lua`, `outcrytext.lua`, `script/_<map>.lua`).
  * Batch code formatting via StyLua (chunked in 50-file batches to prevent Windows CLI overflow).
  * EmmyLua definition export for VS Code code completion.

### 🔍 Binary Toolchain (`/bin`)
Auto-detects helper executables in `./bin/` or adjacent to `SFTool.exe`:
* `luadec_32_deb.exe`
* `luac4.exe`
* `luac5.1.exe`
* `stylua.exe`

## CLI Commands

```
# PAK
SFTool unpack_pak <archive.pak> <out_dir>
SFTool pack_pak <src_dir> <out_archive.pak> [sf1|sf2] [comp: 0-9]
SFTool batch_unpack_pak <root_folder>
SFTool batch_pack_pak <root_folder> [sf1|sf2] [comp: 0-9]

# CFF
SFTool unpack_cff <GameData.cff> <out_dir>
SFTool pack_cff <in_dir> <out_GameData.cff> [comp: 0-9]
SFTool create_diff <base_dir> <mod_dir> <patch.json>
SFTool apply_diff <target_dir> <patch.json>
SFTool clone_slot <cff_dir> <src_slot> <dst_slot>
SFTool replace_slot <cff_dir> <target_slot> <translation.json>

# Lua
SFTool decompile_lua <src_dir> <out_dir> [luadec_exe] [--no-resume]
SFTool check_lua <scripts_dir> [luac_exe]
SFTool check_lua5 <scripts_dir> [luac5_exe]
SFTool format_lua <scripts_dir> [--spaces <n>]
SFTool format_stylua <scripts_dir> [stylua_exe] [--spaces]
SFTool create_map <target_dir> <project_name> <map_name>
SFTool export_emmylua <out_file.lua>
```

## Build

```
cargo build --release
```
