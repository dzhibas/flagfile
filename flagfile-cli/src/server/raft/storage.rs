use std::sync::{Arc, RwLock};

use protobuf::Message as _;
use raft::prelude::*;
use raft::{Error as RaftError, Result as RaftResult, Storage};

/// In-memory Raft log storage.
///
/// Wraps an `Arc<RwLock<_>>` so it can be shared across the Raft tick loop
/// and snapshot operations.
#[derive(Clone)]
pub struct MemRaftStorage {
    inner: Arc<RwLock<MemRaftStorageCore>>,
}

struct MemRaftStorageCore {
    hard_state: HardState,
    conf_state: ConfState,
    snapshot: Snapshot,
    entries: Vec<Entry>,
    /// Index of the first entry in `entries` (entries[0].index == offset).
    offset: u64,
}

impl MemRaftStorage {
    /// Create a new in-memory storage with the given set of initial voter IDs.
    pub fn new(voters: Vec<u64>) -> Self {
        let mut cs = ConfState::default();
        cs.voters = voters;

        // Seed with a dummy entry at index 0 so that first_index() == 1.
        let mut dummy = Entry::default();
        dummy.index = 0;
        dummy.term = 0;

        Self {
            inner: Arc::new(RwLock::new(MemRaftStorageCore {
                hard_state: HardState::default(),
                conf_state: cs,
                snapshot: Snapshot::default(),
                entries: vec![dummy],
                offset: 0,
            })),
        }
    }

    /// Append entries to the log, replacing any existing entries from the same
    /// index onward (handles log conflicts during leader changes).
    pub fn append(&self, entries: &[Entry]) -> RaftResult<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let mut core = self.inner.write().unwrap();
        let first_new = entries[0].index;

        if first_new <= core.offset {
            return Err(RaftError::Store(raft::StorageError::Compacted));
        }

        // Truncate any conflicting tail entries.
        let relative = (first_new - core.offset) as usize;
        if relative < core.entries.len() {
            core.entries.truncate(relative);
        }

        core.entries.extend_from_slice(entries);
        Ok(())
    }

    /// Persist the hard state (term, vote, commit).
    pub fn set_hard_state(&self, hs: HardState) {
        let mut core = self.inner.write().unwrap();
        core.hard_state = hs;
    }

    /// Persist the conf state (voter / learner membership).
    #[allow(dead_code)]
    pub fn set_conf_state(&self, cs: ConfState) {
        let mut core = self.inner.write().unwrap();
        core.conf_state = cs;
    }

    /// Apply an incoming Raft snapshot, replacing all local state.
    pub fn apply_snapshot(&self, snapshot: Snapshot) -> RaftResult<()> {
        let snap_index = snapshot.get_metadata().index;

        let mut core = self.inner.write().unwrap();
        if snap_index <= core.offset {
            return Err(RaftError::Store(raft::StorageError::SnapshotOutOfDate));
        }

        core.conf_state = snapshot.get_metadata().get_conf_state().clone();
        core.offset = snap_index;
        core.entries.clear();

        // Keep a dummy entry so that first_index / last_index stay consistent.
        let mut dummy = Entry::default();
        dummy.index = snap_index;
        dummy.term = snapshot.get_metadata().term;
        core.entries.push(dummy);

        core.snapshot = snapshot;
        Ok(())
    }

    /// Compact the log up to (and including) `index`, discarding old entries.
    pub fn compact(&self, index: u64) -> RaftResult<()> {
        let mut core = self.inner.write().unwrap();

        if index <= core.offset {
            return Err(RaftError::Store(raft::StorageError::Compacted));
        }

        let last_idx = core.entries.last().map(|e| e.index).unwrap_or(0);
        if index > last_idx {
            return Err(RaftError::Store(raft::StorageError::Unavailable));
        }

        let drain_to = (index - core.offset) as usize;
        core.entries.drain(..drain_to);
        core.offset = index;
        Ok(())
    }

    /// Create a snapshot at the given index with the provided conf state and
    /// application data.
    pub fn create_snapshot(&self, index: u64, cs: ConfState, data: Vec<u8>) -> RaftResult<()> {
        let mut core = self.inner.write().unwrap();

        let last_idx = core.entries.last().map(|e| e.index).unwrap_or(0);
        if index > last_idx {
            return Err(RaftError::Store(raft::StorageError::Unavailable));
        }

        // Find the term for the snapshot index.
        let term = {
            let relative = (index - core.offset) as usize;
            if relative >= core.entries.len() {
                return Err(RaftError::Store(raft::StorageError::Unavailable));
            }
            core.entries[relative].term
        };

        let mut snap = Snapshot::default();
        snap.mut_metadata().index = index;
        snap.mut_metadata().term = term;
        snap.mut_metadata().mut_conf_state().voters = cs.voters.clone();
        snap.mut_metadata().mut_conf_state().learners = cs.learners.clone();
        snap.data = data.into();
        core.snapshot = snap;

        Ok(())
    }
}

impl Storage for MemRaftStorage {
    fn initial_state(&self) -> RaftResult<RaftState> {
        let core = self.inner.read().unwrap();
        Ok(RaftState {
            hard_state: core.hard_state.clone(),
            conf_state: core.conf_state.clone(),
        })
    }

    fn entries(
        &self,
        low: u64,
        high: u64,
        max_size: impl Into<Option<u64>>,
        _context: raft::GetEntriesContext,
    ) -> RaftResult<Vec<Entry>> {
        let max_size = max_size.into();
        let core = self.inner.read().unwrap();

        if low <= core.offset {
            return Err(RaftError::Store(raft::StorageError::Compacted));
        }

        let last_idx = core.entries.last().map(|e| e.index).unwrap_or(0);
        if high > last_idx + 1 {
            panic!(
                "entries high({}) is out of bound, last index({})",
                high, last_idx
            );
        }

        let lo = (low - core.offset) as usize;
        let hi = (high - core.offset) as usize;

        let mut result = Vec::new();
        let mut total_size: u64 = 0;

        for entry in &core.entries[lo..hi] {
            total_size += entry.compute_size() as u64;
            if let Some(max) = max_size {
                if !result.is_empty() && total_size > max {
                    break;
                }
            }
            result.push(entry.clone());
        }

        Ok(result)
    }

    fn term(&self, idx: u64) -> RaftResult<u64> {
        let core = self.inner.read().unwrap();

        if idx < core.offset {
            return Err(RaftError::Store(raft::StorageError::Compacted));
        }

        // Check if the snapshot covers this index.
        if !core.snapshot.is_empty() && idx == core.snapshot.get_metadata().index {
            return Ok(core.snapshot.get_metadata().term);
        }

        let relative = (idx - core.offset) as usize;
        if relative >= core.entries.len() {
            return Err(RaftError::Store(raft::StorageError::Unavailable));
        }

        Ok(core.entries[relative].term)
    }

    fn first_index(&self) -> RaftResult<u64> {
        let core = self.inner.read().unwrap();
        Ok(core.offset + 1)
    }

    fn last_index(&self) -> RaftResult<u64> {
        let core = self.inner.read().unwrap();
        let last = core.entries.last().map(|e| e.index).unwrap_or(core.offset);
        Ok(last)
    }

    fn snapshot(&self, _request_index: u64, _to: u64) -> RaftResult<Snapshot> {
        let core = self.inner.read().unwrap();
        Ok(core.snapshot.clone())
    }
}
