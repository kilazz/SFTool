// src/cff/references.rs

use super::tracer::TracerEngine;
use std::path::Path;

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct EntityReference {
    pub category_id: u32,
    pub category_name: String,
    pub record_index: usize,
    pub target_id: u32,
    pub field_name: String,
}

pub fn find_all_references(
    cff_dir: &Path,
    category_id: u32,
    target_id: u32,
) -> Vec<EntityReference> {
    let tracer = TracerEngine::default();
    tracer.find_references(cff_dir, category_id, target_id)
}
