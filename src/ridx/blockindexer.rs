use rust_rocksdb::WriteBatch;
use tracing::info;

use crate::db::{encode_height, rocks_compact_db, rocks_open_db, BlockStore};
use crate::dfiutils::CliDriver;
use crate::lang::{Error, Result};

pub fn index_from_cli() -> Result<()> {
    let mut cli = CliDriver::new();
    let db = rocks_open_db(None)?;
    let block_store = BlockStore::new(&db)?;

    let mut i = 4_100_000;
    let height = cli.get_block_count()?;

    let mut tx_batch = Some(WriteBatch::default());

    loop {
        if i > height {
            break;
        }

        let mut tx = tx_batch.take().ok_or_else(|| Error::from("no tx batch"))?;

        let block = block_store.get_block_from_height(i)?;
        let mut do_index = false;

        if let Some(b) = block {
            let hash = cli.get_block_hash(i)?;
            if hash != b.hash {
                do_index = true;
            } else {
                let block_details = block_store.get_block_from_hash(&hash)?;
                if let Some(block) = block_details {
                    if block.hash != hash {
                        do_index = true;
                    }
                } else {
                    do_index = true;
                }
            }
        } else {
            do_index = true;
        }

        if do_index {
            let hash = cli.get_block_hash(i)?;
            let block_details = cli.get_block(&hash, Some(4))?;
            let k1 = "b/h/".to_owned() + &encode_height(i);
            let k2 = "b/x/".to_owned() + &hash;

            println!("{} = {}, {}", k1, &hash, k2);
            tx.put(k1, &hash);
            tx.put(k2, serde_json::to_string(&block_details)?);

            let txs = block_details["tx"]
                .as_array()
                .ok_or_else(|| Error::from("json field: tx"))?;

            for x in txs.iter() {
                let txid = x["txid"]
                    .as_str()
                    .ok_or_else(|| Error::from("json field: txid"))?;
                tx.put("t/h/".to_owned() + txid, &hash);
            }
        }

        if i % 10000 == 0 {
            info!(i);
            tx.put("x/height", i.to_le_bytes());
            db.write(tx)?;
            tx_batch = Some(WriteBatch::default());
        } else {
            tx_batch = Some(tx);
        }
        i += 1;
    }

    if let Some(tx) = tx_batch {
        db.write(tx)?;
    }

    info!("index complete. flushing db");
    db.flush()?;

    println!("completed: {}", i - 1);
    rocks_compact_db(&db)?;

    Ok(())
}

pub fn check_db_index() -> Result<()> {
    let db = rocks_open_db(None)?;
    let block_store = BlockStore::new(&db)?;

    for i in 0..4_100_000 {
        let b = block_store.get_block_from_height(i)?;
        if let Some(block) = b {
            for x in block.tx {
                let tx = block_store.get_tx_from_hash(&x.txid)?;
                if tx.is_none() {
                    println!("tx not found: {} // {}", x.txid, i);
                    let bx = block_store.get_block_for_tx(&x.txid)?;
                    if bx.is_none() {
                        println!("not found block either");
                    } else {
                        println!("block found");
                    }
                }

                let _tx_data = block_store.get_tx_addr_data_from_hash(&x.txid);
            }
        } else {
            println!("block {} not found", i);
        }

        if i % 10000 == 0 {
            info!(i)
        }
    }

    Ok(())
}
