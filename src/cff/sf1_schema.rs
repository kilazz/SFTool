// src/cff/sf1_schema.rs

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Cursor, Read, Write};

pub trait Sf1Record: Sized {
    const STRIDE: usize;
    fn decode(bytes: &[u8]) -> io::Result<Self>;
    fn encode(&self, out: &mut [u8]) -> io::Result<()>;
}

// =============================================================================
// CATEGORY 2002 (0x07D2) - SpellsMaster (Verified 76 Bytes Stride)
// =============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpellSkillReq {
    pub school: u8,
    pub sub_school: u8,
    pub level: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpellEntry {
    pub spell_id: u16,
    pub spell_line_id: u16,
    pub skill_reqs: [u8; 12],
    pub mana_cost: u16,
    pub cast_time_ms: u32,
    pub recast_time_ms: u32,
    pub min_range: u16,
    pub max_range: u16,
    pub cast_target_faction: u8,
    pub cast_target_mode: u8,
    pub params: [u32; 10],
    pub effect_power: u16,
    pub effect_range: u16,
}

impl Sf1Record for SpellEntry {
    const STRIDE: usize = 76;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        if bytes.len() < Self::STRIDE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Buffer too short for SpellEntry (expected 76 bytes)",
            ));
        }
        let mut cur = Cursor::new(bytes);
        let spell_id = cur.read_u16::<LittleEndian>()?;
        let spell_line_id = cur.read_u16::<LittleEndian>()?;

        let mut skill_reqs = [0u8; 12];
        cur.read_exact(&mut skill_reqs)?;

        let mana_cost = cur.read_u16::<LittleEndian>()?;
        let cast_time_ms = cur.read_u32::<LittleEndian>()?;
        let recast_time_ms = cur.read_u32::<LittleEndian>()?;
        let min_range = cur.read_u16::<LittleEndian>()?;
        let max_range = cur.read_u16::<LittleEndian>()?;
        let cast_target_faction = cur.read_u8()?;
        let cast_target_mode = cur.read_u8()?;

        let mut params = [0u32; 10];
        for p in &mut params {
            *p = cur.read_u32::<LittleEndian>()?;
        }

        let effect_power = cur.read_u16::<LittleEndian>()?;
        let effect_range = cur.read_u16::<LittleEndian>()?;

        Ok(Self {
            spell_id,
            spell_line_id,
            skill_reqs,
            mana_cost,
            cast_time_ms,
            recast_time_ms,
            min_range,
            max_range,
            cast_target_faction,
            cast_target_mode,
            params,
            effect_power,
            effect_range,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        if out.len() < Self::STRIDE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Buffer too short for SpellEntry (expected 76 bytes)",
            ));
        }
        let mut cur = Cursor::new(out);
        cur.write_u16::<LittleEndian>(self.spell_id)?;
        cur.write_u16::<LittleEndian>(self.spell_line_id)?;
        cur.write_all(&self.skill_reqs)?;
        cur.write_u16::<LittleEndian>(self.mana_cost)?;
        cur.write_u32::<LittleEndian>(self.cast_time_ms)?;
        cur.write_u32::<LittleEndian>(self.recast_time_ms)?;
        cur.write_u16::<LittleEndian>(self.min_range)?;
        cur.write_u16::<LittleEndian>(self.max_range)?;
        cur.write_u8(self.cast_target_faction)?;
        cur.write_u8(self.cast_target_mode)?;
        for p in self.params {
            cur.write_u32::<LittleEndian>(p)?;
        }
        cur.write_u16::<LittleEndian>(self.effect_power)?;
        cur.write_u16::<LittleEndian>(self.effect_range)?;
        Ok(())
    }
}

impl SpellEntry {
    pub fn get_parsed_skills(&self) -> Vec<SpellSkillReq> {
        let mut reqs = Vec::new();
        for chunk in self.skill_reqs.as_chunks::<3>().0 {
            if chunk[0] != 0 {
                reqs.push(SpellSkillReq {
                    school: chunk[0],
                    sub_school: chunk[1],
                    level: chunk[2],
                });
            }
        }
        reqs
    }

