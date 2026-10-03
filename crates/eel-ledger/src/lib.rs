//! The append-only ledger.
//!
//! Events are validated by the ontology projector, appended in order, and
//! sealed into blocks. A block commits to its events through a Merkle root
//! and to the resulting ontology through the state root. Nothing is ever
//! removed: there is no operation that deletes or rewrites an event or block.

pub mod merkle;

use eel_core::genesis::GenesisConfig;
use eel_core::*;
use eel_crypto::KeyPair;
use eel_ontology::{is_branch_scoped, OntologyState, ProjectionError, StateProjector};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("event rejected: {0}")]
    Projection(#[from] ProjectionError),
    #[error("event {0} is already in the ledger")]
    Duplicate(EventId),
    #[error("parent event {0} is unknown")]
    UnknownParent(EventId),
    #[error("event timestamp precedes its parent {0}")]
    TimestampBeforeParent(EventId),
    #[error("invalid genesis: {0}")]
    Genesis(#[from] eel_ontology::GenesisError),
    #[error("block: {0}")]
    Block(String),
    #[error("crypto: {0}")]
    Crypto(#[from] eel_crypto::CryptoError),
    #[error(transparent)]
    Core(#[from] CoreError),
}

pub type Result<T> = std::result::Result<T, LedgerError>;

/// A block header as specified. Its hash covers every field except the
/// signature, and the signature is the proposer's Ed25519 signature over
/// that hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub height: u64,
    pub previous_block_hash: BlockHash,
    pub timestamp: Timestamp,
    pub events_root: Hash,
    pub state_root: StateRoot,
    pub proposer: ValidatorId,
    pub signature: String,
}

#[derive(Serialize)]
struct UnsignedHeader<'a> {
    height: u64,
    previous_block_hash: &'a BlockHash,
    timestamp: Timestamp,
    events_root: &'a Hash,
    state_root: &'a StateRoot,
    proposer: &'a ValidatorId,
}

impl Block {
    pub fn hash(&self) -> Result<BlockHash> {
        let u = UnsignedHeader {
            height: self.height,
            previous_block_hash: &self.previous_block_hash,
            timestamp: self.timestamp,
            events_root: &self.events_root,
            state_root: &self.state_root,
            proposer: &self.proposer,
        };
        Ok(BlockHash(u.canonical_hash()?))
    }
}

/// A sealed block together with the ordered ids of its events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockRecord {
    pub block: Block,
    pub block_hash: BlockHash,
    pub event_ids: Vec<EventId>,
}

#[derive(Debug, Clone)]
pub struct Ledger {
    genesis: GenesisConfig,
    genesis_state: OntologyState,
    state: OntologyState,
    events: Vec<(EventId, EpistemicEvent)>,
    index: BTreeMap<EventId, usize>,
    blocks: Vec<BlockRecord>,
    /// Index into `events` of the first event not yet sealed.
    sealed_upto: usize,
}

impl Ledger {
    /// Creates a ledger holding only the genesis block, sealed with `sealer`.
    pub fn new(genesis: GenesisConfig, sealer: &KeyPair) -> Result<Self> {
        let mut l = Self::unsealed(genesis)?;
        let block = l.make_block(sealer, vec![])?;
        l.blocks.push(block);
        Ok(l)
    }

    /// Creates a ledger and imports an existing genesis block.
    pub fn from_genesis_block(genesis: GenesisConfig, block0: BlockRecord) -> Result<Self> {
        let mut l = Self::unsealed(genesis)?;
        l.import_block(block0, vec![])?;
        Ok(l)
    }

    fn unsealed(genesis: GenesisConfig) -> Result<Self> {
        let genesis_state = OntologyState::genesis(&genesis)?;
        Ok(Ledger {
            genesis,
            state: genesis_state.clone(),
            genesis_state,
            events: vec![],
            index: BTreeMap::new(),
            blocks: vec![],
            sealed_upto: 0,
        })
    }

    pub fn genesis(&self) -> &GenesisConfig {
        &self.genesis
    }

    /// Current projected state, including events not yet sealed.
    pub fn state(&self) -> &OntologyState {
        &self.state
    }

    pub fn blocks(&self) -> &[BlockRecord] {
        &self.blocks
    }

