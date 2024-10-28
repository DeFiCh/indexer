#![allow(dead_code)]
use crate::db::{BlockStore, TxAddrData};
use crate::lang::Result;
use crate::models::{Block, Transaction};
use rust_rocksdb::{ColumnFamily, ColumnFamilyDescriptor, CompactOptions, Options, DB};
use tracing::info;

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

pub struct RocksBlockStore<'a> {
    db: &'a DB,
    cf_tx: &'a ColumnFamily,
}

impl<'a> BlockStore for RocksBlockStore<'a> {
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

impl<'a> RocksBlockStore<'a> {
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
