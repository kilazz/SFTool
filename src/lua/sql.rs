// src/lua/sql.rs

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

// -----------------------------------------------------------------------------
// DATA MODELS
// -----------------------------------------------------------------------------

/// Entry from script/sql_item.lua
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SqlItemEntry {
    pub id: u32,
    pub mesh_male_cold: String,
    pub mesh_female_cold: String,
    pub mesh_male_warm: String,
    pub mesh_female_warm: String,
    pub shadow_rng: f64,
    pub selection_size: f64,
    pub anim_set: String,
    pub race: u32,
    pub category: u32,
    pub sub_category: u32,
}

impl Default for SqlItemEntry {
    fn default() -> Self {
        Self {
            id: 0,
            mesh_male_cold: "<undefined>".into(),
            mesh_female_cold: "<undefined>".into(),
            mesh_male_warm: "<undefined>".into(),
            mesh_female_warm: "<undefined>".into(),
            shadow_rng: 0.0,
            selection_size: 1.0,
            anim_set: "normal".into(),
            race: 0,
            category: 0,
            sub_category: 0,
        }
    }
}

/// Entry from script/sql_building.lua
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SqlBuildingEntry {
    pub id: u32,
    pub meshes: Vec<String>,
    pub selection_scaling: f64,
}

impl Default for SqlBuildingEntry {
    fn default() -> Self {
        Self {
            id: 0,
            meshes: Vec::new(),
            selection_scaling: 1.0,
        }
    }
}

/// Entry from script/sql_object.lua
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SqlObjectEntry {
    pub id: u32,
    pub name: String,
    pub meshes: Vec<String>,
    pub shadow: bool,
    pub billboarded: bool,
    pub scale: f64,
    pub selection_scaling: f64,
}

impl Default for SqlObjectEntry {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            meshes: Vec::new(),
            shadow: false,
            billboarded: false,
            scale: 100.0,
            selection_scaling: 1.0,
        }
    }
}

/// Entry from script/sql_head.lua
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct SqlHeadEntry {
    pub id: u32,
    pub mesh_male: String,
    pub mesh_female: String,
}

// -----------------------------------------------------------------------------
// TOLERANT TOKEN SCANNER & PARSER
// -----------------------------------------------------------------------------

/// Extracts clean string contents inside quotes (both ' and ")
fn unquote(s: &str) -> String {
    let trimmed = s.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        if trimmed.len() >= 2 {
            trimmed[1..trimmed.len() - 1].to_string()
        } else {
            String::new()
        }
    } else {
        trimmed.to_string()
    }
}

/// Parses nested Lua table arrays of strings like `{ "mesh1", "mesh2" }`
fn parse_lua_string_array(body: &str) -> Vec<String> {
    let mut list = Vec::new();
    let trimmed = body.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        if !trimmed.is_empty() {
            list.push(unquote(trimmed));
        }
        return list;
    }

    let inner = &trimmed[1..trimmed.len() - 1];
    for token in inner.split(',') {
        let clean = unquote(token);
        if !clean.is_empty() {
            list.push(clean);
        }
    }
    list
}

