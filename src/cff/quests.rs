// src/cff/quests.rs

use super::container::Manifest;
use super::sf1_schema::{QuestEntry, Sf1Record};
use crate::UiLogger;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub struct QuestNode {
    pub quest: QuestEntry,
    pub title: String,
    pub children: Vec<u32>,
}

pub fn generate_quest_hierarchy_tree(
    cff_dir: &Path,
    logger: &UiLogger,
) -> std::io::Result<BTreeMap<u32, QuestNode>> {
    let manifest_str = fs::read_to_string(cff_dir.join("manifest.json"))?;
    let manifest: Manifest = serde_json::from_str(&manifest_str)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    let chunk = manifest
        .chunks
        .iter()
        .find(|c| c.id == 0x080D)
        .ok_or_else(|| std::io::Error::other("Quests chunk 0x080D not found in manifest"))?;

    let bytes = fs::read(cff_dir.join(&chunk.file))?;
    let mut quests = BTreeMap::new();
    let mut root_ids = Vec::new();

    for chunk_slice in bytes.as_chunks::<{ QuestEntry::STRIDE }>().0 {
        if let Ok(q) = QuestEntry::decode(chunk_slice) {
            let q_id = q.quest_id;
            let parent_id = q.parent_quest_id;
            quests.insert(
                q_id,
                QuestNode {
                    quest: q,
                    title: format!("Quest #{}", q_id),
                    children: Vec::new(),
                },
            );
            if parent_id == 0 || parent_id == q_id {
                root_ids.push(q_id);
            }
        }
    }

    // Build parent-child relationships
    let parent_child_pairs: Vec<(u32, u32)> = quests
        .values()
        .filter(|n| n.quest.parent_quest_id != 0 && n.quest.parent_quest_id != n.quest.quest_id)
        .map(|n| (n.quest.parent_quest_id, n.quest.quest_id))
        .collect();

    for (parent_id, child_id) in parent_child_pairs {
        if let Some(parent) = quests.get_mut(&parent_id) {
            parent.children.push(child_id);
        }
    }

    logger.log(&format!(
        "[+] Loaded {} quests into hierarchy. Root quests: {}",
        quests.len(),
        root_ids.len()
    ));
    Ok(quests)
}

pub fn print_quest_ascii_tree(cff_dir: &Path, logger: &UiLogger) -> std::io::Result<()> {
    let tree = generate_quest_hierarchy_tree(cff_dir, logger)?;

    fn print_node(tree: &BTreeMap<u32, QuestNode>, node_id: u32, depth: usize, logger: &UiLogger) {
        if let Some(node) = tree.get(&node_id) {
            let indent = "  ".repeat(depth);
            let kind_tag = if node.quest.is_main_quest != 0 {
                "[MAIN]"
            } else {
                "[SIDE]"
            };
            logger.log(&format!(
                "{}+- {} Quest #{:<4} (Order: {}, NameID: {})",
                indent, kind_tag, node.quest.quest_id, node.quest.order_index, node.quest.name_id
            ));
            for &child_id in &node.children {
                print_node(tree, child_id, depth + 1, logger);
            }
        }
    }

    for node in tree.values() {
        if node.quest.parent_quest_id == 0 || node.quest.parent_quest_id == node.quest.quest_id {
            print_node(&tree, node.quest.quest_id, 0, logger);
        }
    }
    Ok(())
}