    pub fn head(&self) -> &BlockRecord {
        self.blocks.last().expect("genesis block always exists")
    }

    pub fn events(&self) -> impl Iterator<Item = &(EventId, EpistemicEvent)> {
        self.events.iter()
    }

    pub fn event(&self, id: &EventId) -> Option<&EpistemicEvent> {
        self.index.get(id).map(|&i| &self.events[i].1)
    }

    pub fn pending(&self) -> &[(EventId, EpistemicEvent)] {
        &self.events[self.sealed_upto..]
    }

    /// The height of the block containing an event, if sealed.
    pub fn event_height(&self, id: &EventId) -> Option<u64> {
        self.blocks
            .iter()
            .find(|b| b.event_ids.contains(id))
            .map(|b| b.block.height)
    }

    /// Validates an event against the ledger and the projected ontology, then
    /// appends it. Rejected events leave the ledger unchanged.
    pub fn submit(&mut self, event: EpistemicEvent) -> Result<EventId> {
        let id = event.event_id()?;
        if self.index.contains_key(&id) {
            return Err(LedgerError::Duplicate(id));
        }
        for p in &event.parent_events {
            let parent = self.event(p).ok_or(LedgerError::UnknownParent(*p))?;
            if event.timestamp < parent.timestamp {
                return Err(LedgerError::TimestampBeforeParent(*p));
            }
        }
        self.state.apply_event(&event)?;
        self.index.insert(id, self.events.len());
        self.events.push((id, event));
        Ok(id)
    }

    fn make_block(&self, sealer: &KeyPair, event_ids: Vec<EventId>) -> Result<BlockRecord> {
        let proposer = self.genesis.block_sealer.clone();
        let validator = &self.state.validators[&proposer];
        if validator.identity_id != sealer.identity_id() {
            return Err(LedgerError::Block(
                "key does not belong to the configured block sealer".into(),
            ));
        }
        let (height, previous_block_hash, prev_ts) = match self.blocks.last() {
            Some(b) => (b.block.height + 1, b.block_hash, b.block.timestamp),
            None => (0, BlockHash(Hash::ZERO), self.genesis.genesis_time),
        };
        // Block time comes from the events, never from the sealing node's clock.
        let timestamp = event_ids
            .iter()
            .map(|id| self.event(id).unwrap().timestamp)
            .chain([prev_ts])
            .max()
            .unwrap();
        let mut block = Block {
            height,
            previous_block_hash,
            timestamp,
            events_root: merkle::root(&event_ids),
            state_root: self.state.state_root(),
            proposer,
            signature: String::new(),
        };
        let block_hash = block.hash()?;
        block.signature = sealer.sign_bytes(block_hash.as_hash().as_bytes());
        Ok(BlockRecord {
            block,
            block_hash,
            event_ids,
        })
    }

    /// Seals all pending events into a new block. Returns None when nothing is pending.
    pub fn seal(&mut self, sealer: &KeyPair) -> Result<Option<&BlockRecord>> {
        if self.sealed_upto == self.events.len() {
            return Ok(None);
        }
        let ids: Vec<EventId> = self.pending().iter().map(|(id, _)| *id).collect();
        let block = self.make_block(sealer, ids)?;
        self.blocks.push(block);
        self.sealed_upto = self.events.len();
        Ok(self.blocks.last())
    }

