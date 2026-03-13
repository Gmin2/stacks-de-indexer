//! Core database engine.

use rusqlite::Connection;
use std::sync::Mutex;

use stacks_indexer_core::config::IndexerConfig;
use stacks_indexer_core::matcher::MatchedEvent;
use stacks_indexer_core::types::BlockPayload;

/// SQLite-backed storage for indexed events and chain state.
///
/// All writes go through a single `Mutex<Connection>`. SQLite WAL mode allows
/// concurrent readers (e.g. the GraphQL API) without blocking the writer.
pub struct Database {
    pub(crate) conn: Mutex<Connection>,
}

impl Database {
    /// Open (or create) the database and initialize all tables.
    ///
    /// # Errors
    ///
    /// Returns an error if the SQLite file cannot be opened or table creation fails.
    pub fn open(config: &IndexerConfig) -> anyhow::Result<Self> {
        if let Some(parent) = std::path::Path::new(&config.storage.path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let conn = Connection::open(&config.storage.path)?;

        // Optimize for write-heavy workloads
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -64000;
             PRAGMA mmap_size = 268435456;
             PRAGMA temp_store = MEMORY;",
        )?;

        let db = Self {
            conn: Mutex::new(conn),
        };

        db.create_system_tables()?;
        db.create_event_tables(config)?;

        Ok(db)
    }

    /// Create the `_indexer_state` and `_reorg_journal` system tables.
    fn create_system_tables(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _indexer_state (
                block_height INTEGER NOT NULL,
                block_hash TEXT NOT NULL,
                index_block_hash TEXT NOT NULL,
                parent_index_block_hash TEXT NOT NULL,
                block_time INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS _reorg_journal (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                block_height INTEGER NOT NULL,
                block_hash TEXT NOT NULL,
                table_name TEXT NOT NULL,
                row_id INTEGER NOT NULL,
                operation TEXT NOT NULL DEFAULT 'INSERT',
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_reorg_journal_height
                ON _reorg_journal(block_height);",
        )?;
        Ok(())
    }

    /// Create event tables from the YAML configuration.
    fn create_event_tables(&self, config: &IndexerConfig) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();

        for source in &config.sources {
            for event_cfg in &source.events {
                let table = &event_cfg.table;

                conn.execute(
                    &format!(
                        "CREATE TABLE IF NOT EXISTS \"{table}\" (
                            _id INTEGER PRIMARY KEY AUTOINCREMENT,
                            _block_height INTEGER NOT NULL,
                            _block_hash TEXT NOT NULL,
                            _tx_id TEXT NOT NULL,
                            _event_index INTEGER NOT NULL,
                            _timestamp INTEGER NOT NULL DEFAULT 0,
                            _event_type TEXT NOT NULL,
                            _raw_data TEXT NOT NULL
                        )"
                    ),
                    [],
                )?;

                conn.execute(
                    &format!(
                        "CREATE INDEX IF NOT EXISTS \"idx_{table}_block_height\" \
                         ON \"{table}\" (_block_height)"
                    ),
                    [],
                )?;
                conn.execute(
                    &format!(
                        "CREATE INDEX IF NOT EXISTS \"idx_{table}_tx_id\" \
                         ON \"{table}\" (_tx_id)"
                    ),
                    [],
                )?;

                for field in &event_cfg.indexes {
                    conn.execute(
                        &format!(
                            "CREATE INDEX IF NOT EXISTS \"idx_{table}_{field}\" \
                             ON \"{table}\" (json_extract(_raw_data, '$.{field}'))"
                        ),
                        [],
                    )?;
                }
            }
        }

