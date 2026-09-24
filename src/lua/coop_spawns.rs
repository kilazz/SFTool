// src/lua/coop_spawns.rs

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CoopSpawnWave {
    pub seconds_per_tick: u32,
    pub units: Vec<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CoopSpawnGroup {
    pub id: u32,
    pub name: String,
    pub level_range: String,
    pub goal: String, // e.g. "GoalCoopAggressive", "GoalCoopDefensive"
    pub max_units: u32,
    pub start_units: Vec<u32>,
    pub waves: BTreeMap<u32, CoopSpawnWave>, // Key: wave activation minute
}

pub fn parse_coop_spawns(path: &Path) -> io::Result<BTreeMap<u32, CoopSpawnGroup>> {
    let content = fs::read_to_string(path)?;
    let mut groups = BTreeMap::new();
    let mut current_group: Option<CoopSpawnGroup> = None;
    let mut in_data_block = false;
    let mut current_wave_min = 0u32;
    let mut current_wave_tick = 60u32;

    for line in content.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.contains("] =") && !in_data_block {
            if let Some(grp) = current_group.take() {
                groups.insert(grp.id, grp);
            }
            let num_str: String = t
                .chars()
                .skip_while(|&c| c != '[')
                .skip(1)
                .take_while(|&c| c != ']')
                .collect();
            let id = num_str.trim().parse::<u32>().unwrap_or(0);
            current_group = Some(CoopSpawnGroup {
                id,
                name: String::new(),
                level_range: "1-10".into(),
                goal: "GoalCoopAggressive".into(),
                max_units: 20,
                start_units: Vec::new(),
                waves: BTreeMap::new(),
            });
            continue;
        }

        let Some(grp) = current_group.as_mut() else {
            continue;
        };

        if t.starts_with("Name =") {
            grp.name = t
                .split('=')
                .nth(1)
                .unwrap_or("")
                .trim()
                .trim_matches('"')
                .trim_matches(',')
                .trim()
                .to_string();
        } else if t.starts_with("LevelRange =") {
            grp.level_range = t
                .split('=')
                .nth(1)
                .unwrap_or("")
                .trim()
                .trim_matches('"')
                .trim_matches(',')
                .trim()
                .to_string();
        } else if t.starts_with("Goal =") {
            grp.goal = t
                .split('=')
                .nth(1)
                .unwrap_or("")
                .trim()
                .trim_matches(',')
                .trim()
                .to_string();
        } else if t.starts_with("MaxClanSize =") || t.starts_with("MaxUnits =") {
            if let Some(val) = t.split('=').nth(1) {
                grp.max_units = val.trim().trim_matches(',').parse::<u32>().unwrap_or(20);
            }
        } else if t.starts_with("StartUnits =") {
            let slice = t
                .split('{')
                .nth(1)
                .unwrap_or("")
                .trim_end_matches("},")
                .trim_end_matches('}');
            grp.start_units = slice
                .split(',')
                .filter_map(|s| s.trim().parse::<u32>().ok())
                .collect();
        } else if t.starts_with("Data =") {
            in_data_block = true;
        } else if in_data_block {
            if t.starts_with('[') && t.contains("] =") {
                let m_str: String = t
                    .chars()
                    .skip_while(|&c| c != '[')
                    .skip(1)
                    .take_while(|&c| c != ']')
                    .collect();
                current_wave_min = m_str.trim().parse::<u32>().unwrap_or(0);
            } else if t.starts_with("SecondsPerTick =") {
                if let Some(s) = t.split('=').nth(1) {
                    current_wave_tick = s.trim().trim_matches(',').parse::<u32>().unwrap_or(60);
                }
            } else if t.starts_with("Units =") {
                let slice = t
                    .split('{')
                    .nth(1)
                    .unwrap_or("")
                    .trim_end_matches("},")
                    .trim_end_matches('}');
                let wave_units: Vec<u32> = slice
                    .split(',')
                    .filter_map(|s| s.trim().parse::<u32>().ok())
                    .collect();
                grp.waves.insert(
                    current_wave_min,
                    CoopSpawnWave {
                        seconds_per_tick: current_wave_tick,
                        units: wave_units,
                    },
                );
            } else if t.starts_with("},") || t == "}" {
                in_data_block = false;
            }
        }
    }

    if let Some(grp) = current_group {
        groups.insert(grp.id, grp);
    }

    Ok(groups)
}

pub fn save_coop_spawns(path: &Path, groups: &BTreeMap<u32, CoopSpawnGroup>) -> io::Result<()> {
    let mut f = File::create(path)?;
    writeln!(
        f,
        "-- Generated by SFTool: SpellForce RTS Co-op Spawn Groups"
    )?;
    writeln!(f, "return {{")?;

    for (id, grp) in groups {
        writeln!(f, "\t[{}] = {{", id)?;
        writeln!(f, "\t\tName = \"{}\",", grp.name)?;
        writeln!(f, "\t\tLevelRange = \"{}\",", grp.level_range)?;
        writeln!(f, "\t\tGoal = {},", grp.goal)?;
        writeln!(f, "\t\tMaxClanSize = {},", grp.max_units)?;

        let start_str: Vec<String> = grp.start_units.iter().map(|u| u.to_string()).collect();
        writeln!(f, "\t\tStartUnits = {{ {} }},", start_str.join(", "))?;

        writeln!(f, "\t\tData = {{")?;
        for (minute, wave) in &grp.waves {
            writeln!(f, "\t\t\t[{}] = {{", minute)?;
            writeln!(f, "\t\t\t\tSecondsPerTick = {},", wave.seconds_per_tick)?;
            let wave_u_str: Vec<String> = wave.units.iter().map(|u| u.to_string()).collect();
            writeln!(f, "\t\t\t\tUnits = {{ {} }},", wave_u_str.join(", "))?;
            writeln!(f, "\t\t\t}},")?;
        }
        writeln!(f, "\t\t}},")?;
        writeln!(f, "\t}},")?;
    }

    writeln!(f, "}}")?;
    Ok(())
}
