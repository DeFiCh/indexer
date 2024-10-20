use crate::db::{encode_height, rocks_open_db, BlockStore};
use crate::dfiutils::{fold_addr_val_map, get_txin_addr_val_list, get_txout_addr_val_list};
use crate::lang::{Error, Result};
use rust_rocksdb::WriteBatch;
use tracing::{error, info};

pub fn index_tx_data() -> Result<()> {
    let db = rocks_open_db(None)?;
    let block_store = BlockStore::new(&db)?;
    let cf_tx = db.cf_handle("tx").ok_or(Error::from("cf handle"))?;
    let start_block_num = 4_100_000;

    let start_key = "b/h/".to_owned() + &encode_height(start_block_num);
    let iter = db.iterator(rust_rocksdb::IteratorMode::From(
        start_key.as_bytes(),
        rust_rocksdb::Direction::Forward,
    ));

    let mut write_batch = Some(WriteBatch::default());
    for (i, item) in iter.enumerate() {
        let (k, v) = item?;
        let key = std::str::from_utf8(&k)?;
        if !key.starts_with("b/h/") {
            info!("key prefix exceeded: {}", &key);
            break;
        }
        let h = std::str::from_utf8(&v)?;
        let b = block_store.get_block_from_hash(h)?;
        let block = b.ok_or_else(|| Error::from("block not found"))?;
        let mut batch_tx = write_batch.take().unwrap();

        for tx in block.tx {
            let tx_vm = &tx.vm;
            let mut is_evm = false;
            let mut is_dvm = false;

            if let Some(ref vm) = tx.vm {
                match vm.vmtype.as_str() {
                    "evm" => is_evm = true,
                    "dvm" => is_dvm = true,
                    _ => {}
                }
            }
            if is_evm {
                let tx_vm = tx_vm.as_ref().unwrap();
                let tx_type = &tx_vm.txtype;
                batch_tx.put_cf(&cf_tx, format!("{}/t", tx.txid), tx_type);
            } else {
                // info!(height = i,txid = &tx.txid);
                let tx_ins = get_txin_addr_val_list(&tx.vin, &block_store).inspect_err(|_| {
                    error!("tx_in err: {} // {}", &tx.txid, &block.hash);
                })?;
                let tx_ins = fold_addr_val_map(&tx_ins);
                batch_tx.put_cf(
                    &cf_tx,
                    format!("{}/in", tx.txid),
                    &serde_json::to_string(&tx_ins)?,
                );
                // info!("in: {:?}", tx_ins);

                let tx_outs = get_txout_addr_val_list(&tx, &tx.vout);
                let tx_outs = fold_addr_val_map(&tx_outs);
                batch_tx.put_cf(
                    &cf_tx,
                    format!("{}/out", tx.txid),
                    &serde_json::to_string(&tx_outs)?,
                );
                // info!("out: {:?}", tx_outs);
                if is_dvm {
                    let tx_vm = tx_vm.as_ref().unwrap();
                    let tx_type = &tx_vm.txtype;
                    batch_tx.put_cf(&cf_tx, format!("{}/t", tx.txid), tx_type);
                } else {
                    batch_tx.put_cf(&cf_tx, format!("{}/t", tx.txid), "utxo");
                }
            }
        }

        if i % 10000 == 0 {
            info!(i);
            db.write(batch_tx)?;
            write_batch = Some(WriteBatch::default());
        } else {
            write_batch = Some(batch_tx);
        }
    }
    db.write(write_batch.take().unwrap())?;
    Ok(())
}
