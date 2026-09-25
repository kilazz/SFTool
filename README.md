SFTool

Modding, balance, and scripting suite for SpellForce 1 and SpellForce 2. Written
in Rust with a Slint GUI.

Features

📦 PAK & VFS

  - Formats: SF1 (MASSIVE PAKFILE V 4.0) and SF2 (PAK\x01).
  - SF1 Engine Accuracy: 16-bit K&R path hashing, in-engine sorting
    comparator, 4-byte DWORD alignment, SHA-256 payload deduplication.
  - Operations: Single/batch unpack & pack, in-memory asset streaming (.dds,
    .tga, .png), VFS directory tree.

🗄️ CFF Database & JSON Pipeline

  - 100% Stride Coverage: All 49 SF1 database chunks fully mapped with 0
    unmapped bytes.
  - Automated Round-Trip:
      - unpack_cff: extracts .dat chunks \to auto-exports texts_json/ (strings)
        and tables_json/ (all 27+ structured tables in clean JSON).
      - pack_cff: auto-recompiles tables_json/ and texts_json/ back into .dat
        chunks \to builds .cff.
  - Diff & Patch: Non-destructive JSON delta patches between database versions.
  - Audit: Mathematical byte-level coverage verification (audit_coverage).

⚖️ Balance & Entity Editor

  - Contextual 12-Field Inspector:
      - Spells (0x07D2, 0x0806, 0x07E2): Mana, cast/recast (ms), range, power %,
        AoE radius, target faction/mode, damage params, Dual-Card scroll
        preview.
      - Weapons (0x07DF): Damage min/max, speed %, range, type, material, live
        DPS calculation.
      - RTS Tech & Titans (0x07F4): Dedicated fields for all 7 resources (Wood,
        Stone, Iron, Lenya, Aria, Moonglass, Food), research time, button icon
        preview.
      - Loot Tables (0x07F8, 0x0811): Item slots, drop chances, live cascading
        probability readout.
      - Units & Stats (0x07D5, 0x07E8): Attributes, resistances, movement/combat
        speeds, XP falloff curves.
      - Collision (0x07EE, 0x0809): 2D vector polygon inspection for buildings
        and objects.
  - Localization: Multiline editor, language slot clone wizard (Campaign << 24 |
    Lang << 16 | BaseID), session Undo (↶) / Redo (↷).

📜 Lua Suite

  - SpellForce 1 (Lua 4.0.1):
      - Multithreaded decompilation (luadec).
      - Syntax diagnostics (luac4 -p).
      - Code formatter (Phenomic tab-indentation style).
      - 3D visual binding editor (script/sql_*.lua) with Undo/Redo.
      - RTS co-op spawn editor (GdsRtsCoopSpawnGroups.lua \leftrightarrow JSON).
  - SpellForce 2 (Lua 5.1 GDS):
      - 560+ function GDS API database with 1-click snippet copy.
      - Map scaffolding generator (cutscenetext.lua, outcrytext.lua,
        _<map>.lua).
      - Batch StyLua formatter (chunked in 50-file batches).
      - EmmyLua definition export for VS Code.

🛠️ Diagnostics & Math Tools

  - DDS Repair: Auto-fixes corrupt mipmap counts and Direct3D cap flags in DDS
    headers.
  - Terrain Generation: Parallel 16-bit grayscale heightmap synthesis (Rayon,
    cellular erosion, Gaussian blur).
  - CLI Calculators: Weapon DPS, monster XP falloff, cascading loot odds, stat
    curves, flag decoders.

🔍 Binary Toolchain (./bin/)

Auto-detects helper tools in ./bin/, adjacent to SFTool.exe, or in PATH:
luadec_32_deb.exe, luac4.exe, luac5.1.exe, stylua.exe.

CLI Reference
```
# PAK
SFTool list_pak <archive.pak>
SFTool unpack_pak <archive.pak> <out_dir>
SFTool pack_pak <src_dir> <out_archive.pak> [sf1|sf2] [comp: 0-9]
SFTool batch_unpack_pak <root_folder>
SFTool batch_pack_pak <root_folder> [sf1|sf2] [comp: 0-9]

# CFF & Database
SFTool unpack_cff <GameData.cff> <out_dir>
SFTool pack_cff <in_dir> <out_GameData.cff> [comp: 0-9]
SFTool audit_coverage <cff_dir>
SFTool dump_all_json <cff_dir> <out_dir>
SFTool create_diff <base_dir> <mod_dir> <patch.json>
SFTool apply_diff <target_dir> <patch.json>
SFTool validate_cff <cff_dir>
SFTool clone_slot <cff_dir> <src_slot> <dst_slot>
SFTool replace_slot <cff_dir> <target_slot> <translation.json>
SFTool trace_id <cff_dir> <category_id> <target_id>
SFTool find_refs <cff_dir> <category_id> <target_id>

# Textures, Terrain & Diagnostics
SFTool repair_dds <texture_dir_or_file>
SFTool generate_terrain <width> <height> <out.png> [base_z: 2000]

# Math Calculators
SFTool calc_dps <min_dmg> <max_dmg> <speed>
SFTool calc_xp <gain> <falloff> [kills: 500]
SFTool calc_loot <chance1> <chance2>
SFTool calc_hp <stamina> <wisdom> [hp_factor] [mana_factor]
SFTool decode_flags <race|item|ai|cultivation|clan|relation|slot> <value>

# Lua Bindings & Co-op Spawns
SFTool dump_coop_spawns <GdsRtsCoopSpawnGroups.lua> <out.json>
SFTool compile_coop_spawns <in.json> <GdsRtsCoopSpawnGroups.lua>
SFTool dump_sql_items <sql_item.lua> <out.json>
SFTool compile_sql_items <in.json> <sql_item.lua>
SFTool dump_sql_buildings <sql_building.lua> <out.json>
SFTool compile_sql_buildings <in.json> <sql_building.lua>
SFTool dump_sql_objects <sql_object.lua> <out.json>
SFTool compile_sql_objects <in.json> <sql_object.lua>
SFTool dump_sql_heads <sql_head.lua> <out.json>
SFTool compile_sql_heads <in.json> <sql_head.lua>

# Lua Scripts
SFTool decompile_lua <src_dir> <out_dir> [luadec_exe] [--no-resume]
SFTool check_lua <scripts_dir> [luac_exe]
SFTool check_lua5 <scripts_dir> [luac5_exe]
SFTool format_lua <scripts_dir> [--spaces <n>]
SFTool format_stylua <scripts_dir> [stylua_exe] [--spaces]
SFTool create_map <target_dir> <project_name> <map_name>
SFTool export_emmylua <out_file.lua>
```
Build
```
cargo build --release
cargo clippy
```