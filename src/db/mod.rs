#![allow(dead_code)]

#[cfg(feature = "legacy-rocks")]
pub mod rocks;
#[cfg(feature = "legacy-sqlite-v1")]
pub mod sqlite_v1;

use crate::lang::{Result, ResultExt};
use crate::models::{Block, IcxTxSet, Transaction};
use rusqlite::{params, CachedStatement, Connection, OptionalExtension};
use std::collections::HashMap;

pub fn sqlite_init_db_v2(path: Option<&str>) -> Result<Connection> {
    let path = path.unwrap_or("data/index.sqlite");
    let conn = rusqlite::Connection::open(path)?;
    sqlite_init_pragma_v1(&conn)?;
    sqlite_init_tables_v2(&conn)?;
    Ok(conn)
}

fn sqlite_init_pragma_v1(conn: &Connection) -> Result<()> {
    let pragmas = [
        // "pragma locking_mode=exclusive",
        "pragma journal_mode=wal",
        "pragma secure_delete=off",
        "pragma synchronous=normal",
        "pragma analysis_limit=1000",         // recommended
        "pragma wal_autocheckpoint=1000",     // default
        "pragma page_size=4096",              // default
        "pragma auto_vacuum=0",               // 0| none / 1| full / 2|incremental
        "pragma journal_size_limit=67108864", // 1024 * 1024 * 64 // default: -1
        "pragma wal_checkpoint(truncate)",    // let's restart the wal
    ];

    for pragma in &pragmas {
        conn.execute_batch(pragma).ext()?;
    }
    Ok(())
}

fn sqlite_init_tables_v2(conn: &Connection) -> Result<()> {
    // height is coalesced into rowid, so height is stored in the btree
    // and rest is stored on the leaf data page.
    // Note: We add the unique index directly in table to ensure lookups
    // can happen while indexing.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS blocks (
            height INTEGER PRIMARY KEY,
            hash TEXT UNIQUE NOT NULL,
            data TEXT NOT NULL
        )",
        [],
    )?;

    // Note that using text as primary is similar to just an additional
    // index as sqlite will add implicit rowid as the btree* key.
    // We want this as rowid (int), is significantly cheaper to add other
    // indexes on top.
    // DVM out is always all DVM addresses, both in and out.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS txs (
            txid TEXT PRIMARY KEY,
            height INTEGER NOT NULL,
            tx_type TEXT NOT NULL,
            tx_in TEXT NOT NULL,
            tx_out TEXT NOT NULL,
            dvm_in TEXT NOT NULL,
            dvm_out TEXT NOT NULL,
            data TEXT NOT NULL,
            icx_data TEXT NOT NULL,
            icx_addr TEXT NOT NULL,
            icx_btc_exp_amt TEXT NOT NULL,
            swap_from TEXT NOT NULL,
            swap_to TEXT NOT NULL,
            swap_amt TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS tx_addr_graph (
            rowid INTEGER PRIMARY KEY,
            txid TEXT NOT NULL,
            in_addr TEXT NOT NULL,
            out_addr TEXT NOT NULL,
            c_flags TEXT NOT NULL,
            UNIQUE (txid, in_addr, out_addr)
        )",
        [],
    )?;

    Ok(())
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct TxRow<'a> {
    pub txid: String,
    pub height: i64,
    pub tx_type: String,
    pub tx_in: HashMap<String, f64>,
    pub tx_out: HashMap<String, f64>,
    pub dvm_in: Vec<String>,
    pub dvm_out: Vec<String>,
    pub data: Transaction,
    pub icx_data: IcxTxSet<'a>,
    pub icx_addr: String,
    pub icx_btc_exp_amt: String,
    pub swap_from: String,
    pub swap_to: String,
    pub swap_amt: String,
}

impl<'a> TxRow<'a> {
    pub fn from_sqlite_row(row: &rusqlite::Row) -> Result<Self> {
        let mut v = TxRow::from_sqlite_row_partial(row)?;
        let data_str = row.get::<_, String>(7)?;
        let icx_data_str = row.get::<_, String>(8)?;
        if !data_str.is_empty() {
            v.data = serde_json::from_str(&data_str)?;
        }
        if !icx_data_str.is_empty() {
            v.icx_data = serde_json::from_str(&icx_data_str)?;
        }
        Ok(v)
    }