    /// Verifies and appends a block produced elsewhere, applying its events.
    /// Used to load from storage and to follow another node.
    pub fn import_block(&mut self, record: BlockRecord, events: Vec<EpistemicEvent>) -> Result<()> {
        if self.sealed_upto != self.events.len() {
            return Err(LedgerError::Block(
                "cannot import a block while events are pending".into(),
            ));
        }
        let b = &record.block;
        let (height, prev, prev_ts) = match self.blocks.last() {
            Some(last) => (last.block.height + 1, last.block_hash, last.block.timestamp),
            None => (0, BlockHash(Hash::ZERO), self.genesis.genesis_time),
        };
        let fail = |m: &str| Err(LedgerError::Block(m.to_string()));
        if b.height != height || b.previous_block_hash != prev {
            return fail("block does not extend the current head");
        }
        if b.proposer != self.genesis.block_sealer {
            return fail("block proposer is not the configured sealer");
        }
        if b.hash()? != record.block_hash {
            return fail("block hash mismatch");
        }
        let sealer = &self.state.validators[&b.proposer];
        let key = &self.state.identities[&sealer.identity_id].public_key;
        eel_crypto::verify_bytes(key, record.block_hash.as_hash().as_bytes(), &b.signature)?;
        if events.len() != record.event_ids.len() || (height > 0 && events.is_empty()) {
            return fail("block events do not match its event list");
        }
        if merkle::root(&record.event_ids) != b.events_root {
            return fail("events root mismatch");
        }
        let checkpoint = self.clone();
        let result = (|| {
            for (expected, e) in record.event_ids.iter().zip(events) {
                let id = self.submit(e)?;
                if id != *expected {
                    return fail("event id mismatch");
                }
            }
            let max_ts = record
                .event_ids
                .iter()
                .map(|id| self.event(id).unwrap().timestamp)
                .chain([prev_ts])
                .max()
                .unwrap();
            if b.timestamp != max_ts {
                return fail("block timestamp is not derived from its events");
            }
            if self.state.state_root() != b.state_root {
                return fail("state root mismatch");
            }
            Ok(())
        })();
        if let Err(e) = result {
            *self = checkpoint;
            return Err(e);
        }
        self.sealed_upto = self.events.len();
        self.blocks.push(record);
        Ok(())
    }

    /// Events sealed in blocks up to and including `height`, in ledger order.
    pub fn events_through(&self, height: u64) -> Vec<&EpistemicEvent> {
        self.blocks
            .iter()
            .take_while(|b| b.block.height <= height)
            .flat_map(|b| b.event_ids.iter())
            .map(|id| self.event(id).unwrap())
            .collect()
    }

    /// Reconstructs the ontology as it was at a block height by replaying
    /// events from genesis.
    pub fn state_at(&self, height: u64) -> Result<OntologyState> {
        if height >= self.blocks.len() as u64 {
            return Err(LedgerError::Block(format!("no block at height {height}")));
        }
        let mut s = self.genesis_state.clone();
        for e in self.events_through(height) {
            s.apply_event(e)?;
        }
        Ok(s)
    }

    /// The events that make up a branch's own view of history: every
    /// network-wide event, plus branch-scoped events on the branch itself and
    /// on its ancestors up to the point where each descendant forked.
    pub fn branch_view(&self, branch: &BranchId) -> Option<Vec<&EpistemicEvent>> {
        // Map each branch in the ancestry to the position (in ledger order)
        // after which its branch-scoped events no longer belong to the view.
        let mut cutoff: BTreeMap<BranchId, Option<usize>> = BTreeMap::new();
        let mut current = self.state.branches.get(branch)?;
        cutoff.insert(branch.clone(), None);
        while let Some(parent) = &current.branch.parent_branch {
            // The BRANCH_CREATE event for `current` lives on `parent`.
            let create_pos = self.events.iter().position(|(_, e)| {
                matches!(&e.payload, EventPayload::BranchCreate(b) if b.branch_id == current.branch.branch_id)
            })?;
            cutoff.insert(parent.clone(), Some(create_pos));
            current = self.state.branches.get(parent)?;
        }
        let ancestry: BTreeSet<&BranchId> = cutoff.keys().collect();
        Some(
            self.events
                .iter()
                .enumerate()
                .filter(|(pos, (_, e))| {
                    if !is_branch_scoped(e.event_type) {
                        return true;
                    }
                    ancestry.contains(&e.branch)
                        && match cutoff[&e.branch] {
                            None => true,
                            Some(limit) => *pos <= limit,
                        }
                })
                .map(|(_, (_, e))| e)
                .collect(),
        )
    }

    /// Replays a branch's view from genesis on a fresh state.
    pub fn reconstruct_branch(&self, branch: &BranchId) -> Result<OntologyState> {
        let view = self
            .branch_view(branch)
            .ok_or_else(|| LedgerError::Block(format!("unknown branch {branch}")))?;
        let mut s = self.genesis_state.clone();
        for e in view {
            s.apply_event(e)?;
        }
        Ok(s)
    }
}