    pub fn format_skill_reqs(&self) -> String {
        let reqs = self.get_parsed_skills();
        if reqs.is_empty() {
            return "None".to_string();
        }

        reqs.iter()
            .map(|r| {
                let school_str = match r.school {
                    1 => "LCA",
                    2 => "HCA",
                    3 => "Ranged",
                    4 => match r.sub_school {
                        1 => "White[Life]",
                        2 => "White[Nature]",
                        3 => "White[Boon]",
                        _ => "White Magic",
                    },
                    5 => match r.sub_school {
                        1 => "Elem[Fire]",
                        2 => "Elem[Ice]",
                        3 => "Elem[Earth]",
                        _ => "Elemental",
                    },
                    6 => match r.sub_school {
                        1 => "Mind[Enchant]",
                        2 => "Mind[Offensive]",
                        3 => "Mind[Defensive]",
                        _ => "Mind Magic",
                    },
                    7 => match r.sub_school {
                        1 => "Black[Death]",
                        2 => "Black[Necro]",
                        3 => "Black[Curse]",
                        _ => "Black Magic",
                    },
                    _ => "Unknown",
                };
                format!("{} {}", school_str, r.level)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn format_target_faction(&self) -> &'static str {
        match self.cast_target_faction {
            1 => "Enemy",
            2 => "Ally",
            3 => "Other/Neutral",
            _ => "Unknown",
        }
    }

    pub fn format_target_mode(&self) -> &'static str {
        match self.cast_target_mode {
            1 => "Figure",
            2 => "Building",
            3 => "Object",
            4 => "In World",
            5 => "In Area",
            _ => "Unknown",
        }
    }
}

// =============================================================================
// CATEGORY 2054 (0x0806) - SpellLines (Verified 75 Bytes Stride)
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct SpellLineEntry {
    pub line_id: u16,
    pub name_id: u16,
    pub line_flags: u8,
    pub school_id: u8,
    pub sub_school_id: u8,
    pub max_level: u8,
    pub ui_order: u8,
    pub icon_name: String,
    pub description_id: u16,
}

impl Sf1Record for SpellLineEntry {
    const STRIDE: usize = 75;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        if bytes.len() < Self::STRIDE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Buffer too short for SpellLineEntry (expected 75 bytes)",
            ));
        }
        let mut cur = Cursor::new(bytes);
        let line_id = cur.read_u16::<LittleEndian>()?;
        let name_id = cur.read_u16::<LittleEndian>()?;
        let line_flags = cur.read_u8()?;
        let school_id = cur.read_u8()?;
        let sub_school_id = cur.read_u8()?;
        let max_level = cur.read_u8()?;
        let ui_order = cur.read_u8()?;

        let mut icon_buf = [0u8; 64];
        cur.read_exact(&mut icon_buf)?;
        let end = icon_buf.iter().position(|&b| b == 0).unwrap_or(64);
        let icon_name = crate::cff::decode_windows(&icon_buf[..end]);

        let description_id = cur.read_u16::<LittleEndian>()?;

        Ok(Self {
            line_id,
            name_id,
            line_flags,
            school_id,
            sub_school_id,
            max_level,
            ui_order,
            icon_name,
            description_id,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        if out.len() < Self::STRIDE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Buffer too short for SpellLineEntry (expected 75 bytes)",
            ));
        }
        let mut cur = Cursor::new(out);
        cur.write_u16::<LittleEndian>(self.line_id)?;
        cur.write_u16::<LittleEndian>(self.name_id)?;
        cur.write_u8(self.line_flags)?;
        cur.write_u8(self.school_id)?;
        cur.write_u8(self.sub_school_id)?;
        cur.write_u8(self.max_level)?;
        cur.write_u8(self.ui_order)?;

        let mut icon_buf = [0u8; 64];
        let enc = crate::cff::encode_windows(&self.icon_name);
        let len = enc.len().min(63);
        icon_buf[..len].copy_from_slice(&enc[..len]);
        cur.write_all(&icon_buf)?;

        cur.write_u16::<LittleEndian>(self.description_id)?;
        Ok(())
    }
}

impl SpellLineEntry {
    pub fn format_school(&self) -> &'static str {
        match self.school_id {
            0 => "White / Combat",
            1 => "Elemental [Fire]",
            2 => "Elemental [Ice]",
            3 => "Black Magic",
            4 => "Mind Magic",
            5 => "Elemental [Earth]",
            _ => "General / Other",
        }
    }

    pub fn is_aura(&self) -> bool {
        (self.line_flags & 0x04) != 0 || self.line_flags == 0x1C
    }
}

// =============================================================================
// CATEGORY 2003 (0x07D3) - ItemsMaster
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct ItemMasterEntry {
    pub item_id: u16,
    pub item_type1: u8,
    pub item_type2: u8,
    pub name_id: u16,
    pub unit_stats_id: u16,
    pub army_unit_id: u16,
    pub building_id: u16,
    pub option_flags: u8,
    pub sell_value: u32,
    pub buy_value: u32,
    pub item_set_id: u8,
}