/// Parses key-value pairs inside a single Lua table `{ Key = Value, ... }`
fn parse_key_value_block(block: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let mut in_quote = false;
    let mut quote_char = '"';
    let mut depth = 0;
    let mut current = String::new();

    for c in block.chars() {
        match c {
            '"' | '\'' if !in_quote => {
                in_quote = true;
                quote_char = c;
                current.push(c);
            }
            c if in_quote && c == quote_char => {
                in_quote = false;
                current.push(c);
            }
            '{' if !in_quote => {
                depth += 1;
                current.push(c);
            }
            '}' if !in_quote => {
                depth -= 1;
                current.push(c);
            }
            ',' | ';' if !in_quote && depth == 0 => {
                process_field_assignment(&current, &mut map);
                current.clear();
            }
            '\n' | '\r' if !in_quote && depth == 0 => {
                if !current.trim().is_empty() && current.contains('=') {
                    process_field_assignment(&current, &mut map);
                    current.clear();
                }
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.trim().is_empty() {
        process_field_assignment(&current, &mut map);
    }

    map
}

fn process_field_assignment(field_str: &str, map: &mut BTreeMap<String, String>) {
    let parts: Vec<&str> = field_str.splitn(2, '=').collect();
    if parts.len() == 2 {
        let key = parts[0].trim().to_string();
        let val = parts[1].trim().to_string();
        if !key.is_empty() {
            map.insert(key, val);
        }
    }
}

/// Fast stream extractor for blocks: `[ <id> ] = { <fields> }`
fn extract_raw_entries(content: &str) -> Vec<(u32, String)> {
    let mut entries = Vec::new();
    let chars: Vec<char> = content.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '[' {
            let id_start = i + 1;
            let mut id_end = id_start;
            while id_end < len && chars[id_end] != ']' {
                id_end += 1;
            }

            if id_end < len {
                let id_str: String = chars[id_start..id_end].iter().collect();
                if let Ok(id_val) = id_str.trim().parse::<u32>() {
                    let mut cursor = id_end + 1;
                    while cursor < len && (chars[cursor].is_whitespace() || chars[cursor] == '=') {
                        cursor += 1;
                    }

                    if cursor < len && chars[cursor] == '{' {
                        let block_start = cursor + 1;
                        let mut depth = 1;
                        let mut block_end = block_start;

                        let mut in_str = false;
                        let mut str_q = '"';

                        while block_end < len && depth > 0 {
                            let c = chars[block_end];
                            match c {
                                '"' | '\'' if !in_str => {
                                    in_str = true;
                                    str_q = c;
                                }
                                c if in_str && c == str_q => {
                                    in_str = false;
                                }
                                '{' if !in_str => depth += 1,
                                '}' if !in_str => depth -= 1,
                                _ => {}
                            }
                            block_end += 1;
                        }

                        if depth == 0 {
                            let block_text: String =
                                chars[block_start..block_end - 1].iter().collect();
                            entries.push((id_val, block_text));
                            i = block_end;
                            continue;
                        }
                    }
                }
            }
        }
        i += 1;
    }

    entries
}

// -----------------------------------------------------------------------------
// LOAD & SAVE HANDLERS
// -----------------------------------------------------------------------------

// --- 1. SQL ITEMS (sql_item.lua) ---

pub fn load_sql_items(path: &Path) -> io::Result<BTreeMap<u32, SqlItemEntry>> {
    let text = fs::read_to_string(path)?;
    let raw = extract_raw_entries(&text);
    let mut map = BTreeMap::new();

    for (id, block) in raw {
        let fields = parse_key_value_block(&block);
        let entry = SqlItemEntry {
            id,
            mesh_male_cold: fields
                .get("MeshMaleCold")
                .map(|s| unquote(s))
                .unwrap_or_else(|| "<undefined>".into()),
            mesh_female_cold: fields
                .get("MeshFemaleCold")
                .map(|s| unquote(s))
                .unwrap_or_else(|| "<undefined>".into()),
            mesh_male_warm: fields
                .get("MeshMaleWarm")
                .map(|s| unquote(s))
                .unwrap_or_else(|| "<undefined>".into()),
            mesh_female_warm: fields
                .get("MeshFemaleWarm")
                .map(|s| unquote(s))
                .unwrap_or_else(|| "<undefined>".into()),
            shadow_rng: fields
                .get("ShadowRNG")
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0),
            selection_size: fields
                .get("SelectionSize")
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(1.0),
            anim_set: fields
                .get("AnimSet")
                .map(|s| unquote(s))
                .unwrap_or_else(|| "normal".into()),
            race: fields
                .get("Race")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0),
            category: fields
                .get("Category")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0),
            sub_category: fields
                .get("SubCategory")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0),
        };
        map.insert(id, entry);
    }

    Ok(map)
}

pub fn save_sql_items(path: &Path, items: &BTreeMap<u32, SqlItemEntry>) -> io::Result<()> {
    let mut f = File::create(path)?;
    writeln!(f, "-- Generated by SFTool: Visual Item Bindings")?;
    writeln!(f, "return {{")?;

    for (id, it) in items {
        writeln!(f, "\t[{}] = {{", id)?;
        writeln!(f, "\t\tMeshMaleCold = \"{}\",", it.mesh_male_cold)?;
        writeln!(f, "\t\tMeshFemaleCold = \"{}\",", it.mesh_female_cold)?;
        writeln!(f, "\t\tMeshMaleWarm = \"{}\",", it.mesh_male_warm)?;
        writeln!(f, "\t\tMeshFemaleWarm = \"{}\",", it.mesh_female_warm)?;
        writeln!(f, "\t\tShadowRNG = {:.6},", it.shadow_rng)?;
        writeln!(f, "\t\tSelectionSize = {:.6},", it.selection_size)?;
        writeln!(f, "\t\tAnimSet = \"{}\",", it.anim_set)?;
        writeln!(f, "\t\tRace = {},", it.race)?;
        writeln!(f, "\t\tCategory = {},", it.category)?;
        writeln!(f, "\t\tSubCategory = {},", it.sub_category)?;
        writeln!(f, "\t}},")?;
    }

    writeln!(f, "}}")?;
    Ok(())
}

// --- 2. SQL BUILDINGS (sql_building.lua) ---

pub fn load_sql_buildings(path: &Path) -> io::Result<BTreeMap<u32, SqlBuildingEntry>> {
    let text = fs::read_to_string(path)?;
    let raw = extract_raw_entries(&text);
    let mut map = BTreeMap::new();

    for (id, block) in raw {
        let fields = parse_key_value_block(&block);
        let meshes = fields
            .get("Mesh")
            .map(|s| parse_lua_string_array(s))
            .unwrap_or_default();
        let selection_scaling = fields
            .get("SelectionScaling")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(1.0);

        map.insert(
            id,
            SqlBuildingEntry {
                id,
                meshes,
                selection_scaling,
            },
        );
    }

    Ok(map)
}

