#![allow(dead_code)]

use crate::lang::ResultExt;
use crate::models::{Block, Transaction};
use crate::Result;
use rusqlite::{params, CachedStatement, Connection, OptionalExtension};
use rust_rocksdb::{ColumnFamily, ColumnFamilyDescriptor, CompactOptions, Options, DB};
use std::collections::HashMap;
use tracing::info;

pub fn sqlite_init_db(path: Option<&str>) -> Result<Connection> {
    let path = path.unwrap_or("data/index.sqlite");
    let conn = rusqlite::Connection::open(path)?;

    conn.execute_batch("pragma locking_mode=exclusive").ext()?;
    conn.execute_batch("pragma journal_mode=wal2").ext()?;
    conn.execute_batch("pragma secure_delete=off").ext()?;
    conn.execute_batch("pragma synchronous=normal").ext()?;
    conn.execute_batch("pragma analysis_limit=1000").ext()?; // recommended
    conn.execute_batch("pragma wal_autocheckpoint=1000").ext()?; // default
    conn.execute_batch("pragma page_size=4096").ext()?; // default
    conn.execute_batch("pragma auto_vacuum=0").ext()?; // 0| none / 1| full / 2|incremental
    conn.execute_batch("pragma journal_size_limit=67108864")
        .ext()?; // 1024 * 1024 * 64 // default: -1
    conn.execute_batch("pragma wal_checkpoint(truncate)")
        .ext()?; // let's restart the wal

    // height is coalesced into rowid, so height is stored in the btree
    // and rest is stored on the leaf data page.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS blocks (
            height INTEGER PRIMARY KEY,
            hash TEXT NOT NULL,
            data TEXT NOT NULL
        )",
        [],
    )?;

    // Note that using text as primary is similar to just an additional
    // index as sqlite will add implicit rowid as the btree* key.
    // We want this as rowid (int), is significantly cheaper to add other
    // indexes on top.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS txs (
            txid TEXT PRIMARY KEY,
            height INTEGER NOT NULL,
            tx_type TEXT NOT NULL,
            tx_in TEXT NOT NULL,
            tx_out TEXT NOT NULL,
            dvm_addrs TEXT NOT NULL,
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
        "CREATE TABLE IF NOT EXISTS tx_graph (
            rowid INTEGER PRIMARY KEY,
            tx_in_addr TEXT NOT NULL,
            txid TEXT NOT NULL,
            tx_out_addr TEXT NOT NULL,
            c_flags TEXT NOT NULL
        )",
        [],
    )?;

    Ok(conn)
}

pub fn sqlite_get_stmts(conn: &rusqlite::Connection) -> Result<[CachedStatement<'_>; 3]> {
    let insert_block_stmt = conn.prepare_cached(
        "
        insert or replace into blocks (height, hash, data)
        values (?1, ?2, ?3)
    ",
    )?;

    let insert_tx_stmt = conn.prepare_cached(
        "
        insert or replace into txs (
            txid, height, tx_type, tx_in, tx_out, dvm_addrs, data, icx_data, icx_addr, icx_btc_exp_amt, swap_from, swap_to, swap_amt
        )
        values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
    ",
    )?;

    let insert_tx_graph_stmt = conn.prepare_cached(
        "
        insert or replace into tx_graph (tx_in_addr, txid, tx_out_addr, c_flags)
        values (?1, ?2, ?3, ?4)
    ",
    )?;

    Ok([insert_block_stmt, insert_tx_stmt, insert_tx_graph_stmt])
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

pub fn rocks_open_db(path: Option<&str>) -> Result<DB> {
    let db_path = path.unwrap_or("data/db");
    let rocks_opts = rocks_get_db_opts()?;
    let cf_tx = ColumnFamilyDescriptor::new("tx", rocks_opts);
    let db = DB::open_cf_descriptors(&rocks_get_db_opts()?, db_path, vec![cf_tx])?;
    Ok(db)
}

pub fn rocks_get_db_opts() -> Result<Options> {
    use rust_rocksdb::{BlockBasedOptions, Cache, DBCompressionType};
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_size(64 << 10); // kb
    block_opts.set_block_cache(&Cache::new_lru_cache(64 << 20)); // mb
    block_opts.set_cache_index_and_filter_blocks(true);
    block_opts.set_bloom_filter(10.0, true);
    // block_opts.set_pin_top_level_index_and_filter(true);

    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    opts.set_write_buffer_size(64 << 20); // mb
    opts.set_max_write_buffer_number(2);
    opts.set_min_blob_size(2 << 10); // kb
                                     // opts.set_blob_file_size(256 << 20); // mb
    opts.set_enable_blob_files(true);
    opts.set_enable_blob_gc(true);
    opts.set_enable_pipelined_write(true);

    opts.set_compression_type(DBCompressionType::Lz4);
    opts.set_wal_compression_type(DBCompressionType::Zstd);
    opts.set_blob_compression_type(DBCompressionType::Lz4);
    opts.set_bottommost_compression_type(DBCompressionType::Zstd);
    opts.set_block_based_table_factory(&block_opts);
    opts.enable_statistics();
    opts.increase_parallelism(std::thread::available_parallelism()?.get().try_into()?);
    opts.set_level_compaction_dynamic_level_bytes(true);
    Ok(opts)
}

pub fn rocks_compact_db(db: &DB) -> Result<()> {
    info!("start compaction");
    let mut compact_opts = CompactOptions::default();
    compact_opts.set_exclusive_manual_compaction(true);
    compact_opts.set_change_level(true);
    compact_opts.set_bottommost_level_compaction(rust_rocksdb::BottommostLevelCompaction::Force);
    db.compact_range_opt(
        Option::<[u8; 0]>::None,
        Option::<[u8; 0]>::None,
        &compact_opts,
    );
    info!("done compaction");
    Ok(())
}

// We encode height such that it's naturally sortable instead of lexicographic
// Note this doesn't optimize in anyway, just a quick one that sorts
// Uses - prefix for negatives, so they are sorted first.
// Append the length of the digits next in hex, followed by the number itself.
// So this can work for upto 16 digit numbers.
pub fn encode_height(height: i64) -> String {
    let height_abs = height.abs().to_string();
    let is_neg = if height < 0 { "-" } else { "" };
    let length = height_abs.len();
    format!("{is_neg}{length:x}{height_abs}")
}

pub struct BlockStore<'a> {
    db: &'a DB,
    cf_tx: &'a ColumnFamily,
}