impl Sf1Record for ItemMasterEntry {
    const STRIDE: usize = 22;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        Ok(Self {
            item_id: cur.read_u16::<LittleEndian>()?,
            item_type1: cur.read_u8()?,
            item_type2: cur.read_u8()?,
            name_id: cur.read_u16::<LittleEndian>()?,
            unit_stats_id: cur.read_u16::<LittleEndian>()?,
            army_unit_id: cur.read_u16::<LittleEndian>()?,
            building_id: cur.read_u16::<LittleEndian>()?,
            option_flags: cur.read_u8()?,
            sell_value: cur.read_u32::<LittleEndian>()?,
            buy_value: cur.read_u32::<LittleEndian>()?,
            item_set_id: cur.read_u8()?,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        let mut cur = Cursor::new(out);
        cur.write_u16::<LittleEndian>(self.item_id)?;
        cur.write_u8(self.item_type1)?;
        cur.write_u8(self.item_type2)?;
        cur.write_u16::<LittleEndian>(self.name_id)?;
        cur.write_u16::<LittleEndian>(self.unit_stats_id)?;
        cur.write_u16::<LittleEndian>(self.army_unit_id)?;
        cur.write_u16::<LittleEndian>(self.building_id)?;
        cur.write_u8(self.option_flags)?;
        cur.write_u32::<LittleEndian>(self.sell_value)?;
        cur.write_u32::<LittleEndian>(self.buy_value)?;
        cur.write_u8(self.item_set_id)?;
        Ok(())
    }
}

// =============================================================================
// CATEGORY 2004 (0x07D4) - ItemStatsModifiers
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct ItemStatsModifierEntry {
    pub item_id: u16,
    pub strength: i16,
    pub stamina: i16,
    pub agility: i16,
    pub dexterity: i16,
    pub health: i16,
    pub charisma: i16,
    pub intelligence: i16,
    pub wisdom: i16,
    pub mana: i16,
    pub armor: i16,
    pub resist_fire: i16,
    pub resist_ice: i16,
    pub resist_black: i16,
    pub resist_mind: i16,
    pub speed_walk: i16,
    pub speed_fight: i16,
    pub speed_cast: i16,
}

impl Sf1Record for ItemStatsModifierEntry {
    const STRIDE: usize = 36;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        Ok(Self {
            item_id: cur.read_u16::<LittleEndian>()?,
            strength: cur.read_i16::<LittleEndian>()?,
            stamina: cur.read_i16::<LittleEndian>()?,
            agility: cur.read_i16::<LittleEndian>()?,
            dexterity: cur.read_i16::<LittleEndian>()?,
            health: cur.read_i16::<LittleEndian>()?,
            charisma: cur.read_i16::<LittleEndian>()?,
            intelligence: cur.read_i16::<LittleEndian>()?,
            wisdom: cur.read_i16::<LittleEndian>()?,
            mana: cur.read_i16::<LittleEndian>()?,
            armor: cur.read_i16::<LittleEndian>()?,
            resist_fire: cur.read_i16::<LittleEndian>()?,
            resist_ice: cur.read_i16::<LittleEndian>()?,
            resist_black: cur.read_i16::<LittleEndian>()?,
            resist_mind: cur.read_i16::<LittleEndian>()?,
            speed_walk: cur.read_i16::<LittleEndian>()?,
            speed_fight: cur.read_i16::<LittleEndian>()?,
            speed_cast: cur.read_i16::<LittleEndian>()?,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        let mut cur = Cursor::new(out);
        cur.write_u16::<LittleEndian>(self.item_id)?;
        cur.write_i16::<LittleEndian>(self.strength)?;
        cur.write_i16::<LittleEndian>(self.stamina)?;
        cur.write_i16::<LittleEndian>(self.agility)?;
        cur.write_i16::<LittleEndian>(self.dexterity)?;
        cur.write_i16::<LittleEndian>(self.health)?;
        cur.write_i16::<LittleEndian>(self.charisma)?;
        cur.write_i16::<LittleEndian>(self.intelligence)?;
        cur.write_i16::<LittleEndian>(self.wisdom)?;
        cur.write_i16::<LittleEndian>(self.mana)?;
        cur.write_i16::<LittleEndian>(self.armor)?;
        cur.write_i16::<LittleEndian>(self.resist_fire)?;
        cur.write_i16::<LittleEndian>(self.resist_ice)?;
        cur.write_i16::<LittleEndian>(self.resist_black)?;
        cur.write_i16::<LittleEndian>(self.resist_mind)?;
        cur.write_i16::<LittleEndian>(self.speed_walk)?;
        cur.write_i16::<LittleEndian>(self.speed_fight)?;
        cur.write_i16::<LittleEndian>(self.speed_cast)?;
        Ok(())
    }
}

// =============================================================================
// CATEGORY 2005 (0x07D5) - UnitStats (Verified 47 Bytes Stride)
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct UnitStatsEntry {
    pub stats_id: u16,
    pub unit_level: u16,
    pub unit_race: u8,
    pub agility: u16,
    pub dexterity: u16,
    pub charisma: u16,
    pub intelligence: u16,
    pub stamina: u16,
    pub strength: u16,
    pub wisdom: u16,
    pub random_init: u16,
    pub res_fire: u16,
    pub res_ice: u16,
    pub res_black: u16,
    pub res_mind: u16,
    pub speed_walk: u16,
    pub speed_fight: u16,
    pub speed_cast: u16,
    pub unit_size: u16,
    pub mana_usage: u16,
    pub spawn_base_time: u32,
    pub unit_flags: u8,
    pub head_id: u16,
    pub equipment_mode: u8,
}

impl Sf1Record for UnitStatsEntry {
    const STRIDE: usize = 47;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        Ok(Self {
            stats_id: cur.read_u16::<LittleEndian>()?,
            unit_level: cur.read_u16::<LittleEndian>()?,
            unit_race: cur.read_u8()?,
            agility: cur.read_u16::<LittleEndian>()?,
            dexterity: cur.read_u16::<LittleEndian>()?,
            charisma: cur.read_u16::<LittleEndian>()?,
            intelligence: cur.read_u16::<LittleEndian>()?,
            stamina: cur.read_u16::<LittleEndian>()?,
            strength: cur.read_u16::<LittleEndian>()?,
            wisdom: cur.read_u16::<LittleEndian>()?,
            random_init: cur.read_u16::<LittleEndian>()?,
            res_fire: cur.read_u16::<LittleEndian>()?,
            res_ice: cur.read_u16::<LittleEndian>()?,
            res_black: cur.read_u16::<LittleEndian>()?,
            res_mind: cur.read_u16::<LittleEndian>()?,
            speed_walk: cur.read_u16::<LittleEndian>()?,
            speed_fight: cur.read_u16::<LittleEndian>()?,
            speed_cast: cur.read_u16::<LittleEndian>()?,
            unit_size: cur.read_u16::<LittleEndian>()?,
            mana_usage: cur.read_u16::<LittleEndian>()?,
            spawn_base_time: cur.read_u32::<LittleEndian>()?,
            unit_flags: cur.read_u8()?,
            head_id: cur.read_u16::<LittleEndian>()?,
            equipment_mode: cur.read_u8()?,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        let mut cur = Cursor::new(out);
        cur.write_u16::<LittleEndian>(self.stats_id)?;
        cur.write_u16::<LittleEndian>(self.unit_level)?;
        cur.write_u8(self.unit_race)?;
        cur.write_u16::<LittleEndian>(self.agility)?;
        cur.write_u16::<LittleEndian>(self.dexterity)?;
        cur.write_u16::<LittleEndian>(self.charisma)?;
        cur.write_u16::<LittleEndian>(self.intelligence)?;
        cur.write_u16::<LittleEndian>(self.stamina)?;
        cur.write_u16::<LittleEndian>(self.strength)?;
        cur.write_u16::<LittleEndian>(self.wisdom)?;
        cur.write_u16::<LittleEndian>(self.random_init)?;
        cur.write_u16::<LittleEndian>(self.res_fire)?;
        cur.write_u16::<LittleEndian>(self.res_ice)?;
        cur.write_u16::<LittleEndian>(self.res_black)?;
        cur.write_u16::<LittleEndian>(self.res_mind)?;
        cur.write_u16::<LittleEndian>(self.speed_walk)?;
        cur.write_u16::<LittleEndian>(self.speed_fight)?;
        cur.write_u16::<LittleEndian>(self.speed_cast)?;
        cur.write_u16::<LittleEndian>(self.unit_size)?;
        cur.write_u16::<LittleEndian>(self.mana_usage)?;
        cur.write_u32::<LittleEndian>(self.spawn_base_time)?;
        cur.write_u8(self.unit_flags)?;
        cur.write_u16::<LittleEndian>(self.head_id)?;
        cur.write_u8(self.equipment_mode)?;
        Ok(())
    }
}

impl UnitStatsEntry {
    pub fn is_female(&self) -> bool {
        (self.unit_flags & 0x01) != 0
    }
    pub fn is_unkillable(&self) -> bool {
        (self.unit_flags & 0x02) != 0
    }
    pub fn calculate_effective_hp_and_mana(
        &self,
        health_factor: u16,
        mana_factor: u16,
    ) -> (u32, u32) {
        let hp = ((self.stamina as u32) * (health_factor as u32)) / 100;
        let mana = ((self.wisdom as u32) * (mana_factor as u32)) / 100;
        (hp, mana)
    }
}

// =============================================================================
// CATEGORY 2012 (0x07DC) - 2D Gfx Items
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct Gfx2dItemEntry {
    pub item_id: u16,
    pub flag: u8,
    pub mesh_name: String,
    pub extra: u16,
}

impl Sf1Record for Gfx2dItemEntry {
    const STRIDE: usize = 69;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let item_id = Cursor::new(&bytes[0..2]).read_u16::<LittleEndian>()?;
        let flag = bytes[2];
        let mesh_bytes = &bytes[3..67];
        let end = mesh_bytes
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(mesh_bytes.len());
        let mesh_name = crate::cff::decode_windows(&mesh_bytes[..end]);
        let extra = Cursor::new(&bytes[67..69]).read_u16::<LittleEndian>()?;