pub fn save_sql_buildings(
    path: &Path,
    buildings: &BTreeMap<u32, SqlBuildingEntry>,
) -> io::Result<()> {
    let mut f = File::create(path)?;
    writeln!(f, "-- Generated by SFTool: Visual Building Bindings")?;
    writeln!(f, "return {{")?;

    for (id, b) in buildings {
        let mesh_str: Vec<String> = b.meshes.iter().map(|m| format!("\"{}\"", m)).collect();
        writeln!(f, "\t[{}] = {{", id)?;
        writeln!(f, "\t\tMesh = {{ {} }},", mesh_str.join(", "))?;
        writeln!(f, "\t\tSelectionScaling = {:.6},", b.selection_scaling)?;
        writeln!(f, "\t}},")?;
    }

    writeln!(f, "}}")?;
    Ok(())
}

// --- 3. SQL OBJECTS (sql_object.lua) ---

pub fn load_sql_objects(path: &Path) -> io::Result<BTreeMap<u32, SqlObjectEntry>> {
    let text = fs::read_to_string(path)?;
    let raw = extract_raw_entries(&text);
    let mut map = BTreeMap::new();

    for (id, block) in raw {
        let fields = parse_key_value_block(&block);
        let name = fields.get("Name").map(|s| unquote(s)).unwrap_or_default();
        let meshes = fields
            .get("Mesh")
            .map(|s| parse_lua_string_array(s))
            .unwrap_or_default();
        let shadow = fields
            .get("Shadow")
            .map(|s| s.trim() == "true" || s.trim() == "1")
            .unwrap_or(false);
        let billboarded = fields
            .get("Billboarded")
            .map(|s| s.trim() == "true" || s.trim() == "1")
            .unwrap_or(false);
        let scale = fields
            .get("Scale")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(100.0);
        let selection_scaling = fields
            .get("SelectionScaling")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(1.0);

        map.insert(
            id,
            SqlObjectEntry {
                id,
                name,
                meshes,
                shadow,
                billboarded,
                scale,
                selection_scaling,
            },
        );
    }

    Ok(map)
}

pub fn save_sql_objects(path: &Path, objects: &BTreeMap<u32, SqlObjectEntry>) -> io::Result<()> {
    let mut f = File::create(path)?;
    writeln!(f, "-- Generated by SFTool: Visual Object Bindings")?;
    writeln!(f, "return {{")?;

    for (id, obj) in objects {
        let mesh_str: Vec<String> = obj.meshes.iter().map(|m| format!("\"{}\"", m)).collect();
        writeln!(f, "\t[{}] = {{", id)?;
        writeln!(f, "\t\tName = \"{}\",", obj.name)?;
        writeln!(f, "\t\tMesh = {{ {} }},", mesh_str.join(", "))?;
        writeln!(
            f,
            "\t\tShadow = {},",
            if obj.shadow { "true" } else { "false" }
        )?;
        writeln!(
            f,
            "\t\tBillboarded = {},",
            if obj.billboarded { "true" } else { "false" }
        )?;
        writeln!(f, "\t\tScale = {:.2},", obj.scale)?;
        writeln!(f, "\t\tSelectionScaling = {:.6},", obj.selection_scaling)?;
        writeln!(f, "\t}},")?;
    }

    writeln!(f, "}}")?;
    Ok(())
}

// --- 4. SQL HEADS (sql_head.lua) ---

pub fn load_sql_heads(path: &Path) -> io::Result<BTreeMap<u32, SqlHeadEntry>> {
    let text = fs::read_to_string(path)?;
    let raw = extract_raw_entries(&text);
    let mut map = BTreeMap::new();

    for (id, block) in raw {
        let fields = parse_key_value_block(&block);
        let mesh_male = fields
            .get("MeshMale")
            .map(|s| unquote(s))
            .unwrap_or_default();
        let mesh_female = fields
            .get("MeshFemale")
            .map(|s| unquote(s))
            .unwrap_or_default();

        map.insert(
            id,
            SqlHeadEntry {
                id,
                mesh_male,
                mesh_female,
            },
        );
    }

    Ok(map)
}

pub fn save_sql_heads(path: &Path, heads: &BTreeMap<u32, SqlHeadEntry>) -> io::Result<()> {
    let mut f = File::create(path)?;
    writeln!(f, "-- Generated by SFTool: Visual Head Mesh Bindings")?;
    writeln!(f, "return {{")?;

    for (id, h) in heads {
        writeln!(f, "\t[{}] = {{", id)?;
        writeln!(f, "\t\tMeshMale = \"{}\",", h.mesh_male)?;
        writeln!(f, "\t\tMeshFemale = \"{}\",", h.mesh_female)?;
        writeln!(f, "\t}},")?;
    }

    writeln!(f, "}}")?;
    Ok(())
}