    pub fn from_sqlite_row_partial(row: &rusqlite::Row) -> Result<Self> {
        let tx_in_ref = row.get_ref(3)?;
        let tx_in_str = tx_in_ref.as_str().map_err(|_| "tx_in error")?;
        let tx_in = if tx_in_str.is_empty() {
            HashMap::new()
        } else {
            serde_json::from_str(tx_in_str)?
        };

        let tx_out_ref = row.get_ref(4)?;
        let tx_out_str = tx_out_ref.as_str().map_err(|_| "tx_out error")?;
        let tx_out = if tx_out_str.is_empty() {
            HashMap::new()
        } else {
            serde_json::from_str(tx_out_str)?
        };

        let dvm_in_ref = row.get_ref(5)?;
        let dvm_in_str = dvm_in_ref.as_str().map_err(|_| "dvm_in error")?;
        let dvm_in = if dvm_in_str.is_empty() {
            Vec::new()
        } else {
            serde_json::from_str(dvm_in_str)?
        };

        let dvm_out_ref = row.get_ref(6)?;
        let dvm_out_str = dvm_out_ref.as_str().map_err(|_| "dvm_out error")?;
        let dvm_out = if dvm_out_str.is_empty() {
            Vec::new()
        } else {
            serde_json::from_str(dvm_out_str)?
        };
        Ok(Self {
            txid: row.get(0)?,
            height: row.get(1)?,
            tx_type: row.get(2)?,
            tx_in,
            tx_out,
            dvm_in,
            dvm_out,
            data: Transaction::default(), // Placeholder or default value
            icx_data: IcxTxSet::default(), // Placeholder or default value
            icx_addr: row.get(9)?,
            icx_btc_exp_amt: row.get(10)?,
            swap_from: row.get(11)?,
            swap_to: row.get(12)?,
            swap_amt: row.get(13)?,
        })
    }
}

pub fn sqlite_create_index_factory_v2(
    conn: &rusqlite::Connection,
) -> impl Iterator<Item = (&str, impl Fn() -> rusqlite::Result<()> + '_)> {
    let indexes = vec![
        (
            "CREATE INDEX IF NOT EXISTS idx_txs_height ON txs (height)",
            "idx_txs_height",
        ),
        (
            "CREATE INDEX IF NOT EXISTS idx_txs_tx_type ON txs (tx_type)",
            "idx_txs_tx_type",
        ),
        (
            "CREATE INDEX IF NOT EXISTS idx_txs_icx_addr ON txs (icx_addr)",
            "idx_txs_icx_addr",
        ),
        (
            "CREATE INDEX IF NOT EXISTS idx_txs_swap_from ON txs (swap_from)",
            "idx_txs_swap_from",
        ),
        (
            "CREATE INDEX IF NOT EXISTS idx_txs_swap_to ON txs (swap_to)",
            "idx_txs_swap_to",
        ),
        (
            "CREATE INDEX IF NOT EXISTS idx_tx_addr_graph_txid ON tx_addr_graph (txid)",
            "idx_tx_addr_graph_txid",
        ),
        (
            "CREATE INDEX IF NOT EXISTS idx_tx_addr_graph_in_addr ON tx_addr_graph (in_addr)",
            "idx_tx_addr_graph_in_addr",
        ),
        (
            "CREATE INDEX IF NOT EXISTS idx_tx_addr_graph_out_addr ON tx_addr_graph (out_addr)",
            "idx_tx_addr_graph_out_addr",
        ),
    ];

    let mut itr = indexes.into_iter();

    std::iter::from_fn(move || {
        if let Some((query, name)) = itr.next() {
            let closure = Box::new(|| conn.execute(query, []).map(|_| ()));
            return Some((name, closure));
        }
        None
    })
}

pub fn sqlite_get_stmts_v2(conn: &rusqlite::Connection) -> Result<[CachedStatement<'_>; 3]> {
    let insert_block_stmt = conn.prepare_cached(
        "
        insert or replace into blocks (height, hash, data)
        values (?1, ?2, ?3)
    ",
    )?;

    let insert_tx_stmt = conn.prepare_cached(
        "
        insert or replace into txs (
            txid, height, tx_type, tx_in, tx_out, dvm_in, dvm_out, data, icx_data, icx_addr, icx_btc_exp_amt, swap_from, swap_to, swap_amt
        )
        values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
    ",
    )?;

    let insert_tx_addr_graph_stmt = conn.prepare_cached(
        "
        insert or replace into tx_addr_graph (txid, in_addr, out_addr, c_flags)
        values (?1, ?2, ?3, ?4)
    ",
    )?;

    Ok([insert_block_stmt, insert_tx_stmt, insert_tx_addr_graph_stmt])
}