        Ok(Self {
            item_id,
            flag,
            mesh_name,
            extra,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        out[0..2].copy_from_slice(&self.item_id.to_le_bytes());
        out[2] = self.flag;
        let enc = crate::cff::encode_windows(&self.mesh_name);
        let mut padded = [0u8; 64];
        let len = enc.len().min(63);
        padded[..len].copy_from_slice(&enc[..len]);
        out[3..67].copy_from_slice(&padded);
        out[67..69].copy_from_slice(&self.extra.to_le_bytes());
        Ok(())
    }
}

// =============================================================================
// CATEGORY 2015 (0x07DF) - WeaponStats
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct WeaponStatsEntry {
    pub item_id: u16,
    pub min_damage: u16,
    pub max_damage: u16,
    pub min_range: u16,
    pub max_range: u16,
    pub speed: u16,
    pub weapon_type: u16,
    pub material: u16,
}

impl Sf1Record for WeaponStatsEntry {
    const STRIDE: usize = 16;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        Ok(Self {
            item_id: cur.read_u16::<LittleEndian>()?,
            min_damage: cur.read_u16::<LittleEndian>()?,
            max_damage: cur.read_u16::<LittleEndian>()?,
            min_range: cur.read_u16::<LittleEndian>()?,
            max_range: cur.read_u16::<LittleEndian>()?,
            speed: cur.read_u16::<LittleEndian>()?,
            weapon_type: cur.read_u16::<LittleEndian>()?,
            material: cur.read_u16::<LittleEndian>()?,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        let mut cur = Cursor::new(out);
        cur.write_u16::<LittleEndian>(self.item_id)?;
        cur.write_u16::<LittleEndian>(self.min_damage)?;
        cur.write_u16::<LittleEndian>(self.max_damage)?;
        cur.write_u16::<LittleEndian>(self.min_range)?;
        cur.write_u16::<LittleEndian>(self.max_range)?;
        cur.write_u16::<LittleEndian>(self.speed)?;
        cur.write_u16::<LittleEndian>(self.weapon_type)?;
        cur.write_u16::<LittleEndian>(self.material)?;
        Ok(())
    }
}

// =============================================================================
// CATEGORY 2018 (0x07E2) - SpellsBiMap
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct SpellsBiMapEntry {
    pub spell_id: u16,
    pub scroll_item_id: u16,
}

impl Sf1Record for SpellsBiMapEntry {
    const STRIDE: usize = 4;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        Ok(Self {
            spell_id: cur.read_u16::<LittleEndian>()?,
            scroll_item_id: cur.read_u16::<LittleEndian>()?,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        let mut cur = Cursor::new(out);
        cur.write_u16::<LittleEndian>(self.spell_id)?;
        cur.write_u16::<LittleEndian>(self.scroll_item_id)?;
        Ok(())
    }
}

// =============================================================================
// CATEGORY 2022 (0x07E6) - Races
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct RaceEntry {
    pub race_id: u8,
    pub vis_day: u8,
    pub vis_night: u8,
    pub hear_range: u8,
    pub aggro_factor: u8,
    pub moral: u8,
    pub aggressiveness: u8,
    pub text_id: u16,
    pub flags: u8,
    pub faction_id: u16,
    pub dmg_taken_blunt: u8,
    pub dmg_taken_slash: u8,
    pub ai_flags: u16,
    pub group_size_min: u8,
    pub group_size_max: u8,
    pub group_chance: u8,
    pub group_formation: u8,
    pub flee: u16,
    pub retreat_on_dmg: u16,
    pub retreat_follow: u16,
    pub attack_speed_factor: u8,
}

impl Sf1Record for RaceEntry {
    const STRIDE: usize = 27;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        Ok(Self {
            race_id: cur.read_u8()?,
            vis_day: cur.read_u8()?,
            vis_night: cur.read_u8()?,
            hear_range: cur.read_u8()?,
            aggro_factor: cur.read_u8()?,
            moral: cur.read_u8()?,
            aggressiveness: cur.read_u8()?,
            text_id: cur.read_u16::<LittleEndian>()?,
            flags: cur.read_u8()?,
            faction_id: cur.read_u16::<LittleEndian>()?,
            dmg_taken_blunt: cur.read_u8()?,
            dmg_taken_slash: cur.read_u8()?,
            ai_flags: cur.read_u16::<LittleEndian>()?,
            group_size_min: cur.read_u8()?,
            group_size_max: cur.read_u8()?,
            group_chance: cur.read_u8()?,
            group_formation: cur.read_u8()?,
            flee: cur.read_u16::<LittleEndian>()?,
            retreat_on_dmg: cur.read_u16::<LittleEndian>()?,
            retreat_follow: cur.read_u16::<LittleEndian>()?,
            attack_speed_factor: cur.read_u8()?,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        let mut cur = Cursor::new(out);
        cur.write_u8(self.race_id)?;
        cur.write_u8(self.vis_day)?;
        cur.write_u8(self.vis_night)?;
        cur.write_u8(self.hear_range)?;
        cur.write_u8(self.aggro_factor)?;
        cur.write_u8(self.moral)?;
        cur.write_u8(self.aggressiveness)?;
        cur.write_u16::<LittleEndian>(self.text_id)?;
        cur.write_u8(self.flags)?;
        cur.write_u16::<LittleEndian>(self.faction_id)?;
        cur.write_u8(self.dmg_taken_blunt)?;
        cur.write_u8(self.dmg_taken_slash)?;
        cur.write_u16::<LittleEndian>(self.ai_flags)?;
        cur.write_u8(self.group_size_min)?;
        cur.write_u8(self.group_size_max)?;
        cur.write_u8(self.group_chance)?;
        cur.write_u8(self.group_formation)?;
        cur.write_u16::<LittleEndian>(self.flee)?;
        cur.write_u16::<LittleEndian>(self.retreat_on_dmg)?;
        cur.write_u16::<LittleEndian>(self.retreat_follow)?;
        cur.write_u8(self.attack_speed_factor)?;
        Ok(())
    }
}

// =============================================================================
// CATEGORY 2024 (0x07E8) - UnitsMaster (Verified 64 Bytes Stride)
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct UnitMasterEntry {
    pub unit_id: u16,
    pub name_id: u16,
    pub stats_id: u16,
    pub xp_gain: u32,
    pub xp_falloff: u16,
    pub copper: u32,
    pub raw_mid: [u8; 7],
    pub internal_name: String,
    pub spawn_flag: u8,
}

impl Sf1Record for UnitMasterEntry {
    const STRIDE: usize = 64;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        let unit_id = cur.read_u16::<LittleEndian>()?;
        let name_id = cur.read_u16::<LittleEndian>()?;
        let stats_id = cur.read_u16::<LittleEndian>()?;
        let xp_gain = cur.read_u32::<LittleEndian>()?;
        let xp_falloff = cur.read_u16::<LittleEndian>()?;
        let copper = cur.read_u32::<LittleEndian>()?;
        let mut raw_mid = [0u8; 7];
        cur.read_exact(&mut raw_mid)?;

