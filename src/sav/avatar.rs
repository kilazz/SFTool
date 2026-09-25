// src/sav/avatar.rs

use byteorder::{LittleEndian, ReadBytesExt};
use std::fs::File;
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::path::Path;

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
}

pub fn inspect_avatar_save(sav_path: &Path) -> io::Result<Option<AvatarSaveInfo>> {
    let mut f = File::open(sav_path)?;
    let mut data = Vec::new();
    f.read_to_end(&mut data)?;

    if data.len() < 24 {
        return Ok(None);
    }

    let mut cur = Cursor::new(&data);
    cur.set_position(20);

    while (cur.position() as usize) + 12 <= data.len() {
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
        if (cur.position() as usize) + required > data.len() {
            break;
        }

        // Search for Avatar UnitStats chunk (0x07D5)
        if chunk_id == 0x07D5 {
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

            if uncomp.len() >= 47 {
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

                return Ok(Some(AvatarSaveInfo {
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
                }));
            }
        } else {
            cur.seek(SeekFrom::Current(required as i64))?;
        }
    }

    Ok(None)
}
