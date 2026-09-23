use super::container::Manifest;
use super::editor::EditorItem;
use super::text::{decode_windows, encode_windows};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::fs::{self, File};
use std::io::{Cursor, Write};
use std::path::Path;

pub struct Sf1ChunkInfo {
    pub name: &'static str,
    pub stride: usize,
    pub default_c_type: i16,
}

pub fn get_sf1_chunk_info(id: u32) -> Option<Sf1ChunkInfo> {
    match id {
        0x07DC => Some(Sf1ChunkInfo {
            name: "2dGfxItems",
            stride: 69,
            default_c_type: 1,
        }),
        0x07E1 => Some(Sf1ChunkInfo {
            name: "SpellMultiMap",
            stride: 6,
            default_c_type: 1,
        }),
        0x07E2 => Some(Sf1ChunkInfo {
            name: "SpellsBiMap",
            stride: 4,
            default_c_type: 1,
        }),
        0x07F7 => Some(Sf1ChunkInfo {
            name: "TextDialogueMap",
            stride: 4,
            default_c_type: 1,
        }),
        0x07FC => Some(Sf1ChunkInfo {
            name: "TypeCategoryMap",
            stride: 3,
            default_c_type: 1,
        }),
        0x07FF => Some(Sf1ChunkInfo {
            name: "EntityLinkMap",
            stride: 5,
            default_c_type: 1,
        }),
        0x0800 => Some(Sf1ChunkInfo {
            name: "ComplexProperties",
            stride: 15,
            default_c_type: 3,
        }),
        0x0801 => Some(Sf1ChunkInfo {
            name: "WordValuesArray",
            stride: 2,
            default_c_type: 1,
        }),
        0x080A => Some(Sf1ChunkInfo {
            name: "LocalizedStringIds",
            stride: 4,
            default_c_type: 1,
        }),
        0x080B => Some(Sf1ChunkInfo {
            name: "AudioSpeechParams",
            stride: 6,
            default_c_type: 1,
        }),
        0x080E => Some(Sf1ChunkInfo {
            name: "CompoundKeyTable",
            stride: 9,
            default_c_type: 1,
        }),
        0x0818 => Some(Sf1ChunkInfo {
            name: "SystemLookupMap",
            stride: 4,
            default_c_type: 1,
        }),
        _ => None,
    }
}

pub fn load_sf1_items(cff_dir: &Path, category: &str, filter: &str) -> Vec<EditorItem> {
    let mut items = Vec::new();
    let filter_lower = filter.to_lowercase();
    let manifest_path = cff_dir.join("manifest.json");

    let Ok(m_str) = fs::read_to_string(manifest_path) else {
        return items;
    };
    let Ok(manifest) = serde_json::from_str::<Manifest>(&m_str) else {
        return items;
    };

    if category.contains("0x07DC") {
        if let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07DC)
            && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
        {
            let num_records = bytes.len() / 69;
            for i in 0..num_records {
                let offset = i * 69;
                let item_id = Cursor::new(&bytes[offset..offset + 2])
                    .read_u16::<LittleEndian>()
                    .unwrap_or(0);
                let flag = bytes[offset + 2];
                let mesh_bytes = &bytes[offset + 3..offset + 67];
                let mesh_end = mesh_bytes
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(mesh_bytes.len());
                let mesh_name = decode_windows(&mesh_bytes[..mesh_end]);
                let extra = Cursor::new(&bytes[offset + 67..offset + 69])
                    .read_u16::<LittleEndian>()
                    .unwrap_or(0);

                let display = format!("ID: {:<5} [Flag: {}] | Mesh: {}", item_id, flag, mesh_name);
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: item_id.to_string(),
                        val1: mesh_name,
                        val2: format!("Flag: {}, Extra: {}", flag, extra),
                        display,
                    });
                }
            }
        }
    } else if category.contains("0x07E2")
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07E2)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        let num_records = bytes.len() / 4;
        for i in 0..num_records {
            let offset = i * 4;
            let spell_id = Cursor::new(&bytes[offset..offset + 2])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);
            let related_id = Cursor::new(&bytes[offset + 2..offset + 4])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);

            let display = format!(
                "Spell ID: {:<5} -> Target/Scroll ID: {}",
                spell_id, related_id
            );
            if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                items.push(EditorItem {
                    id_str: spell_id.to_string(),
                    val1: related_id.to_string(),
                    val2: "BiMap Entry".into(),
                    display,
                });
            }
        }
    }

    items
}

pub fn save_sf1_item(
    cff_dir: &Path,
    category: &str,
    index: usize,
    val1: &str,
) -> std::io::Result<()> {
    let manifest_path = cff_dir.join("manifest.json");
    let m_str = fs::read_to_string(manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&m_str)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    if category.contains("0x07DC") {
        if let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07DC) {
            let chunk_path = cff_dir.join(&chunk.file);
            let mut bytes = fs::read(&chunk_path)?;
            let offset = index * 69;
            if offset + 69 <= bytes.len() {
                let enc_mesh = encode_windows(val1);
                let mut padded_mesh = vec![0u8; 64];
                let len = enc_mesh.len().min(63);
                padded_mesh[..len].copy_from_slice(&enc_mesh[..len]);
                bytes[offset + 3..offset + 67].copy_from_slice(&padded_mesh);
                File::create(chunk_path)?.write_all(&bytes)?;
            }
        }
    } else if category.contains("0x07E2")
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07E2)
    {
        let chunk_path = cff_dir.join(&chunk.file);
        let mut bytes = fs::read(&chunk_path)?;
        let offset = index * 4;
        if offset + 4 <= bytes.len()
            && let Ok(new_rel) = val1.parse::<u16>()
        {
            let mut cur = Cursor::new(&mut bytes[offset + 2..offset + 4]);
            cur.write_u16::<LittleEndian>(new_rel)?;
            File::create(chunk_path)?.write_all(&bytes)?;
        }
    }
    Ok(())
}