// Raw tx to get around the borrow checker.
pub fn sqlite_begin_tx(conn: &rusqlite::Connection) -> Result<usize> {
    conn.execute("begin transaction", []).ext()
}

pub fn sqlite_commit_tx(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch("commit").ext()
}

pub fn sqlite_commit_and_begin_tx(conn: &rusqlite::Connection) -> Result<usize> {
    sqlite_commit_tx(conn)?;
    sqlite_begin_tx(conn)
}

// Block Store

#[derive(Debug, Clone)]
pub struct TxAddrData {
    pub tx_type: String,
    pub tx_in: HashMap<String, f64>,
    pub tx_out: HashMap<String, f64>,
}

pub trait BlockStore {
    fn get_block_from_hash(&self, hash: &str) -> Result<Option<Block>>;
    fn get_block_hash(&self, height: i64) -> Result<Option<String>>;
    fn get_block_hash_for_tx(&self, tx_hash: &str) -> Result<Option<String>>;
    fn get_block_for_tx(&self, tx_hash: &str) -> Result<Option<Block>>;
    fn get_block_from_height(&self, height: i64) -> Result<Option<Block>>;
    fn get_tx_from_hash(&self, hash: &str) -> Result<Option<Transaction>>;
    fn get_tx_addr_data_from_hash(&self, hash: &str) -> Result<Option<TxAddrData>>;
}

impl BlockStore for SqliteBlockStore {
    fn get_block_from_hash(&self, hash: &str) -> Result<Option<Block>> {
        self.get_block_from_hash(hash)
    }

    fn get_block_hash(&self, height: i64) -> Result<Option<String>> {
        self.get_block_hash(height)
    }

    fn get_block_hash_for_tx(&self, tx_hash: &str) -> Result<Option<String>> {
        self.get_block_hash_for_tx(tx_hash)
    }

    fn get_block_for_tx(&self, tx_hash: &str) -> Result<Option<Block>> {
        self.get_block_for_tx(tx_hash)
    }

    fn get_block_from_height(&self, height: i64) -> Result<Option<Block>> {
        self.get_block_from_height(height)
    }

    fn get_tx_from_hash(&self, hash: &str) -> Result<Option<Transaction>> {
        self.get_tx_from_hash(hash)
    }

    fn get_tx_addr_data_from_hash(&self, hash: &str) -> Result<Option<TxAddrData>> {
        self.get_tx_addr_data_from_hash(hash)
    }
}

pub struct SqliteBlockStore {
    pub conn: Connection,
}

impl SqliteBlockStore {
    #[cfg(feature = "legacy-sqlite-v1")]
    pub fn new_v1(path: Option<&str>) -> Result<Self> {
        let conn = crate::db::sqlite_v1::sqlite_init_db_v1(path)?;
        Ok(Self { conn })
    }

    pub fn new_v2(path: Option<&str>) -> Result<Self> {
        let conn = sqlite_init_db_v2(path)?;
        Ok(Self { conn })
    }

