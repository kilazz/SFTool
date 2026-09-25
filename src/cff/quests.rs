// src/cff/quests.rs

use super::container::Manifest;
use super::sf1_schema::{QuestEntry, Sf1Record};
use crate::UiLogger;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct VisualGraphNode {
    pub id: i32,
    pub title: String,
    pub subtitle: String,
    pub is_main: bool,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub record_index: i32,
}

#[derive(Clone, Debug)]
pub struct VisualGraphEdge {
    pub from_x: f32,
    pub from_y: f32,
    pub to_x: f32,
    pub to_y: f32,
}

pub struct QuestNode {
    pub quest: QuestEntry,
    pub record_index: usize,
    pub children: Vec<u32>,
}

struct GraphBuilder<'a> {
    tree: &'a BTreeMap<u32, QuestNode>,
    nodes: Vec<VisualGraphNode>,
    edges: Vec<VisualGraphEdge>,
    current_y: f32,
    node_w: f32,
    node_h: f32,
    col_gap: f32,
    row_gap: f32,
}

impl<'a> GraphBuilder<'a> {
    fn new(tree: &'a BTreeMap<u32, QuestNode>) -> Self {
        Self {
            tree,
            nodes: Vec::new(),
            edges: Vec::new(),
            current_y: 40.0,
            node_w: 200.0,
            node_h: 70.0,
            col_gap: 100.0,
            row_gap: 25.0,
        }
    }

    fn layout_subtree(&mut self, node_id: u32, depth: usize) -> (f32, f32) {
        let node = match self.tree.get(&node_id) {
            Some(n) => n,
            None => return (0.0, self.current_y),
        };

        let x = 40.0 + depth as f32 * (self.node_w + self.col_gap);
        let my_y;

        if node.children.is_empty() {
            my_y = self.current_y;
            self.current_y += self.node_h + self.row_gap;
        } else {
            let start_y = self.current_y;
            let mut child_positions = Vec::new();

            for &child_id in &node.children {
                let pos = self.layout_subtree(child_id, depth + 1);
                child_positions.push(pos);
            }

            let first_y = child_positions.first().map(|p| p.1).unwrap_or(start_y);
            let last_y = child_positions.last().map(|p| p.1).unwrap_or(start_y);
            my_y = (first_y + last_y) / 2.0;

            for &(cx, cy) in &child_positions {
                self.edges.push(VisualGraphEdge {
                    from_x: x + self.node_w,
                    from_y: my_y + self.node_h / 2.0,
                    to_x: cx,
                    to_y: cy + self.node_h / 2.0,
                });
            }
        }

        let is_main = node.quest.is_main_quest != 0;
        let title = format!("Quest #{}", node.quest.quest_id);
        let subtitle = format!(
            "Order: {} | NameID: {}",
            node.quest.order_index, node.quest.name_id
        );

        self.nodes.push(VisualGraphNode {
            id: node.quest.quest_id as i32,
            title,
            subtitle,
            is_main,
            x,
            y: my_y,
            width: self.node_w,
            height: self.node_h,
            record_index: node.record_index as i32,
        });

        (x, my_y)
    }
}

pub fn build_quest_graph_layout(
    cff_dir: &Path,
    logger: &UiLogger,
) -> std::io::Result<(Vec<VisualGraphNode>, Vec<VisualGraphEdge>)> {
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
    let mut roots = Vec::new();

    for (rec_idx, chunk_slice) in bytes
        .as_chunks::<{ QuestEntry::STRIDE }>()
        .0
        .iter()
        .enumerate()
    {
        if let Ok(q) = QuestEntry::decode(chunk_slice) {
            let q_id = q.quest_id;
            let parent_id = q.parent_quest_id;
            quests.insert(
                q_id,
                QuestNode {
                    quest: q,
                    record_index: rec_idx,
                    children: Vec::new(),
                },
            );
            if parent_id == 0 || parent_id == q_id {
                roots.push(q_id);
            }
        }
    }

    let parent_child: Vec<(u32, u32)> = quests
        .values()
        .filter(|n| n.quest.parent_quest_id != 0 && n.quest.parent_quest_id != n.quest.quest_id)
        .map(|n| (n.quest.parent_quest_id, n.quest.quest_id))
        .collect();

    for (p, c) in parent_child {
        if let Some(parent) = quests.get_mut(&p) {
            parent.children.push(c);
        }
    }

    let mut builder = GraphBuilder::new(&quests);

    for root_id in roots {
        builder.layout_subtree(root_id, 0);
        builder.current_y += 30.0;
    }

    logger.log(&format!(
        "[+] Generated Node Graph: {} quest cards, {} connection cables.",
        builder.nodes.len(),
        builder.edges.len()
    ));

    Ok((builder.nodes, builder.edges))
}

pub fn print_quest_ascii_tree(cff_dir: &Path, logger: &UiLogger) -> std::io::Result<()> {
    let manifest_str = fs::read_to_string(cff_dir.join("manifest.json"))?;
    let manifest: Manifest = serde_json::from_str(&manifest_str)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    let chunk = manifest
        .chunks
        .iter()
        .find(|c| c.id == 0x080D)
        .ok_or_else(|| std::io::Error::other("Quests chunk not found"))?;

    let bytes = fs::read(cff_dir.join(&chunk.file))?;
    let mut quests = BTreeMap::new();

    for chunk_slice in bytes.as_chunks::<{ QuestEntry::STRIDE }>().0 {
        if let Ok(q) = QuestEntry::decode(chunk_slice) {
            quests.insert(q.quest_id, q);
        }
    }

    for q in quests.values() {
        if q.parent_quest_id == 0 || q.parent_quest_id == q.quest_id {
            logger.log(&format!(
                "+- [{}] Quest #{:<4} (Order: {}, NameID: {})",
                if q.is_main_quest != 0 { "MAIN" } else { "SIDE" },
                q.quest_id,
                q.order_index,
                q.name_id
            ));
        }
    }
    Ok(())
}
