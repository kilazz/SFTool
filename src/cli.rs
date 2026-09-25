// src/cli.rs

use crate::cff;
use crate::dds;
use crate::logger::make_cli_logger;
use crate::lua;
use crate::pak;
use crate::sav;
use crate::terrain;
use crate::tools;
use std::fs;
use std::path::{Path, PathBuf};

pub fn print_help() {
    println!(
        "\
SFTool v2.2 - SpellForce Modding, Balance & Scripting Studio (CLI Mode)
Usage: SFTool <command> [arguments...]

PAK & VFS Commands:
  list_pak <pak_file>
      List all files stored inside a SpellForce 1 or SpellForce 2 PAK archive.

  unpack_pak <pak_file> <out_dir>
      Extract all files from a SpellForce 1 or SpellForce 2 PAK archive.

  pack_pak <src_dir> <out_pak> [fmt: sf1|sf2] [comp: 0-9]
      Pack directory into PAK (SF1 uses verified in-engine VFS ordering).

  batch_unpack_pak <root_folder>
      Recursively find and extract all .pak archives in root_folder.

  batch_pack_pak <root_folder> [fmt: sf1|sf2] [comp: 0-9]
      Batch pack all '*_extracted' directories back into .pak files.

CFF Database & SaveGame Commands:
  unpack_cff <input_cff> <out_dir>
      Unpack CFF container into binary chunks and export texts to JSON.

  pack_cff <in_dir> <out_cff> [compression_level: 0-9, default: 6]
      Import texts from JSON into chunks and compile into a CFF container.

  audit_coverage <cff_dir>
      Scans all 49 database chunks and verifies 100% byte-level stride integrity.

  dump_all_json <cff_dir> <out_dir>
      Decodes and exports all 27+ structured game tables into clean editable JSON files.

  unpack_sav <input.sav> <out_dir>
      Inspect and extract all chunks and nested containers from a savegame (.sav).

  create_diff <base_cff_dir> <mod_cff_dir> <out_patch.json>
      Generate a non-destructive mod diff patch between base and modified databases.

  apply_diff <target_cff_dir> <patch.json>
      Apply and merge a mod diff patch into a target working database directory.

  validate_cff <cff_dir>
      Audit entire database and report all broken relational links / missing IDs.

  clone_slot <cff_dir> <src_slot: 0-4> <dst_slot: 5>
      Clone an existing language slot into a new slot (e.g. Russian slot 5).

  replace_slot <cff_dir> <target_slot: 0-5> <translation.json>
      Replace all phrases of a language slot with texts from a JSON file.

  trace_id <cff_dir> <category_id> <target_id>
      Resolve the exact file record index, print table group & trace incoming references.

  find_refs <cff_dir> <category_id> <target_id>
      Trace all cross-category relational foreign key references across all database tables.

Diagnostic & Balance Tools:
  repair_dds <texture_dir_or_file>
      Scans and repairs corrupt DDS texture mipmap headers that cause DirectX crashes.

  calc_dps <min_damage> <max_damage> <speed>
      Calculates weapon Damage-Per-Second using Phenomic's combat formula.

  calc_xp <xp_gain> <xp_falloff> [kills: default 500]
      Computes total farmable experience with monster kill falloff curve.

  calc_loot <chance1> <chance2>
      Computes cascading drop chances for monster/chest slots.

  calc_hp <stamina> <wisdom> [hp_factor: 100] [mana_factor: 100]
      Computes effective creature Health and Mana based on stat curves.

  decode_flags <race|item|ai|cultivation|clan|relation|slot> <value>
      Translates binary bitmasks, clans, relations, or equipment slots into human-readable tags.

Terrain & Map Generation:
  generate_terrain <width> <height> <out.png> [base_z: default 2000]
      Procedurally generate an erosion heightmap using parallel Rayon algorithms.

SpellForce 1 Visual Bindings & Co-op Spawns:
  dump_coop_spawns <GdsRtsCoopSpawnGroups.lua> <out.json>
      Export RTS co-op spawn groups, waves, and schedules into editable JSON.

  compile_coop_spawns <in.json> <GdsRtsCoopSpawnGroups.lua>
      Compile JSON back into valid GdsRtsCoopSpawnGroups.lua.

  dump_sql_items <sql_item.lua> <out.json>
      Dumps sql_item.lua into clean editable JSON.

  compile_sql_items <in.json> <sql_item.lua>
      Compiles JSON back into valid Phenomic sql_item.lua.

  dump_sql_buildings <sql_building.lua> <out.json>
      Dumps sql_building.lua into clean editable JSON.

  compile_sql_buildings <in.json> <sql_building.lua>
      Compiles JSON back into valid Phenomic sql_building.lua.

  dump_sql_objects <sql_object.lua> <out.json>
      Dumps sql_object.lua into clean editable JSON.

  compile_sql_objects <in.json> <sql_object.lua>
      Compiles JSON back into valid Phenomic sql_object.lua.

  dump_sql_heads <sql_head.lua> <out.json>
      Dumps sql_head.lua into clean editable JSON.

  compile_sql_heads <in.json> <sql_head.lua>
      Compiles JSON back into valid Phenomic sql_head.lua.

Lua Scripting Commands:
  decompile_lua <src_dir> <out_dir> [luadec_exe] [--no-resume]
      High-performance parallel Lua 4.0 bytecode decompiler (SF1).

  check_lua <scripts_dir> [luac_exe]
      Parallel syntax validation using 'luac -p' (defaults to luac4.exe).

  check_lua5 <scripts_dir> [luac5_exe]
      Parallel syntax validation using 'luac5.1 -p' (SF2).

  format_lua <scripts_dir> [--spaces <n>]
      Beautify and indent Lua 4.0 scripts to match original Phenomic source.

  format_stylua <scripts_dir> [stylua_exe] [--spaces]
      Batch format SpellForce 2 Lua 5.1 scripts using StyLua.

  create_map <target_dir> <project_name> <map_name>
      Generate standard SpellForce 2 map scaffolding (scripts, dialogs).

  export_emmylua <out_file.lua>
      Export complete SpellForce 2 Lua API definitions for VS Code (EmmyLua).

General:
  help, --help, -h
      Show this help message."
    );
}