        let mut name_buf = [0u8; 40];
        cur.read_exact(&mut name_buf)?;
        let end = name_buf.iter().position(|&b| b == 0).unwrap_or(40);
        let internal_name = crate::cff::decode_windows(&name_buf[..end]);

        let spawn_flag = cur.read_u8()?;

        Ok(Self {
            unit_id,
            name_id,
            stats_id,
            xp_gain,
            xp_falloff,
            copper,
            raw_mid,
            internal_name,
            spawn_flag,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        let mut cur = Cursor::new(out);
        cur.write_u16::<LittleEndian>(self.unit_id)?;
        cur.write_u16::<LittleEndian>(self.name_id)?;
        cur.write_u16::<LittleEndian>(self.stats_id)?;
        cur.write_u32::<LittleEndian>(self.xp_gain)?;
        cur.write_u16::<LittleEndian>(self.xp_falloff)?;
        cur.write_u32::<LittleEndian>(self.copper)?;
        cur.write_all(&self.raw_mid)?;

        let mut name_buf = [0u8; 40];
        let enc = crate::cff::encode_windows(&self.internal_name);
        let len = enc.len().min(39);
        name_buf[..len].copy_from_slice(&enc[..len]);
        cur.write_all(&name_buf)?;

        cur.write_u8(self.spawn_flag)?;
        Ok(())
    }
}

// =============================================================================
// CATEGORY 2025 (0x07E9) - UnitEquipment
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct UnitEquipmentEntry {
    pub unit_id: u16,
    pub equipment_slot: u8,
    pub item_id: u16,
}

impl Sf1Record for UnitEquipmentEntry {
    const STRIDE: usize = 5;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        Ok(Self {
            unit_id: cur.read_u16::<LittleEndian>()?,
            equipment_slot: cur.read_u8()?,
            item_id: cur.read_u16::<LittleEndian>()?,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        let mut cur = Cursor::new(out);
        cur.write_u16::<LittleEndian>(self.unit_id)?;
        cur.write_u8(self.equipment_slot)?;
        cur.write_u16::<LittleEndian>(self.item_id)?;
        Ok(())
    }
}

// =============================================================================
// CATEGORY 2029 (0x07ED) - BuildingsMaster (Verified 23 Bytes Stride)
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct BuildingMasterEntry {
    pub building_id: u16,
    pub race_id: u8,
    pub can_enter: u8,
    pub slots: u8,
    pub health: u16,
    pub name_id: u16,
    pub rot_center_x: i16,
    pub rot_center_y: i16,
    pub num_of_polygons: u8,
    pub worker_cycle_time: u16,
    pub building_req_id: u16,
    pub initial_angle: u16,
    pub description_ext_id: u16,
    pub flags: u8,
}

impl Sf1Record for BuildingMasterEntry {
    const STRIDE: usize = 23;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        Ok(Self {
            building_id: cur.read_u16::<LittleEndian>()?,
            race_id: cur.read_u8()?,
            can_enter: cur.read_u8()?,
            slots: cur.read_u8()?,
            health: cur.read_u16::<LittleEndian>()?,
            name_id: cur.read_u16::<LittleEndian>()?,
            rot_center_x: cur.read_i16::<LittleEndian>()?,
            rot_center_y: cur.read_i16::<LittleEndian>()?,
            num_of_polygons: cur.read_u8()?,
            worker_cycle_time: cur.read_u16::<LittleEndian>()?,
            building_req_id: cur.read_u16::<LittleEndian>()?,
            initial_angle: cur.read_u16::<LittleEndian>()?,
            description_ext_id: cur.read_u16::<LittleEndian>()?,
            flags: cur.read_u8()?,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        let mut cur = Cursor::new(out);
        cur.write_u16::<LittleEndian>(self.building_id)?;
        cur.write_u8(self.race_id)?;
        cur.write_u8(self.can_enter)?;
        cur.write_u8(self.slots)?;
        cur.write_u16::<LittleEndian>(self.health)?;
        cur.write_u16::<LittleEndian>(self.name_id)?;
        cur.write_i16::<LittleEndian>(self.rot_center_x)?;
        cur.write_i16::<LittleEndian>(self.rot_center_y)?;
        cur.write_u8(self.num_of_polygons)?;
        cur.write_u16::<LittleEndian>(self.worker_cycle_time)?;
        cur.write_u16::<LittleEndian>(self.building_req_id)?;
        cur.write_u16::<LittleEndian>(self.initial_angle)?;
        cur.write_u16::<LittleEndian>(self.description_ext_id)?;
        cur.write_u8(self.flags)?;
        Ok(())
    }
}

// =============================================================================
// CATEGORY 2032 (0x07F0) - TerrainCultivation
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct TerrainCultivationEntry {
    pub terrain_id: u16,
    pub block_value: u8,
    pub cultivation_flags: u8,
}

impl Sf1Record for TerrainCultivationEntry {
    const STRIDE: usize = 4;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        Ok(Self {
            terrain_id: cur.read_u16::<LittleEndian>()?,
            block_value: cur.read_u8()?,
            cultivation_flags: cur.read_u8()?,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        let mut cur = Cursor::new(out);
        cur.write_u16::<LittleEndian>(self.terrain_id)?;
        cur.write_u8(self.block_value)?;
        cur.write_u8(self.cultivation_flags)?;
        Ok(())
    }
}

// =============================================================================
// CATEGORY 2040 (0x07F8) - UnitLootTables (Verified 11 Bytes Stride)
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct UnitLootTableEntry {
    pub unit_id: u16,
    pub slot: u8,
    pub item1: u16,
    pub chance1: u8,
    pub item2: u16,
    pub chance2: u8,
    pub item3: u16,
}

impl Sf1Record for UnitLootTableEntry {
    const STRIDE: usize = 11;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        Ok(Self {
            unit_id: cur.read_u16::<LittleEndian>()?,
            slot: cur.read_u8()?,
            item1: cur.read_u16::<LittleEndian>()?,
            chance1: cur.read_u8()?,
            item2: cur.read_u16::<LittleEndian>()?,
            chance2: cur.read_u8()?,
            item3: cur.read_u16::<LittleEndian>()?,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        let mut cur = Cursor::new(out);
        cur.write_u16::<LittleEndian>(self.unit_id)?;
        cur.write_u8(self.slot)?;
        cur.write_u16::<LittleEndian>(self.item1)?;
        cur.write_u8(self.chance1)?;
        cur.write_u16::<LittleEndian>(self.item2)?;
        cur.write_u8(self.chance2)?;
        cur.write_u16::<LittleEndian>(self.item3)?;
        Ok(())
    }
}

// =============================================================================
// CATEGORY 2042 (0x07FA) - MerchantInventory
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct MerchantInventoryEntry {
    pub merchant_id: u16,
    pub item_id: u16,
    pub stock: u16,
}

impl Sf1Record for MerchantInventoryEntry {
    const STRIDE: usize = 6;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        Ok(Self {
            merchant_id: cur.read_u16::<LittleEndian>()?,
            item_id: cur.read_u16::<LittleEndian>()?,
            stock: cur.read_u16::<LittleEndian>()?,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        let mut cur = Cursor::new(out);
        cur.write_u16::<LittleEndian>(self.merchant_id)?;
        cur.write_u16::<LittleEndian>(self.item_id)?;
        cur.write_u16::<LittleEndian>(self.stock)?;
        Ok(())
    }
}

// =============================================================================
// CATEGORY 2048 (0x0800) - ComplexProperties
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct ComplexPropertyEntry {
    pub level: u8,
    pub health_factor: u16,
    pub mana_factor: u16,
    pub experience_required: u32,
    pub attribute_point_limit: u8,
    pub skill_point_limit: u8,
    pub damage_factor: u16,
    pub armor_class_factor: u16,
}

impl Sf1Record for ComplexPropertyEntry {
    const STRIDE: usize = 15;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        Ok(Self {
            level: cur.read_u8()?,
            health_factor: cur.read_u16::<LittleEndian>()?,
            mana_factor: cur.read_u16::<LittleEndian>()?,
            experience_required: cur.read_u32::<LittleEndian>()?,
            attribute_point_limit: cur.read_u8()?,
            skill_point_limit: cur.read_u8()?,
            damage_factor: cur.read_u16::<LittleEndian>()?,
            armor_class_factor: cur.read_u16::<LittleEndian>()?,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        let mut cur = Cursor::new(out);
        cur.write_u8(self.level)?;
        cur.write_u16::<LittleEndian>(self.health_factor)?;
        cur.write_u16::<LittleEndian>(self.mana_factor)?;
        cur.write_u32::<LittleEndian>(self.experience_required)?;
        cur.write_u8(self.attribute_point_limit)?;
        cur.write_u8(self.skill_point_limit)?;
        cur.write_u16::<LittleEndian>(self.damage_factor)?;
        cur.write_u16::<LittleEndian>(self.armor_class_factor)?;
        Ok(())
    }
}

// =============================================================================
// CATEGORY 2050 (0x0802) - ObjectsMaster (Verified 54 Bytes Stride)
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectMasterEntry {
    pub object_id: u16,
    pub name_id: u16,
    pub flags: u8,
    pub flatten_mode: u8,
    pub polygon_num: u8,
    pub category_name: String,
    pub resource_amount: u16,
    pub width: u16,
    pub height: u16,
}

impl Sf1Record for ObjectMasterEntry {
    const STRIDE: usize = 54;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        let object_id = cur.read_u16::<LittleEndian>()?;
        let name_id = cur.read_u16::<LittleEndian>()?;
        let flags = cur.read_u8()?;
        let flatten_mode = cur.read_u8()?;
        let polygon_num = cur.read_u8()?;

