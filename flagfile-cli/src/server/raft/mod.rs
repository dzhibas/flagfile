pub mod node;
pub mod state_machine;
pub mod storage;
pub mod transport;

use serde::{Deserialize, Serialize};

use super::store::Meta;

/// Commands replicated through Raft log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaftCommand {
    PutFlagfile {
        namespace: String,
        content: Vec<u8>,
        meta: Meta,
    },
}