pub fn handle_cli(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let cmd = args[1].to_lowercase();
    match cmd.as_str() {
        "--help" | "-h" | "help" => print_help(),

        "list_pak" => {
            if args.len() < 3 {
                eprintln!("Usage: SFTool list_pak <pak_file>");
                return Ok(());
            }
            let p = Path::new(&args[2]);
            let files = pak::list_pak_files(p)?;
            println!("[+] Files in {:?} ({} entries):", p, files.len());
            for f in files {
                println!("  {}", f);
            }
        }

        "repair_dds" => {
            if args.len() < 3 {
                eprintln!("Usage: SFTool repair_dds <texture_dir_or_file>");
                return Ok(());
            }
            let target_path = Path::new(&args[2]);
            let (logger, handle) = make_cli_logger();

            if target_path.is_file() {
                let fixed = dds::repair_single_dds(target_path)?;
                if fixed {
                    println!("[+] Successfully repaired DDS header: {:?}", target_path);
                } else {
                    println!(
                        "[*] Texture header is valid or unmodified: {:?}",
                        target_path
                    );
                }
            } else if target_path.is_dir() {
                let report = dds::batch_repair_dds(target_path, &logger)?;
                println!(
                    "[+] Finished scan. Scanned: {}, Corrupt found: {}, Fixed: {}",
                    report.total_scanned, report.broken_found, report.fixed_count
                );
            }
            drop(logger);
            let _ = handle.join();
        }

        "unpack_sav" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool unpack_sav <input.sav> <out_dir>");
                return Ok(());
            }
            let (logger, handle) = make_cli_logger();
            sav::inspect_and_unpack_sav(Path::new(&args[2]), Path::new(&args[3]), &logger)?;
            drop(logger);
            let _ = handle.join();
        }

        "dump_coop_spawns" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool dump_coop_spawns <GdsRtsCoopSpawnGroups.lua> <out.json>");
                return Ok(());
            }
            let map = lua::coop_spawns::parse_coop_spawns(Path::new(&args[2]))?;
            let f = fs::File::create(Path::new(&args[3]))?;
            serde_json::to_writer_pretty(f, &map)?;
            println!(
                "[+] Exported {} coop spawn groups to {:?}",
                map.len(),
                args[3]
            );
        }

        "compile_coop_spawns" => {
            if args.len() < 4 {
                eprintln!(
                    "Usage: SFTool compile_coop_spawns <in.json> <GdsRtsCoopSpawnGroups.lua>"
                );
                return Ok(());
            }
            let content = fs::read_to_string(Path::new(&args[2]))?;
            let map: std::collections::BTreeMap<u32, lua::coop_spawns::CoopSpawnGroup> =
                serde_json::from_str(&content)?;
            lua::coop_spawns::save_coop_spawns(Path::new(&args[3]), &map)?;
            println!(
                "[+] Compiled {} coop spawn groups into {:?}",
                map.len(),
                args[3]
            );
        }

        "generate_terrain" => {
            if args.len() < 5 {
                eprintln!("Usage: SFTool generate_terrain <width> <height> <out.png> [base_z]");
                return Ok(());
            }
            let width = args[2].parse::<usize>().unwrap_or(256);
            let height = args[3].parse::<usize>().unwrap_or(256);
            let base_z = args
                .get(5)
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or(2000);

            let cfg = terrain::generator::MapGenConfig {
                width,
                height,
                base_z,
                ..Default::default()
            };

            println!(
                "[*] Generating {}x{} heightmap with cellular erosion...",
                width, height
            );
            let hdata = terrain::generator::TerrainGenerator::generate_heightmap(&cfg);
            terrain::generator::TerrainGenerator::export_png_16bit(
                &hdata,
                width as u32,
                height as u32,
                Path::new(&args[4]),
            )?;
            println!("[+] Heightmap exported successfully to {:?}", args[4]);
        }

        "trace_id" => {
            if args.len() < 5 {
                eprintln!("Usage: SFTool trace_id <cff_dir> <category_id> <target_id>");
                return Ok(());
            }
            let cat_id = args[3].parse::<u32>().unwrap_or(2003);
            let target_id = args[4].parse::<u32>().unwrap_or(0);
            let cff_dir = Path::new(&args[2]);

            let mut tracer = cff::tracer::TracerEngine::default();
            let effective_cat = tracer.get_redirect(cat_id);
            if effective_cat != cat_id {
                println!(
                    "[*] Category 0x{:04X} redirected to relational root 0x{:04X}",
                    cat_id, effective_cat
                );
            }

            if let Some(group) = tracer.get_table_group(cat_id) {
                let grp_str = group
                    .iter()
                    .map(|id| format!("0x{:04X}", id))
                    .collect::<Vec<_>>()
                    .join(", ");
                println!("[*] Relational Table Group: [{}]", grp_str);
            }

            if let Some(rec_idx) =
                cff::tracer::TracerEngine::resolve_entity_index(cff_dir, cat_id, target_id)
            {
                tracer.push_step(cff::tracer::TracePoint {
                    category_id: cat_id,
                    entity_id: target_id,
                    record_index: rec_idx,
                });
                if let Some(curr) = tracer.current() {
                    println!(
                        "[+] Resolved Entity ID {} in Category 0x{:04X} -> Record Index #{}",
                        curr.entity_id, curr.category_id, curr.record_index
                    );
                }
            } else {
                println!(
                    "[-] Entity ID {} not found in Category 0x{:04X}",
                    target_id, cat_id
                );
            }

            let incoming = tracer.find_references(cff_dir, cat_id, target_id);
            if !incoming.is_empty() {
                println!(
                    "[+] Incoming Foreign Key References ({} found):",
                    incoming.len()
                );
                for r in &incoming {
                    println!(
                        "  -> [{}] Record #{} (Field: {})",
                        r.category_name, r.record_index, r.field_name
                    );
                }

                if let Some(first_ref) = incoming.first() {
                    tracer.push_step(cff::tracer::TracePoint {
                        category_id: first_ref.category_id,
                        entity_id: first_ref.target_id,
                        record_index: first_ref.record_index,
                    });
                    if tracer.can_go_back()
                        && let Some(prev) = tracer.go_back()
                    {
                        println!(
                            "  [*] Relational Trail: navigated back to Category 0x{:04X} (Record #{})",
                            prev.category_id, prev.record_index
                        );
                    }
                    if tracer.can_go_forward()
                        && let Some(next) = tracer.go_forward()
                    {
                        println!(
                            "  [*] Relational Trail: navigated forward to Category 0x{:04X} (Record #{})",
                            next.category_id, next.record_index
                        );
                    }
                }
            }
        }

        "find_refs" => {
            if args.len() < 5 {
                eprintln!("Usage: SFTool find_refs <cff_dir> <category_id> <target_id>");
                return Ok(());
            }
            let cat_id = args[3].parse::<u32>().unwrap_or(2003);
            let target_id = args[4].parse::<u32>().unwrap_or(0);
            let refs = cff::references::find_all_references(Path::new(&args[2]), cat_id, target_id);
            let desc = cff::sf1::describe_category(cat_id).unwrap_or("General Category");
            println!(
                "[+] Found {} references for ID {} in Category 0x{:04X} [{}]:",
                refs.len(),
                target_id,
                cat_id,
                desc
            );
            for r in refs {
                println!(
                    "  -> [{}] Record #{} (Field: {})",
                    r.category_name, r.record_index, r.field_name
                );
            }
        }

        "calc_dps" => {
            if args.len() < 5 {
                eprintln!("Usage: SFTool calc_dps <min_dmg> <max_dmg> <speed>");
                return Ok(());
            }
            let min = args[2].parse::<u16>().unwrap_or(10);
            let max = args[3].parse::<u16>().unwrap_or(20);
            let spd = args[4].parse::<u16>().unwrap_or(100);
            let dps = cff::formulas::calculate_weapon_dps(min, max, spd);
            println!(
                "[+] Weapon Damage: {}-{} | Speed: {}% | Calculated DPS: {:.2}",
                min, max, spd, dps
            );
        }

        "calc_xp" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool calc_xp <xp_gain> <xp_falloff> [kills]");
                return Ok(());
            }
            let gain = args[2].parse::<u32>().unwrap_or(100);
            let falloff = args[3].parse::<u16>().unwrap_or(10);
            let kills = args
                .get(4)
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(500);
            let total = cff::formulas::calculate_total_xp(gain, falloff, kills);
            println!(
                "[+] Base XP: {} | Falloff: {} | Max Farmable XP ({} kills): {}",
                gain, falloff, kills, total
            );
        }

        "calc_loot" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool calc_loot <chance1> <chance2>");
                return Ok(());
            }
            let c1 = args[2].parse::<u8>().unwrap_or(50);
            let c2 = args[3].parse::<u8>().unwrap_or(50);
            let (eff1, eff2, eff3) = cff::formulas::calculate_cascade_loot_chances(c1, c2);
            println!(
                "[+] Cascading Effective Chances: Slot 1: {:.1}%, Slot 2: {:.1}%, No Drop: {:.1}%",
                eff1, eff2, eff3
            );
        }

        "calc_hp" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool calc_hp <stamina> <wisdom> [hp_factor] [mana_factor]");
                return Ok(());
            }
            let sta = args[2].parse::<u16>().unwrap_or(10);
            let wis = args[3].parse::<u16>().unwrap_or(10);
            let h_fac = args
                .get(4)
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or(100);
            let m_fac = args
                .get(5)
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or(100);
            let (hp, mana) = cff::formulas::calculate_health_and_mana(sta, wis, h_fac, m_fac);
            println!(
                "[+] Calculated Unit Stats: Health: {} HP, Mana: {} MP",
                hp, mana
            );
        }

        "decode_flags" => {
            if args.len() < 4 {
                eprintln!(
                    "Usage: SFTool decode_flags <race|item|ai|cultivation|clan|relation|slot> <value>"
                );
                return Ok(());
            }
            let val = args[3].parse::<u16>().unwrap_or(0);
            match args[2].to_lowercase().as_str() {
                "race" => println!(
                    "[+] Race Flags (0x{:02X}): {}",
                    val as u8,
                    cff::formulas::format_race_flags(val as u8)
                ),
                "item" => println!(
                    "[+] Item Options (0x{:02X}): {}",
                    val as u8,
                    cff::formulas::format_item_options(val as u8)
                ),
                "ai" => println!(
                    "[+] Race AI Flags (0x{:04X}): {:?}",
                    val,
                    cff::formulas::decode_ai_flags(val)
                ),
                "cultivation" => println!(
                    "[+] Cultivation Flags (0x{:02X}): {:?}",
                    val as u8,
                    cff::formulas::decode_cultivation_flags(val as u8)
                ),
                "clan" => println!(
                    "[+] Faction Clan #{} [Diplomacy]: {}",
                    val as u8,
                    cff::sf1_schema::get_clan_name(val as u8)
                ),
                "relation" => println!(
                    "[+] Diplomacy Relation ({}): {}",
                    val as u8,
                    cff::sf1_schema::decode_diplomacy_relation(val as u8)
                ),
                "slot" => println!(
                    "[+] Unit Equipment Slot #{}: {}",
                    val as u8,
                    cff::sf1_schema::get_equipment_slot_name(val as u8)
                ),
                _ => {
                    eprintln!(
                        "[!] Unknown flag type. Use 'race', 'item', 'ai', 'cultivation', 'clan', 'relation', or 'slot'."
                    )
                }
            }
        }

        "dump_sql_items" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool dump_sql_items <sql_item.lua> <out.json>");
                return Ok(());
            }
            let map = lua::sql::load_sql_items(Path::new(&args[2]))?;
            let f = fs::File::create(Path::new(&args[3]))?;
            serde_json::to_writer_pretty(f, &map)?;
            println!(
                "[+] Exported {} items from {} to {:?}",
                map.len(),
                args[2],
                args[3]
            );
        }

        "compile_sql_items" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool compile_sql_items <in.json> <sql_item.lua>");
                return Ok(());
            }
            let content = fs::read_to_string(Path::new(&args[2]))?;
            let map: std::collections::BTreeMap<u32, lua::sql::SqlItemEntry> =
                serde_json::from_str(&content)?;
            lua::sql::save_sql_items(Path::new(&args[3]), &map)?;
            println!(
                "[+] Compiled {} items from JSON into {:?}",
                map.len(),
                args[3]
            );
        }

        "dump_sql_buildings" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool dump_sql_buildings <sql_building.lua> <out.json>");
                return Ok(());
            }
            let map = lua::sql::load_sql_buildings(Path::new(&args[2]))?;
            let f = fs::File::create(Path::new(&args[3]))?;
            serde_json::to_writer_pretty(f, &map)?;
            println!(
                "[+] Exported {} buildings from {} to {:?}",
                map.len(),
                args[2],
                args[3]
            );
        }

        "compile_sql_buildings" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool compile_sql_buildings <in.json> <sql_building.lua>");
                return Ok(());
            }
            let content = fs::read_to_string(Path::new(&args[2]))?;
            let map: std::collections::BTreeMap<u32, lua::sql::SqlBuildingEntry> =
                serde_json::from_str(&content)?;
            lua::sql::save_sql_buildings(Path::new(&args[3]), &map)?;
            println!(
                "[+] Compiled {} buildings from JSON into {:?}",
                map.len(),
                args[3]
            );
        }

        "dump_sql_objects" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool dump_sql_objects <sql_object.lua> <out.json>");
                return Ok(());
            }
            let map = lua::sql::load_sql_objects(Path::new(&args[2]))?;
            let f = fs::File::create(Path::new(&args[3]))?;
            serde_json::to_writer_pretty(f, &map)?;
            println!(
                "[+] Exported {} objects from {} to {:?}",
                map.len(),
                args[2],
                args[3]
            );
        }

        "compile_sql_objects" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool compile_sql_objects <in.json> <sql_object.lua>");
                return Ok(());
            }
            let content = fs::read_to_string(Path::new(&args[2]))?;
            let map: std::collections::BTreeMap<u32, lua::sql::SqlObjectEntry> =
                serde_json::from_str(&content)?;
            lua::sql::save_sql_objects(Path::new(&args[3]), &map)?;
            println!(
                "[+] Compiled {} objects from JSON into {:?}",
                map.len(),
                args[3]
            );
        }

        "dump_sql_heads" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool dump_sql_heads <sql_head.lua> <out.json>");
                return Ok(());
            }
            let map = lua::sql::load_sql_heads(Path::new(&args[2]))?;
            let f = fs::File::create(Path::new(&args[3]))?;
            serde_json::to_writer_pretty(f, &map)?;
            println!(
                "[+] Exported {} heads from {} to {:?}",
                map.len(),
                args[2],
                args[3]
            );
        }

        "compile_sql_heads" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool compile_sql_heads <in.json> <sql_head.lua>");
                return Ok(());
            }
            let content = fs::read_to_string(Path::new(&args[2]))?;
            let map: std::collections::BTreeMap<u32, lua::sql::SqlHeadEntry> =
                serde_json::from_str(&content)?;
            lua::sql::save_sql_heads(Path::new(&args[3]), &map)?;
            println!(
                "[+] Compiled {} heads from JSON into {:?}",
                map.len(),
                args[3]
            );
        }

        "unpack_cff" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool unpack_cff <input_cff> <out_dir>");
                return Ok(());
            }
            let (logger, handle) = make_cli_logger();
            cff::unpack_all(Path::new(&args[2]), Path::new(&args[3]), &logger)?;
            drop(logger);
            let _ = handle.join();
        }

        "pack_cff" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool pack_cff <in_dir> <out_cff> [compression_level]");
                return Ok(());
            }
            let comp = args.get(4).and_then(|s| s.parse::<u32>().ok()).unwrap_or(6);
            let (logger, handle) = make_cli_logger();
            cff::pack_all(Path::new(&args[2]), Path::new(&args[3]), comp, &logger)?;
            drop(logger);
            let _ = handle.join();
        }

        "audit_coverage" => {
            if args.len() < 3 {
                eprintln!("Usage: SFTool audit_coverage <cff_dir>");
                return Ok(());
            }
            let (logger, handle) = make_cli_logger();
            cff::dump::audit_coverage(Path::new(&args[2]), &logger)?;
            drop(logger);
            let _ = handle.join();
        }

        "dump_all_json" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool dump_all_json <cff_dir> <out_dir>");
                return Ok(());
            }
            let (logger, handle) = make_cli_logger();
            cff::dump::dump_all_json(Path::new(&args[2]), Path::new(&args[3]), &logger)?;
            drop(logger);
            let _ = handle.join();
        }

        "create_diff" => {
            if args.len() < 5 {
                eprintln!(
                    "Usage: SFTool create_diff <base_cff_dir> <mod_cff_dir> <out_patch.json>"
                );
                return Ok(());
            }
            let (logger, handle) = make_cli_logger();
            cff::create_diff(
                Path::new(&args[2]),
                Path::new(&args[3]),
                Path::new(&args[4]),
                &logger,
            )?;
            drop(logger);
            let _ = handle.join();
        }

        "apply_diff" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool apply_diff <target_cff_dir> <patch.json>");
                return Ok(());
            }
            let (logger, handle) = make_cli_logger();
            cff::apply_patch(Path::new(&args[2]), Path::new(&args[3]), &logger)?;
            drop(logger);
            let _ = handle.join();
        }

        "validate_cff" => {
            if args.len() < 3 {
                eprintln!("Usage: SFTool validate_cff <cff_dir>");
                return Ok(());
            }
            let (logger, handle) = make_cli_logger();
            let report = cff::validation::validate_cff_integrity(Path::new(&args[2]), &logger)?;
            drop(logger);
            let _ = handle.join();

            if report.broken_references.is_empty() {
                println!(
                    "[+] Database integrity verified: {} tables and {} foreign keys checked. No broken links found.",
                    report.total_tables_checked, report.total_foreign_keys_checked
                );
            } else {
                println!(
                    "[-] Database integrity check finished: found {} broken reference(s) across {} tables.",
                    report.broken_references.len(),
                    report.total_tables_checked
                );
            }
        }

        "clone_slot" => {
            if args.len() < 5 {
                eprintln!("Usage: SFTool clone_slot <cff_dir> <src_slot> <dst_slot>");
                return Ok(());
            }
            let src = args[3].parse::<u16>().unwrap_or(1);
            let dst = args[4].parse::<u16>().unwrap_or(5);
            let (logger, handle) = make_cli_logger();
            cff::clone_language_slot(Path::new(&args[2]), src, dst, &logger)?;
            drop(logger);
            let _ = handle.join();
        }

        "replace_slot" => {
            if args.len() < 5 {
                eprintln!("Usage: SFTool replace_slot <cff_dir> <target_slot> <translation.json>");
                return Ok(());
            }
            let slot = args[3].parse::<u16>().unwrap_or(1);
            let (logger, handle) = make_cli_logger();
            cff::replace_slot_from_json(Path::new(&args[2]), slot, Path::new(&args[4]), &logger)?;
            drop(logger);
            let _ = handle.join();
        }

        "unpack_pak" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool unpack_pak <pak_file> <out_dir>");
                return Ok(());
            }
            let (logger, handle) = make_cli_logger();
            pak::unpack_pak(Path::new(&args[2]), Path::new(&args[3]), &logger)?;
            drop(logger);
            let _ = handle.join();
        }

        "pack_pak" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool pack_pak <src_dir> <out_pak> [fmt: sf1/sf2] [comp]");
                return Ok(());
            }
            let fmt = args.get(4).map(|s| s.as_str()).unwrap_or("sf1");
            let comp = args.get(5).and_then(|s| s.parse::<u32>().ok()).unwrap_or(6);
            let (logger, handle) = make_cli_logger();
            pak::pack_pak(Path::new(&args[2]), Path::new(&args[3]), fmt, comp, &logger)?;
            drop(logger);
            let _ = handle.join();
        }

        "batch_unpack_pak" => {
            if args.len() < 3 {
                eprintln!("Usage: SFTool batch_unpack_pak <root_folder>");
                return Ok(());
            }
            let (logger, handle) = make_cli_logger();
            pak::batch_unpack_paks(Path::new(&args[2]), &logger)?;
            drop(logger);
            let _ = handle.join();
        }

        "batch_pack_pak" => {
            if args.len() < 3 {
                eprintln!("Usage: SFTool batch_pack_pak <root_folder> [fmt: sf1/sf2] [comp]");
                return Ok(());
            }
            let fmt = args.get(3).map(|s| s.as_str()).unwrap_or("sf1");
            let comp = args.get(4).and_then(|s| s.parse::<u32>().ok()).unwrap_or(6);
            let (logger, handle) = make_cli_logger();
            pak::batch_pack_folders(Path::new(&args[2]), fmt, comp, &logger)?;
            drop(logger);
            let _ = handle.join();
        }

        "decompile_lua" => {
            if args.len() < 4 {
                eprintln!(
                    "Usage: SFTool decompile_lua <src_dir> <out_dir> [luadec_exe] [--no-resume]"
                );
                return Ok(());
            }
            let luadec = args
                .get(4)
                .map(PathBuf::from)
                .unwrap_or_else(|| tools::find_tool("luadec_32_deb.exe"));
            let resume = !args.iter().any(|a| a == "--no-resume");
            let (logger, handle) = make_cli_logger();
            lua::sf1::batch_decompile(
                Path::new(&args[2]),
                Path::new(&args[3]),
                &luadec,
                resume,
                &logger,
            )?;
            drop(logger);
            let _ = handle.join();
        }

        "check_lua" => {
            if args.len() < 3 {
                eprintln!("Usage: SFTool check_lua <scripts_dir> [luac_exe]");
                return Ok(());
            }
            let luac = args
                .get(3)
                .map(PathBuf::from)
                .unwrap_or_else(|| tools::find_tool("luac4.exe"));
            let (logger, handle) = make_cli_logger();
            lua::sf1::batch_check_syntax(Path::new(&args[2]), &luac, &logger)?;
            drop(logger);
            let _ = handle.join();
        }

        "check_lua5" => {
            if args.len() < 3 {
                eprintln!("Usage: SFTool check_lua5 <scripts_dir> [luac5_exe]");
                return Ok(());
            }
            let luac = args
                .get(3)
                .map(PathBuf::from)
                .unwrap_or_else(|| tools::find_tool("luac5.1.exe"));
            let (logger, handle) = make_cli_logger();
            let _ = lua::sf2::batch_check_syntax_sf2(Path::new(&args[2]), &luac, &logger)?;
            drop(logger);
            let _ = handle.join();
        }

        "format_lua" => {
            if args.len() < 3 {
                eprintln!("Usage: SFTool format_lua <scripts_dir> [--spaces <n>]");
                return Ok(());
            }
            let unit = if let Some(idx) = args.iter().position(|a| a == "--spaces") {
                let n = args
                    .get(idx + 1)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(2);
                " ".repeat(n)
            } else {
                "\t".to_string()
            };
            let (logger, handle) = make_cli_logger();
            lua::sf1::batch_format(Path::new(&args[2]), &unit, &logger)?;
            drop(logger);
            let _ = handle.join();
        }

        "format_stylua" => {
            if args.len() < 3 {
                eprintln!("Usage: SFTool format_stylua <scripts_dir> [stylua_exe] [--spaces]");
                return Ok(());
            }
            let stylua = args
                .get(3)
                .map(PathBuf::from)
                .unwrap_or_else(|| tools::find_tool("stylua.exe"));
            let use_tabs = !args.iter().any(|a| a == "--spaces");
            let (logger, handle) = make_cli_logger();
            lua::sf2::batch_format_stylua(Path::new(&args[2]), &stylua, use_tabs, &logger)?;
            drop(logger);
            let _ = handle.join();
        }

        "create_map" => {
            if args.len() < 5 {
                eprintln!("Usage: SFTool create_map <target_dir> <project_name> <map_name>");
                return Ok(());
            }
            let path = lua::sf2::create_map_scaffolding(
                Path::new(&args[2]),
                args[3].as_str(),
                args[4].as_str(),
            )?;
            println!("[+] Map project scaffold created at: {:?}", path);
        }

        "export_emmylua" => {
            if args.len() < 3 {
                eprintln!("Usage: SFTool export_emmylua <out_file.lua>");
                return Ok(());
            }
            let count = lua::sf2_api::export_emmylua_definitions(Path::new(&args[2]))?;
            println!(
                "[+] Exported {} EmmyLua definitions to {:?}",
                count, args[2]
            );
        }

        unknown => {
            eprintln!("[!] Unknown command: '{}'", unknown);
            print_help();
        }
    }
    Ok(())
}
