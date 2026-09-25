// src/sav/avatar.rs

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use memmap2::Mmap;
use std::fs::File;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct AvatarSkill {
    pub skill_id: u8,
    pub level: u8,
}

#[derive(Debug, Clone)]
pub struct AvatarEquipment {
    pub slot: u8,
    pub item_id: u16,
}

#[derive(Debug, Clone)]
pub struct AvatarSpell {
    pub slot: u8,
    pub spell_id: u16,
}

#[derive(Debug, Clone)]
pub struct AvatarSaveInfo {
    pub level: u16,
    pub race_id: u8,
    pub strength: u16,
    pub stamina: u16,
    pub agility: u16,
    pub dexterity: u16,
    pub intelligence: u16,
    pub wisdom: u16,
    pub charisma: u16,
    pub walk_speed: u16,
    pub fight_speed: u16,
    pub cast_speed: u16,
    pub skills: Vec<AvatarSkill>,
    pub equipment: Vec<AvatarEquipment>,
    pub spellbook: Vec<AvatarSpell>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AvatarStatsUpdate {
    pub level: u16,
    pub strength: u16,
    pub stamina: u16,
    pub agility: u16,
    pub dexterity: u16,
    pub intelligence: u16,
    pub wisdom: u16,
    pub charisma: u16,
}

pub fn inspect_avatar_save(sav_path: &Path) -> io::Result<Option<AvatarSaveInfo>> {
    let file = File::open(sav_path)?;
    let mmap = unsafe { Mmap::map(&file)? };

    if mmap.len() < 24 {
        return Ok(None);
    }

    let mut cur = Cursor::new(&mmap[..]);
    cur.set_position(20);

    let mut avatar_info: Option<AvatarSaveInfo> = None;
    let mut skills = Vec::new();
    let mut equipment = Vec::new();
    let mut spellbook = Vec::new();

    while (cur.position() as usize) + 12 <= mmap.len() {
        let chunk_id = cur.read_u16::<LittleEndian>()?;
        let _occurrence = cur.read_i16::<LittleEndian>()?;
        let is_packed = cur.read_i16::<LittleEndian>()?;
        let packed_size = cur.read_i32::<LittleEndian>()?;
        let _data_type = cur.read_i16::<LittleEndian>()?;

        if packed_size <= 0 {
            break;
        }

        let required = if is_packed == 0 {
            packed_size as usize
        } else {
            4 + (packed_size as usize)
        };
        if (cur.position() as usize) + required > mmap.len() {
            break;
        }

        let uncomp = if is_packed == 0 {
            let mut buf = vec![0u8; packed_size as usize];
            cur.read_exact(&mut buf)?;
            buf
        } else {
            let _uncomp_sz = cur.read_i32::<LittleEndian>()?;
            let mut comp = vec![0u8; packed_size as usize];
            cur.read_exact(&mut comp)?;
            let mut decomp = Vec::new();
            let mut decoder = flate2::read::ZlibDecoder::new(comp.as_slice());
            if decoder.read_to_end(&mut decomp).is_ok() {
                decomp
            } else {
                comp
            }
        };

        match chunk_id {
            // 0x07D5: UnitStats (Level, Attributes, Speeds)
            0x07D5 if uncomp.len() >= 47 && avatar_info.is_none() => {
                let mut s_cur = Cursor::new(&uncomp);
                let _stats_id = s_cur.read_u16::<LittleEndian>()?;
                let level = s_cur.read_u16::<LittleEndian>()?;
                let race_id = s_cur.read_u8()?;
                let agility = s_cur.read_u16::<LittleEndian>()?;
                let dexterity = s_cur.read_u16::<LittleEndian>()?;
                let charisma = s_cur.read_u16::<LittleEndian>()?;
                let intelligence = s_cur.read_u16::<LittleEndian>()?;
                let stamina = s_cur.read_u16::<LittleEndian>()?;
                let strength = s_cur.read_u16::<LittleEndian>()?;
                let wisdom = s_cur.read_u16::<LittleEndian>()?;
                let _rand = s_cur.read_u16::<LittleEndian>()?;
                let _rf = s_cur.read_u16::<LittleEndian>()?;
                let _ri = s_cur.read_u16::<LittleEndian>()?;
                let _rb = s_cur.read_u16::<LittleEndian>()?;
                let _rm = s_cur.read_u16::<LittleEndian>()?;
                let walk_speed = s_cur.read_u16::<LittleEndian>()?;
                let fight_speed = s_cur.read_u16::<LittleEndian>()?;
                let cast_speed = s_cur.read_u16::<LittleEndian>()?;

                avatar_info = Some(AvatarSaveInfo {
                    level,
                    race_id,
                    strength,
                    stamina,
                    agility,
                    dexterity,
                    intelligence,
                    wisdom,
                    charisma,
                    walk_speed,
                    fight_speed,
                    cast_speed,
                    skills: Vec::new(),
                    equipment: Vec::new(),
                    spellbook: Vec::new(),
                });
            }
            // 0x07D6: UnitSkills (Learned skills)
            0x07D6 if uncomp.len() >= 5 => {
                let count = uncomp.len() / 5;
                for i in 0..count {
                    let base = i * 5;
                    let skill_id = uncomp[base + 2];
                    let level = uncomp[base + 3];
                    skills.push(AvatarSkill { skill_id, level });
                }
            }
            // 0x07E9: UnitEquipment (Equipped inventory slots)
            0x07E9 if uncomp.len() >= 5 => {
                let count = uncomp.len() / 5;
                for i in 0..count {
                    let base = i * 5;
                    let slot = uncomp[base + 2];
                    let item_id = Cursor::new(&uncomp[base + 3..base + 5])
                        .read_u16::<LittleEndian>()
                        .unwrap_or(0);
                    equipment.push(AvatarEquipment { slot, item_id });
                }
            }
            // 0x07EA: UnitSpellbook (Scribed spells)
            0x07EA if uncomp.len() >= 5 => {
                let count = uncomp.len() / 5;
                for i in 0..count {
                    let base = i * 5;
                    let slot = uncomp[base + 2];
                    let spell_id = Cursor::new(&uncomp[base + 3..base + 5])
                        .read_u16::<LittleEndian>()
                        .unwrap_or(0);
                    spellbook.push(AvatarSpell { slot, spell_id });
                }
            }
            _ => {}
        }
    }

    if let Some(mut info) = avatar_info {
        info.skills = skills;
        info.equipment = equipment;
        info.spellbook = spellbook;
        Ok(Some(info))
    } else {
        Ok(None)
    }
}

pub fn modify_avatar_stats(sav_path: &Path, stats: &AvatarStatsUpdate) -> io::Result<bool> {
    let mut data = std::fs::read(sav_path)?;
    if data.len() < 24 {
        return Ok(false);
    }

    let mut cur = Cursor::new(&mut data[..]);
    cur.set_position(20);

    while (cur.position() as usize) + 12 <= cur.get_ref().len() {
        let chunk_id = cur.read_u16::<LittleEndian>()?;
        let _occurrence = cur.read_i16::<LittleEndian>()?;
        let is_packed = cur.read_i16::<LittleEndian>()?;
        let packed_size = cur.read_i32::<LittleEndian>()?;
        let _data_type = cur.read_i16::<LittleEndian>()?;

        if packed_size <= 0 {
            break;
        }

        if chunk_id == 0x07D5 {
            let chunk_pos = cur.position() as usize;
            if is_packed == 0 {
                if chunk_pos + (packed_size as usize) <= cur.get_ref().len() {
                    let d = cur.get_mut();
                    d[chunk_pos + 2..chunk_pos + 4].copy_from_slice(&stats.level.to_le_bytes());
                    d[chunk_pos + 5..chunk_pos + 7].copy_from_slice(&stats.agility.to_le_bytes());
                    d[chunk_pos + 7..chunk_pos + 9].copy_from_slice(&stats.dexterity.to_le_bytes());
                    d[chunk_pos + 9..chunk_pos + 11].copy_from_slice(&stats.charisma.to_le_bytes());
                    d[chunk_pos + 11..chunk_pos + 13]
                        .copy_from_slice(&stats.intelligence.to_le_bytes());
                    d[chunk_pos + 13..chunk_pos + 15].copy_from_slice(&stats.stamina.to_le_bytes());
                    d[chunk_pos + 15..chunk_pos + 17]
                        .copy_from_slice(&stats.strength.to_le_bytes());
                    d[chunk_pos + 17..chunk_pos + 19].copy_from_slice(&stats.wisdom.to_le_bytes());

                    crate::tools::atomic_write(sav_path, d)?;
                    return Ok(true);
                }
            } else {
                let _uncomp_sz = cur.read_i32::<LittleEndian>()?;
                let mut comp = vec![0u8; packed_size as usize];
                cur.read_exact(&mut comp)?;
                let mut decomp = Vec::new();
                let mut decoder = flate2::read::ZlibDecoder::new(comp.as_slice());
                if decoder.read_to_end(&mut decomp).is_ok() && decomp.len() >= 47 {
                    decomp[2..4].copy_from_slice(&stats.level.to_le_bytes());
                    decomp[5..7].copy_from_slice(&stats.agility.to_le_bytes());
                    decomp[7..9].copy_from_slice(&stats.dexterity.to_le_bytes());
                    decomp[9..11].copy_from_slice(&stats.charisma.to_le_bytes());
                    decomp[11..13].copy_from_slice(&stats.intelligence.to_le_bytes());
                    decomp[13..15].copy_from_slice(&stats.stamina.to_le_bytes());
                    decomp[15..17].copy_from_slice(&stats.strength.to_le_bytes());
                    decomp[17..19].copy_from_slice(&stats.wisdom.to_le_bytes());

                    let mut enc =
                        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                    enc.write_all(&decomp)?;
                    let new_comp = enc.finish()?;

                    let mut new_file_data = Vec::new();
                    new_file_data.extend_from_slice(&data[..chunk_pos - 8]);
                    new_file_data.write_i32::<LittleEndian>(new_comp.len() as i32)?;
                    new_file_data.extend_from_slice(&data[chunk_pos - 4..chunk_pos]);
                    new_file_data.write_i32::<LittleEndian>(decomp.len() as i32)?;
                    new_file_data.extend_from_slice(&new_comp);

                    let remainder_start = chunk_pos + 4 + (packed_size as usize);
                    if remainder_start < data.len() {
                        new_file_data.extend_from_slice(&data[remainder_start..]);
                    }

                    crate::tools::atomic_write(sav_path, &new_file_data)?;
                    return Ok(true);
                }
            }
            break;
        } else {
            let required = if is_packed == 0 {
                packed_size as i64
            } else {
                4 + (packed_size as i64)
            };
            cur.seek(SeekFrom::Current(required))?;
        }
    }
    Ok(false)
}