        let mut str_buf = [0u8; 41];
        cur.read_exact(&mut str_buf)?;
        let end = str_buf.iter().position(|&b| b == 0).unwrap_or(41);
        let category_name = crate::cff::decode_windows(&str_buf[..end]);

        let resource_amount = cur.read_u16::<LittleEndian>()?;
        let width = cur.read_u16::<LittleEndian>()?;
        let height = cur.read_u16::<LittleEndian>()?;

        Ok(Self {
            object_id,
            name_id,
            flags,
            flatten_mode,
            polygon_num,
            category_name,
            resource_amount,
            width,
            height,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        let mut cur = Cursor::new(out);
        cur.write_u16::<LittleEndian>(self.object_id)?;
        cur.write_u16::<LittleEndian>(self.name_id)?;
        cur.write_u8(self.flags)?;
        cur.write_u8(self.flatten_mode)?;
        cur.write_u8(self.polygon_num)?;

        let mut str_buf = [0u8; 41];
        let enc = crate::cff::encode_windows(&self.category_name);
        let len = enc.len().min(40);
        str_buf[..len].copy_from_slice(&enc[..len]);
        cur.write_all(&str_buf)?;

        cur.write_u16::<LittleEndian>(self.resource_amount)?;
        cur.write_u16::<LittleEndian>(self.width)?;
        cur.write_u16::<LittleEndian>(self.height)?;
        Ok(())
    }
}

#[allow(dead_code)]
impl ObjectMasterEntry {
    pub fn blocks_terrain(&self) -> bool {
        (self.flags & 0x01) != 0
    }
    pub fn adjusts_height(&self) -> bool {
        (self.flags & 0x02) != 0
    }
    pub fn is_placeable(&self) -> bool {
        (self.flags & 0x04) != 0
    }
    pub fn contains_loot(&self) -> bool {
        (self.flags & 0x80) != 0
    }
}

// =============================================================================
// CATEGORY 2053 (0x0805) - Portals (Verified 13 Bytes Stride)
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct PortalEntry {
    pub portal_id: u16,
    pub map_id: u32,
    pub pos_x: u16,
    pub pos_y: u16,
    pub is_default: u8,
    pub name_id: u16,
}

impl Sf1Record for PortalEntry {
    const STRIDE: usize = 13;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        Ok(Self {
            portal_id: cur.read_u16::<LittleEndian>()?,
            map_id: cur.read_u32::<LittleEndian>()?,
            pos_x: cur.read_u16::<LittleEndian>()?,
            pos_y: cur.read_u16::<LittleEndian>()?,
            is_default: cur.read_u8()?,
            name_id: cur.read_u16::<LittleEndian>()?,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        let mut cur = Cursor::new(out);
        cur.write_u16::<LittleEndian>(self.portal_id)?;
        cur.write_u32::<LittleEndian>(self.map_id)?;
        cur.write_u16::<LittleEndian>(self.pos_x)?;
        cur.write_u16::<LittleEndian>(self.pos_y)?;
        cur.write_u8(self.is_default)?;
        cur.write_u16::<LittleEndian>(self.name_id)?;
        Ok(())
    }
}

// =============================================================================
// CATEGORY 2058 (0x080A) - Descriptions
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct DescriptionEntry {
    pub description_id: u16,
    pub text_id: u16,
}

impl Sf1Record for DescriptionEntry {
    const STRIDE: usize = 4;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        Ok(Self {
            description_id: cur.read_u16::<LittleEndian>()?,
            text_id: cur.read_u16::<LittleEndian>()?,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        let mut cur = Cursor::new(out);
        cur.write_u16::<LittleEndian>(self.description_id)?;
        cur.write_u16::<LittleEndian>(self.text_id)?;
        Ok(())
    }
}

// =============================================================================
// CATEGORY 2061 (0x080D) - Quests (Verified 17 Bytes Stride)
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct QuestEntry {
    pub quest_id: u32,
    pub parent_quest_id: u32,
    pub is_main_quest: u8,
    pub name_id: u16,
    pub description_id: u16,
    pub order_index: u32,
}

impl Sf1Record for QuestEntry {
    const STRIDE: usize = 17;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        Ok(Self {
            quest_id: cur.read_u32::<LittleEndian>()?,
            parent_quest_id: cur.read_u32::<LittleEndian>()?,
            is_main_quest: cur.read_u8()?,
            name_id: cur.read_u16::<LittleEndian>()?,
            description_id: cur.read_u16::<LittleEndian>()?,
            order_index: cur.read_u32::<LittleEndian>()?,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        let mut cur = Cursor::new(out);
        cur.write_u32::<LittleEndian>(self.quest_id)?;
        cur.write_u32::<LittleEndian>(self.parent_quest_id)?;
        cur.write_u8(self.is_main_quest)?;
        cur.write_u16::<LittleEndian>(self.name_id)?;
        cur.write_u16::<LittleEndian>(self.description_id)?;
        cur.write_u32::<LittleEndian>(self.order_index)?;
        Ok(())
    }
}

// =============================================================================
// CATEGORY 2063 (0x080F) & 2064 (0x0810) - Weapon Types & Materials
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct WeaponTypeEntry {
    pub type_id: u16,
    pub name_id: u16,
    pub sharpness: u8,
}

impl Sf1Record for WeaponTypeEntry {
    const STRIDE: usize = 5;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        Ok(Self {
            type_id: cur.read_u16::<LittleEndian>()?,
            name_id: cur.read_u16::<LittleEndian>()?,
            sharpness: cur.read_u8()?,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        let mut cur = Cursor::new(out);
        cur.write_u16::<LittleEndian>(self.type_id)?;
        cur.write_u16::<LittleEndian>(self.name_id)?;
        cur.write_u8(self.sharpness)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeaponMaterialEntry {
    pub material_id: u16,
    pub name_id: u16,
}

impl Sf1Record for WeaponMaterialEntry {
    const STRIDE: usize = 4;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        Ok(Self {
            material_id: cur.read_u16::<LittleEndian>()?,
            name_id: cur.read_u16::<LittleEndian>()?,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        let mut cur = Cursor::new(out);
        cur.write_u16::<LittleEndian>(self.material_id)?;
        cur.write_u16::<LittleEndian>(self.name_id)?;
        Ok(())
    }
}

// =============================================================================
// CATEGORY 2072 (0x0818) - ItemSets
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct ItemSetEntry {
    pub set_id: u8,
    pub description_id: u16,
    pub set_type: u8,
}

impl Sf1Record for ItemSetEntry {
    const STRIDE: usize = 4;

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        Ok(Self {
            set_id: cur.read_u8()?,
            description_id: cur.read_u16::<LittleEndian>()?,
            set_type: cur.read_u8()?,
        })
    }

    fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        let mut cur = Cursor::new(out);
        cur.write_u8(self.set_id)?;
        cur.write_u16::<LittleEndian>(self.description_id)?;
        cur.write_u8(self.set_type)?;
        Ok(())
    }
}

// =============================================================================
// DIPLOMACY & EQUIPMENT SLOTS HELPERS
// =============================================================================

pub const CLAN_NAMES: [&str; 32] = [
    "Neutral",
    "Friendly neutral [Humans]",
    "Friendly neutral [Elves]",
    "Neutral [Animals for meat]",
    "Friendly neutral [Dwarves]",
    "Hostile [Grargs]",
    "Hostile [Imperial]",
    "Hostile [Uroks]",
    "Hostile [Undead]",
    "Hostile [Monsters/Demons]",
    "Player",
    "Player Elves",
    "Player Humans",
    "Player Dwarves",
    "Player Orcs",
    "Player Trolls",
    "Player Darkelves",
    "Hostile [Animals]",
    "KillAll",
    "Hostile [Beastmen]",
    "Hostile [Gorge]",
    "Unknown 22",
    "Unknown 23",
    "Hostile [Blades]",
    "Unknown 25",
    "Hostile [Multiplayer enemies]",
    "Hostile [Ogres]",
    "Neutral [NPCs]",
    "Hostile [Soulforger]",
    "Hostile [Bloodash]",
    "Unknown 31",
    "Hostile [Dervish]",
];

pub fn get_clan_name(clan_id: u8) -> &'static str {
    if clan_id >= 1 && (clan_id as usize) <= CLAN_NAMES.len() {
        CLAN_NAMES[(clan_id - 1) as usize]
    } else {
        "Unknown Clan"
    }
}

pub fn decode_diplomacy_relation(val: u8) -> &'static str {
    match val {
        0 => "Neutral",
        100 => "Friendly",
        156 => "Hostile",
        _ => "Custom",
    }
}

pub const EQUIPMENT_SLOT_NAMES: [&str; 7] = [
    "Helmet (0)",
    "Right Hand / Weapon (1)",
    "Chest / Armor (2)",
    "Left Hand / Shield (3)",
    "Right Ring (4)",
    "Legs (5)",
    "Left Ring (6)",
];

pub fn get_equipment_slot_name(slot: u8) -> &'static str {
    if (slot as usize) < EQUIPMENT_SLOT_NAMES.len() {
        EQUIPMENT_SLOT_NAMES[slot as usize]
    } else {
        "Unknown Slot"
    }
}
