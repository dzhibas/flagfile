use std::collections::HashMap;

use flagfile_lib::ast::FlagMetadata;
use flagfile_lib::eval::Segments;
use flagfile_lib::parse_flagfile::Rule;
use tokio::sync::RwLock;

pub type ParsedFlags = (
    HashMap<String, Vec<Rule>>,
    HashMap<String, FlagMetadata>,
    Segments,
);

pub struct FlagStore {
    pub flagfile_content: String,
    pub flags: HashMap<String, Vec<Rule>>,
    pub metadata: HashMap<String, FlagMetadata>,
    pub segments: Segments,
    pub env: Option<String>,
}

pub struct AppState {
    pub store: RwLock<FlagStore>,
}