        Ok(())
    }

    /// Get the last processed block height and hash.
    ///
    /// Returns `(0, "")` if no blocks have been processed yet.
    pub fn get_last_processed_block(&self) -> anyhow::Result<(u64, String)> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT block_height, block_hash FROM _indexer_state \
             ORDER BY block_height DESC LIMIT 1",
            [],
            |row| Ok((row.get::<_, i64>(0)? as u64, row.get::<_, String>(1)?)),
        );
        match result {
            Ok(v) => Ok(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok((0, String::new())),
            Err(e) => Err(e.into()),
        }
    }

    /// Get the `index_block_hash` of the current chain tip.
    pub fn get_last_index_block_hash(&self) -> anyhow::Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT index_block_hash FROM _indexer_state \
             ORDER BY block_height DESC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(hash) => Ok(Some(hash)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Store a block's matched events and update indexer state atomically.
    pub fn apply_block(
        &self,
        block: &BlockPayload,
        matched_events: &[MatchedEvent],
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;

        for event in matched_events {
            let raw_data = serde_json::to_string(&event.envelope.event.to_json())?;

            tx.execute(
                &format!(
                    "INSERT INTO \"{}\" \
                     (_block_height, _block_hash, _tx_id, _event_index, _timestamp, _event_type, _raw_data) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    event.table
                ),
                rusqlite::params![
                    block.block_height as i64,
                    &block.block_hash,
                    &event.envelope.txid,
                    event.envelope.event_index as i64,
                    block.block_time.unwrap_or(0) as i64,
                    event.envelope.event.type_name(),
                    raw_data,
                ],
            )?;

            let row_id = tx.last_insert_rowid();

            tx.execute(
                "INSERT INTO _reorg_journal \
                 (block_height, block_hash, table_name, row_id, operation) \
                 VALUES (?1, ?2, ?3, ?4, 'INSERT')",
                rusqlite::params![
                    block.block_height as i64,
                    &block.block_hash,
                    &event.table,
                    row_id,
                ],
            )?;
        }

        tx.execute(
            "INSERT INTO _indexer_state \
             (block_height, block_hash, index_block_hash, parent_index_block_hash, block_time) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                block.block_height as i64,
                &block.block_hash,
                &block.index_block_hash,
                &block.parent_index_block_hash,
                block.block_time.unwrap_or(0) as i64,
            ],
        )?;

        tx.commit()?;
        Ok(())
    }

    /// Roll back all data from `height` (inclusive) and above.
    ///
    /// Replays the reorg journal in reverse, deleting inserted rows.
    /// Returns the number of journal entries undone.
    pub fn rollback_from_height(&self, height: u64) -> anyhow::Result<u64> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;

        let mut stmt = tx.prepare(
            "SELECT id, table_name, row_id, operation FROM _reorg_journal \
             WHERE block_height >= ?1 ORDER BY id DESC",
        )?;

        let entries: Vec<(i64, String, i64, String)> = stmt
            .query_map([height as i64], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        let count = entries.len() as u64;

        for (_id, table_name, row_id, operation) in &entries {
            if operation == "INSERT" {
                tx.execute(
                    &format!("DELETE FROM \"{table_name}\" WHERE _id = ?1"),
                    [row_id],
                )?;
            }
        }

        tx.execute(
            "DELETE FROM _reorg_journal WHERE block_height >= ?1",
            [height as i64],
        )?;
        tx.execute(
            "DELETE FROM _indexer_state WHERE block_height >= ?1",
            [height as i64],
        )?;

        tx.commit()?;
        Ok(count)
    }

    /// Delete journal entries older than `keep_blocks` behind the tip.
    pub fn prune_journal(&self, keep_blocks: u64) -> anyhow::Result<()> {
        let (tip, _) = self.get_last_processed_block()?;
        if tip <= keep_blocks {
            return Ok(());
        }
        let cutoff = tip - keep_blocks;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM _reorg_journal WHERE block_height < ?1",
            [cutoff as i64],
        )?;
        Ok(())
    }

    /// Get row counts for all configured event tables.
    pub fn table_row_counts(&self, config: &IndexerConfig) -> anyhow::Result<Vec<(String, u64)>> {
        let conn = self.conn.lock().unwrap();
        let mut counts = Vec::new();
        for source in &config.sources {
            for ev in &source.events {
                let count = conn
                    .query_row(
                        &format!("SELECT COUNT(*) FROM \"{}\"", ev.table),
                        [],
                        |row| row.get::<_, i64>(0),
                    )
                    .unwrap_or(0) as u64;
                counts.push((ev.table.clone(), count));
            }
        }
        Ok(counts)
    }

    /// Query an event table with filters, ordering, and pagination.
    ///
    /// Filters are `(field, operator, value)` tuples. Fields starting with `_`
    /// query system columns directly; others use `json_extract` on `_raw_data`.
    ///
    /// Returns `(rows, total_count)`.
    pub fn query_table(
        &self,
        table: &str,
        filters: &[(String, String, serde_json::Value)],
        order_by: Option<(&str, &str)>,
        limit: u32,
        offset: u32,
    ) -> anyhow::Result<(Vec<serde_json::Value>, u64)> {
        let conn = self.conn.lock().unwrap();

        let mut where_clauses = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        for (i, (field, op, value)) in filters.iter().enumerate() {
            let col = if field.starts_with('_') {
                format!("\"{field}\"")
            } else {
                format!("json_extract(_raw_data, '$.{field}')")
            };

            let sql_op = match op.as_str() {
                "eq" => "=",
                "neq" => "!=",
                "gt" => ">",
                "gte" => ">=",
                "lt" => "<",
                "lte" => "<=",
                "like" => "LIKE",
                _ => "=",
            };

            where_clauses.push(format!("{col} {sql_op} ?{}", i + 1));

            match value {
                serde_json::Value::String(s) => params.push(Box::new(s.clone())),
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        params.push(Box::new(i));
                    } else if let Some(f) = n.as_f64() {
                        params.push(Box::new(f));
                    }
                }
                serde_json::Value::Bool(b) => params.push(Box::new(*b)),
                _ => params.push(Box::new(value.to_string())),
            }
        }

        let where_sql = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        let order_sql = match order_by {
            Some((field, dir)) => {
                let col = if field.starts_with('_') {
                    format!("\"{field}\"")
                } else {
                    format!("json_extract(_raw_data, '$.{field}')")
                };
                format!("ORDER BY {col} {dir}")
            }
            None => "ORDER BY _id DESC".to_string(),
        };

        let refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let total: u64 = conn
            .query_row(
                &format!("SELECT COUNT(*) FROM \"{table}\" {where_sql}"),
                refs.as_slice(),
                |row| row.get::<_, i64>(0),
            )
            .map(|c| c as u64)?;

        let refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&format!(
            "SELECT _id, _block_height, _block_hash, _tx_id, _event_index, \
                    _timestamp, _event_type, _raw_data \
             FROM \"{table}\" {where_sql} {order_sql} LIMIT {limit} OFFSET {offset}"
        ))?;

        let rows = stmt
            .query_map(refs.as_slice(), |row| {
                let raw_data: String = row.get(7)?;
                let data: serde_json::Value =
                    serde_json::from_str(&raw_data).unwrap_or(serde_json::Value::Null);
                Ok(serde_json::json!({
                    "_id": row.get::<_, i64>(0)?,
                    "_block_height": row.get::<_, i64>(1)?,
                    "_block_hash": row.get::<_, String>(2)?,
                    "_tx_id": row.get::<_, String>(3)?,
                    "_event_index": row.get::<_, i64>(4)?,
                    "_timestamp": row.get::<_, i64>(5)?,
                    "_event_type": row.get::<_, String>(6)?,
                    "data": data,
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok((rows, total))
    }
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use stacks_indexer_core::config::parse_config;

    fn test_config() -> IndexerConfig {
        parse_config(
            r#"
name: test
network: devnet
server: { event_listener_port: 20445, api_port: 4000 }
storage: { path: ":memory:" }
sources:
  - contract: "SP1.vault"
    events:
      - { name: vault_created, type: print_event, table: vaults, indexes: ["owner"] }
"#,
        )
        .unwrap()
    }

    fn make_block(height: u64, hash: &str, parent: &str) -> BlockPayload {
        BlockPayload {
            block_hash: hash.to_string(),
            block_height: height,
            block_time: Some(1_700_000_000 + height),
            burn_block_hash: "0xburn".into(),
            burn_block_height: 800_000,
            miner_txid: None,
            burn_block_time: None,
            index_block_hash: format!("0xidx_{hash}"),
            parent_block_hash: parent.to_string(),
            parent_index_block_hash: format!("0xidx_{parent}"),
            parent_microblock: None,
            parent_microblock_sequence: None,
            consensus_hash: None,
            tenure_height: None,
            transactions: vec![],
            events: vec![],
            parent_burn_block_hash: None,
            parent_burn_block_height: None,
            parent_burn_block_timestamp: None,
            anchored_cost: None,
            confirmed_microblocks_cost: None,
            signer_bitvec: None,
            reward_set: None,
            cycle_number: None,
            signer_signature_hash: None,
            miner_signature: None,
            signer_signature: vec![],
            matured_miner_rewards: vec![],
            pox_v1_unlock_height: None,
            pox_v2_unlock_height: None,
            pox_v3_unlock_height: None,
        }
    }

    #[test]
    fn open_creates_tables() {
        let cfg = test_config();
        let db = Database::open(&cfg).unwrap();
        let (height, _) = db.get_last_processed_block().unwrap();
        assert_eq!(height, 0);
    }

    #[test]
    fn apply_block_updates_state() {
        let cfg = test_config();
        let db = Database::open(&cfg).unwrap();
        let block = make_block(100, "0xaaa", "0x000");
        db.apply_block(&block, &[]).unwrap();

        let (height, hash) = db.get_last_processed_block().unwrap();
        assert_eq!(height, 100);
        assert_eq!(hash, "0xaaa");
    }

    #[test]
    fn rollback_removes_blocks() {
        let cfg = test_config();
        let db = Database::open(&cfg).unwrap();

        for i in 1..=3 {
            let block = make_block(i, &format!("0x{i:03}"), &format!("0x{:03}", i - 1));
            db.apply_block(&block, &[]).unwrap();
        }

        assert_eq!(db.get_last_processed_block().unwrap().0, 3);

        db.rollback_from_height(2).unwrap();

        assert_eq!(db.get_last_processed_block().unwrap().0, 1);
    }
}