    // Note index for this might not be there in the beginning.
    pub fn get_block_hash(&self, height: i64) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT hash FROM blocks WHERE height = ?1")?;
        let hash: Option<String> = stmt
            .query_row(params![height], |row| row.get(0))
            .optional()?;
        Ok(hash)
    }

    pub fn get_block_hash_for_tx(&self, tx_hash: &str) -> Result<Option<String>> {
        // We do the filter before join to ensure we join on the filtered
        // and not other way
        let query = "
            SELECT b.hash
            FROM blocks b
            JOIN (
                SELECT height
                FROM txs
                WHERE txid = ?1
                LIMIT 1
            ) t ON b.height = t.height
        ";
        let mut stmt = self.conn.prepare_cached(query)?;
        let hash: Option<String> = stmt
            .query_row(params![tx_hash], |row| row.get(0))
            .optional()?;

        Ok(hash)
    }

    pub fn get_block_from_height(&self, height: i64) -> Result<Option<Block>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT data FROM blocks WHERE height = ?1")?;
        let block: Option<String> = stmt
            .query_row(params![height], |row| row.get(0))
            .optional()?;
        match block {
            Some(data) => {
                let block: Block = serde_json::from_str(&data)?;
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    pub fn get_block_from_hash(&self, hash: &str) -> Result<Option<Block>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT data FROM blocks WHERE hash = ?1")?;
        let block: Option<String> = stmt.query_row(params![hash], |row| row.get(0)).optional()?;
        match block {
            Some(data) => {
                let block: Block = serde_json::from_str(&data)?;
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    pub fn get_block_for_tx(&self, tx_hash: &str) -> Result<Option<Block>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT height FROM txs WHERE txid = ?1 limit 1")?;
        let height: Option<i64> = stmt
            .query_row(params![tx_hash], |row| row.get(0))
            .optional()?;
        match height {
            Some(h) => self.get_block_from_height(h),
            None => Ok(None),
        }
    }

    pub fn get_tx_from_hash(&self, hash: &str) -> Result<Option<Transaction>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT data FROM txs WHERE txid = ?1")?;
        let tx: Option<String> = stmt.query_row(params![hash], |row| row.get(0)).optional()?;
        match tx {
            Some(data) => {
                let tx: Transaction = serde_json::from_str(&data)?;
                Ok(Some(tx))
            }
            None => Ok(None),
        }
    }

    pub fn get_tx_addr_data_from_hash(&self, hash: &str) -> Result<Option<TxAddrData>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT tx_in, tx_out, tx_type FROM txs WHERE txid = ?1")?;
        let tx_data: Option<(String, String, String)> = stmt
            .query_row(params![hash], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .optional()?;

        match tx_data {
            Some((tx_in_data, tx_out_data, tx_type)) => {
                let tx_in: HashMap<String, f64> = serde_json::from_str(&tx_in_data)?;
                let tx_out: HashMap<String, f64> = serde_json::from_str(&tx_out_data)?;

                let tx_addr_data = TxAddrData {
                    tx_type,
                    tx_in,
                    tx_out,
                };

                Ok(Some(tx_addr_data))
            }
            None => Ok(None),
        }
    }

    pub fn iter_blocks<F>(&self, modifier: Option<&str>, mut f: F) -> Result<()>
    where
        F: FnMut(Result<Block>) -> Result<()>,
    {
        let query = match modifier {
            Some(ext) => format!("SELECT data FROM blocks {}", ext),
            None => "SELECT data FROM blocks".to_string(),
        };
        let mut stmt = self.conn.prepare(&query)?;
        let mut q = stmt.query([])?;
        while let Some(row) = q.next()? {
            let data: &str = row.get_ref(0)?.as_str().map_err(|_| "ref error")?;
            let block: Result<Block> = serde_json::from_str(data).map_err(|e| e.into());
            f(block)?;
        }
        Ok(())
    }

    pub fn iter_txs<F>(&self, modifier: Option<&str>, mut f: F) -> Result<()>
    where
        F: FnMut(Result<TxRow>) -> Result<()>,
    {
        let query = match modifier {
            Some(ext) => format!("SELECT * FROM txs {}", ext),
            None => "SELECT * FROM txs".to_string(),
        };
        let mut stmt = self.conn.prepare(&query)?;
        let mut q = stmt.query([])?;
        while let Some(row) = q.next()? {
            let tx_row = TxRow::from_sqlite_row(row)?;
            f(Ok(tx_row))?;
        }
        Ok(())
    }

    pub fn iter_txs_partial<F>(&self, modifier: Option<&str>, mut f: F) -> Result<()>
    where
        F: FnMut(Result<TxRow>) -> Result<()>,
    {
        let query = match modifier {
            Some(ext) => format!("SELECT * FROM txs {}", ext),
            None => "SELECT * FROM txs".to_string(),
        };
        let mut stmt = self.conn.prepare(&query)?;
        let mut q = stmt.query([])?;
        while let Some(row) = q.next()? {
            // debug!("{:?}", row);
            let tx_row = TxRow::from_sqlite_row_partial(row)?;
            // let tx_row = TxRow::default();
            f(Ok(tx_row))?;
        }
        Ok(())
    }
}