#[derive(Debug, Clone)]
pub struct TxAddrData {
    pub tx_type: String,
    pub tx_in: HashMap<String, f64>,
    pub tx_out: HashMap<String, f64>,
}

impl<'a> BlockStore<'a> {
    pub fn new(db: &'a DB) -> Result<Self> {
        let cf_tx = db
            .cf_handle("tx")
            .ok_or(crate::lang::Error::from("cf handle"))?;
        Ok(Self { db, cf_tx })
    }

    pub fn get_block_from_hash(&self, hash: &str) -> Result<Option<Block>> {
        let key = "b/x/".to_owned() + hash;
        let res = self.db.get(key)?;
        if res.is_some() {
            let raw = res.unwrap();
            let s = std::str::from_utf8(&raw)?;
            let v: Block = serde_json::from_str(s)?;
            Ok(Some(v))
        } else {
            Ok(None)
        }
    }

    pub fn get_block_hash(&self, height: i64) -> Result<Option<String>> {
        let res = self.db.get("b/h/".to_owned() + &encode_height(height))?;
        match res {
            Some(v) => Ok(Some(String::from_utf8(v)?)),
            None => Ok(None),
        }
    }

    pub fn get_block_hash_for_tx(&self, tx_hash: &str) -> Result<Option<String>> {
        let res = self.db.get("t/h/".to_owned() + tx_hash)?;
        match res {
            Some(v) => Ok(Some(String::from_utf8(v)?)),
            None => Ok(None),
        }
    }

    pub fn get_block_for_tx(&self, tx_hash: &str) -> Result<Option<Block>> {
        let block_hash = self.get_block_hash_for_tx(tx_hash)?;
        match block_hash {
            Some(v) => self.get_block_from_hash(&v),
            None => Ok(None),
        }
    }

    pub fn get_block_from_height(&self, height: i64) -> Result<Option<Block>> {
        let block_hash = self.get_block_hash(height)?;
        match block_hash {
            Some(v) => self.get_block_from_hash(&v),
            None => Ok(None),
        }
    }

    pub fn get_tx_from_hash(&self, hash: &str) -> Result<Option<Transaction>> {
        let block = self.get_block_for_tx(hash)?;
        if block.is_none() {
            return Ok(None);
        };
        let block = block.unwrap();
        let tx = block.tx.iter().find(|x| x.txid == hash);
        match tx {
            Some(v) => Ok(Some(v.clone())),
            None => Err(anyhow::format_err!("block found, no but tx with hash: {}", hash).into()),
        }
    }

    pub fn get_tx_addr_data_from_hash(&self, hash: &str) -> Result<Option<TxAddrData>> {
        let in_key = format!("{}/in", hash);
        let out_key = format!("{}/out", hash);
        let type_key = format!("{}/t", hash);

        let mut res = self.db.multi_get_cf([
            (&self.cf_tx, type_key),
            (&self.cf_tx, in_key),
            (&self.cf_tx, out_key),
        ]);

        for x in res.iter_mut() {
            if x.is_err() {
                let e = std::mem::replace(x, Ok(None));
                return Err(e.unwrap_err().into());
            }
            if x.as_ref().unwrap().is_none() {
                return Ok(None);
            }
        }

        // We've already handled error, safe to unwrap
        let tx_type_buf = std::mem::replace(&mut res[0], Ok(None))?.unwrap();
        let tx_in_buf = std::mem::replace(&mut res[1], Ok(None))?.unwrap();
        let tx_out_buf = std::mem::replace(&mut res[2], Ok(None))?.unwrap();

        Ok(Some(TxAddrData {
            tx_type: String::from_utf8(tx_type_buf)?,
            tx_in: serde_json::from_str(std::str::from_utf8(&tx_in_buf)?)?,
            tx_out: serde_json::from_str(std::str::from_utf8(&tx_out_buf)?)?,
        }))
    }
}

// TODO: SqliteBlockStore to convert from rockdb. Not yet complete.

pub struct SqliteBlockStore {
    conn: Connection,
}

impl SqliteBlockStore {
    pub fn new(path: Option<&str>) -> Result<Self> {
        let conn = sqlite_init_db(path)?;
        Ok(Self { conn })
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
            .prepare_cached("SELECT height FROM txs WHERE txid = ?1")?;
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
}
