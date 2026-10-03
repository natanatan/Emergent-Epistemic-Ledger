//! SQLite persistence.
//!
//! The database stores the genesis configuration, every event and every block.
//! On open, the ledger is rebuilt by re-verifying and replaying every block
//! from genesis, so a tampered database fails to load instead of serving a
//! different ontology. Projection tables are refreshed after each block for
//! external queries; they are caches, never read back by the node.

use eel_core::genesis::GenesisConfig;
use eel_core::*;
use eel_crypto::KeyPair;
use eel_ledger::{Block, BlockRecord, Ledger};
use eel_ontology::OntologyState;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use std::path::Path;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database: {0}")]
    Db(#[from] sqlx::Error),
    #[error("migration: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("ledger: {0}")]
    Ledger(#[from] eel_ledger::LedgerError),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub struct Store {
    pool: SqlitePool,
}

fn s<T: serde::Serialize>(v: &T) -> Result<String> {
    Ok(serde_json::to_string(v)?)
}

fn enum_name<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

impl Store {
    /// Opens (creating if needed) a database file and runs migrations.
    pub async fn open(path: &Path) -> Result<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(opts)
            .await?;
        MIGRATOR.run(&pool).await?;
        Ok(Store { pool })
    }

    pub async fn in_memory() -> Result<Self> {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await?;
        MIGRATOR.run(&pool).await?;
        Ok(Store { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn genesis(&self) -> Result<Option<GenesisConfig>> {
        let row = sqlx::query("SELECT value FROM meta WHERE key = 'genesis'")
            .fetch_optional(&self.pool)
            .await?;
        Ok(match row {
            Some(r) => Some(serde_json::from_str(r.get::<&str, _>(0))?),
            None => None,
        })
    }

    /// Writes a new ledger's genesis configuration and genesis block.
    pub async fn initialize(&self, ledger: &Ledger) -> Result<()> {
        if self.genesis().await?.is_some() {
            return Err(StorageError::Invalid(
                "database is already initialized".into(),
            ));
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO meta (key, value) VALUES ('genesis', ?)")
            .bind(s(ledger.genesis())?)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        self.append_block(ledger, &ledger.blocks()[0]).await
    }

    /// Persists one sealed block and its events, then refreshes projections.
    pub async fn append_block(&self, ledger: &Ledger, record: &BlockRecord) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let start: i64 = sqlx::query("SELECT COALESCE(MAX(position) + 1, 0) FROM events")
            .fetch_one(&mut *tx)
            .await?
            .get(0);
        for (i, id) in record.event_ids.iter().enumerate() {
            let e = ledger
                .event(id)
                .ok_or_else(|| StorageError::Invalid(format!("event {id} not in ledger")))?;
            sqlx::query(
                "INSERT INTO events (event_id, position, block_height, event_type, branch, author, timestamp, body)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id.to_string())
            .bind(start + i as i64)
            .bind(record.block.height as i64)
            .bind(enum_name(&e.event_type))
            .bind(e.branch.to_string())
            .bind(e.author.to_string())
            .bind(e.timestamp.0)
            .bind(s(e)?)
            .execute(&mut *tx)
            .await?;
        }
        let b = &record.block;
        sqlx::query(
            "INSERT INTO blocks (height, block_hash, previous_block_hash, timestamp, events_root, state_root, proposer, signature, event_ids)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(b.height as i64)
        .bind(record.block_hash.to_string())
        .bind(b.previous_block_hash.to_string())
        .bind(b.timestamp.0)
        .bind(b.events_root.to_string())
        .bind(b.state_root.to_string())
        .bind(b.proposer.to_string())
        .bind(&b.signature)
        .bind(s(&record.event_ids)?)
        .execute(&mut *tx)
        .await?;
        write_projections(&mut tx, ledger.state()).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Loads and fully re-verifies the ledger.
    pub async fn load(&self) -> Result<Ledger> {
        let genesis = self
            .genesis()
            .await?
            .ok_or_else(|| StorageError::Invalid("database is not initialized".into()))?;
        let rows = sqlx::query(
            "SELECT height, block_hash, previous_block_hash, timestamp, events_root, state_root, proposer, signature, event_ids
             FROM blocks ORDER BY height",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut ledger: Option<Ledger> = None;
        for row in rows {
            let parse = |i: usize| -> Result<Hash> {
                Hash::from_str(row.get::<&str, _>(i))
                    .map_err(|e| StorageError::Invalid(e.to_string()))
            };
            let event_ids: Vec<EventId> = serde_json::from_str(row.get::<&str, _>(8))?;
            let record = BlockRecord {
                block: Block {
                    height: row.get::<i64, _>(0) as u64,
                    previous_block_hash: BlockHash(parse(2)?),
                    timestamp: Timestamp(row.get(3)),
                    events_root: parse(4)?,
                    state_root: StateRoot(parse(5)?),
                    proposer: row
                        .get::<&str, _>(6)
                        .parse()
                        .map_err(|e: CoreError| StorageError::Invalid(e.to_string()))?,
                    signature: row.get::<String, _>(7),
                },
                block_hash: BlockHash(parse(1)?),
                event_ids: event_ids.clone(),
            };
            let mut events = vec![];
            for id in &event_ids {
                let body: String = sqlx::query("SELECT body FROM events WHERE event_id = ?")
                    .bind(id.to_string())
                    .fetch_one(&self.pool)
                    .await?
                    .get(0);
                events.push(serde_json::from_str::<EpistemicEvent>(&body)?);
            }
            match ledger.as_mut() {
                None => ledger = Some(Ledger::from_genesis_block(genesis.clone(), record)?),
                Some(l) => l.import_block(record, events)?,
            }
        }
        ledger.ok_or_else(|| StorageError::Invalid("no genesis block".into()))
    }
}

async fn write_projections(tx: &mut sqlx::SqliteConnection, st: &OntologyState) -> Result<()> {
    for t in [
        "identities",
        "validators",
        "branches",
        "claims",
        "claim_versions",
        "ontology_edges",
        "evidence",
        "objections",
        "validations",
        "canonicalization_proposals",
        "artifacts",
        "xchange_assets",
        "research_requests",
        "support_commitments",
        "research_results",
        "simulation_jobs",
        "simulation_results",
        "simulation_verifications",
    ] {
        sqlx::query(&format!("DELETE FROM {t}"))
            .execute(&mut *tx)
            .await?;
    }
    for (id, r) in &st.identities {
        sqlx::query("INSERT INTO identities VALUES (?, ?, ?, ?)")
            .bind(id.to_string())
            .bind(&r.public_key)
            .bind(r.created_at.0)
            .bind(s(r)?)
            .execute(&mut *tx)
            .await?;
    }
    for (id, v) in &st.validators {
        sqlx::query("INSERT INTO validators VALUES (?, ?, ?)")
            .bind(id.to_string())
            .bind(v.identity_id.to_string())
            .bind(s(v)?)
            .execute(&mut *tx)
            .await?;
    }
    for (bid, b) in &st.branches {
        let br = bid.to_string();
        sqlx::query("INSERT INTO branches VALUES (?, ?, ?, ?, ?)")
            .bind(&br)
            .bind(b.branch.parent_branch.as_ref().map(|p| p.to_string()))
            .bind(b.branch.fork_event.map(|e| e.to_string()))
            .bind(b.head.map(|e| e.to_string()))
            .bind(s(&b.branch)?)
            .execute(&mut *tx)
            .await?;
        let bs = &b.state;
        for (cid, c) in &bs.claims {
            sqlx::query("INSERT INTO claims VALUES (?, ?, ?, ?, ?)")
                .bind(&br)
                .bind(cid.to_string())
                .bind(c.current.version as i64)
                .bind(enum_name(&c.current.status))
                .bind(s(c)?)
                .execute(&mut *tx)
                .await?;
            for v in &c.versions {
                sqlx::query("INSERT INTO claim_versions VALUES (?, ?, ?, ?, ?)")
                    .bind(&br)
                    .bind(cid.to_string())
                    .bind(v.claim.version as i64)
                    .bind(v.provenance.event.to_string())
                    .bind(s(v)?)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        for e in &bs.edges {
            sqlx::query("INSERT INTO ontology_edges VALUES (?, ?, ?, ?, ?)")
                .bind(&br)
                .bind(e.source.to_string())
                .bind(enum_name(&e.relation))
                .bind(e.target.to_string())
                .bind(e.originating_event.to_string())
                .execute(&mut *tx)
                .await?;
        }
        for (id, e) in &bs.evidence {
            sqlx::query("INSERT INTO evidence VALUES (?, ?, ?, ?)")
                .bind(&br)
                .bind(id.to_string())
                .bind(e.evidence.claim_id.to_string())
                .bind(s(e)?)
                .execute(&mut *tx)
                .await?;
        }
        for (id, o) in &bs.objections {
            sqlx::query("INSERT INTO objections VALUES (?, ?, ?, ?)")
                .bind(&br)
                .bind(id.to_string())
                .bind(o.resolved)
                .bind(s(o)?)
                .execute(&mut *tx)
                .await?;
        }
        for (id, v) in &bs.validations {
            sqlx::query("INSERT INTO validations VALUES (?, ?, ?, ?)")
                .bind(&br)
                .bind(id.to_string())
                .bind(v.record.validator.to_string())
                .bind(s(v)?)
                .execute(&mut *tx)
                .await?;
        }
        for (id, p) in &bs.proposals {
            sqlx::query("INSERT INTO canonicalization_proposals VALUES (?, ?, ?, ?)")
                .bind(&br)
                .bind(id.to_string())
                .bind(enum_name(&p.state))
                .bind(s(p)?)
                .execute(&mut *tx)
                .await?;
        }
    }
    for (id, a) in &st.artifacts {
        sqlx::query("INSERT INTO artifacts VALUES (?, ?, ?)")
            .bind(id.to_string())
            .bind(&a.uri)
            .bind(s(a)?)
            .execute(&mut *tx)
            .await?;
    }
    for (id, (a, _)) in &st.xchange_assets {
        sqlx::query("INSERT INTO xchange_assets VALUES (?, ?, ?)")
            .bind(id.to_string())
            .bind(enum_name(&a.asset_type))
            .bind(s(a)?)
            .execute(&mut *tx)
            .await?;
    }
    for (id, r) in &st.research_requests {
        sqlx::query("INSERT INTO research_requests VALUES (?, ?)")
            .bind(id.to_string())
            .bind(s(r)?)
            .execute(&mut *tx)
            .await?;
        for (i, c) in r.commitments.iter().enumerate() {
            sqlx::query("INSERT INTO support_commitments VALUES (?, ?, ?, ?, ?)")
                .bind(id.to_string())
                .bind(i as i64)
                .bind(c.supporter.to_string())
                .bind(c.amount_units as i64)
                .bind(s(c)?)
                .execute(&mut *tx)
                .await?;
        }
    }
    for (id, r) in &st.research_results {
        sqlx::query("INSERT INTO research_results VALUES (?, ?, ?, ?)")
            .bind(id.to_string())
            .bind(r.result.request_id.to_string())
            .bind(r.credited)
            .bind(s(r)?)
            .execute(&mut *tx)
            .await?;
    }
    for (id, j) in &st.simulation_jobs {
        sqlx::query("INSERT INTO simulation_jobs VALUES (?, ?)")
            .bind(id.to_string())
            .bind(s(j)?)
            .execute(&mut *tx)
            .await?;
    }
    for (id, r) in &st.simulation_results {
        sqlx::query("INSERT INTO simulation_results VALUES (?, ?, ?)")
            .bind(id.to_string())
            .bind(r.result.job_id.to_string())
            .bind(s(r)?)
            .execute(&mut *tx)
            .await?;
        for (v, ok) in &r.verifications {
            sqlx::query("INSERT INTO simulation_verifications VALUES (?, ?, ?)")
                .bind(id.to_string())
                .bind(v.to_string())
                .bind(*ok)
                .execute(&mut *tx)
                .await?;
        }
    }
    Ok(())
}

/// A ledger backed by a store. Every accepted event is sealed into its own
/// block and persisted before the call returns (decisions D-007).
pub struct PersistentNode {
    pub ledger: Ledger,
    pub store: Store,
    sealer: Option<KeyPair>,
}

impl PersistentNode {
    /// Opens an existing database, or initializes it from `genesis`.
    pub async fn open(
        path: &Path,
        genesis: Option<GenesisConfig>,
        sealer: Option<KeyPair>,
    ) -> Result<Self> {
        let store = Store::open(path).await?;
        let ledger = match store.genesis().await? {
            Some(_) => store.load().await?,
            None => {
                let genesis = genesis.ok_or_else(|| {
                    StorageError::Invalid("database is empty and no genesis was given".into())
                })?;
                let key = sealer.as_ref().ok_or_else(|| {
                    StorageError::Invalid("the block sealer key is needed to create genesis".into())
                })?;
                let ledger = Ledger::new(genesis, key)?;
                store.initialize(&ledger).await?;
                ledger
            }
        };
        Ok(PersistentNode {
            ledger,
            store,
            sealer,
        })
    }

    pub fn can_seal(&self) -> bool {
        self.sealer.is_some()
    }

    /// Validates, applies, seals and persists one event.
    pub async fn submit(&mut self, event: EpistemicEvent) -> Result<EventId> {
        let key = self
            .sealer
            .clone()
            .ok_or_else(|| StorageError::Invalid("this node has no block sealer key".into()))?;
        let checkpoint = self.ledger.clone();
        let id = self.ledger.submit(event)?;
        let record = self
            .ledger
            .seal(&key)?
            .expect("one event is pending")
            .clone();
        if let Err(e) = self.store.append_block(&self.ledger, &record).await {
            self.ledger = checkpoint;
            return Err(e);
        }
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn events_and_blocks_are_append_only() {
        let store = Store::in_memory().await.unwrap();
        sqlx::query("INSERT INTO meta (key, value) VALUES ('x', 'y')")
            .execute(store.pool())
            .await
            .unwrap();
        assert!(sqlx::query("DELETE FROM meta")
            .execute(store.pool())
            .await
            .is_err());
        assert!(sqlx::query("UPDATE events SET body = ''")
            .execute(store.pool())
            .await
            .is_ok()); // no rows yet
        sqlx::query("INSERT INTO events VALUES ('e', 0, 0, 't', 'main', 'a', 0, '{}')")
            .execute(store.pool())
            .await
            .unwrap();
        assert!(sqlx::query("UPDATE events SET body = ''")
            .execute(store.pool())
            .await
            .is_err());
        assert!(sqlx::query("DELETE FROM events")
            .execute(store.pool())
            .await
            .is_err());
    }
}
