#![feature(error_generic_member_access)]

mod args;
mod db;
mod dfiutils;
mod lang;
mod models;

use args::Args;
use clap::Parser;
use db::{
    encode_height, rocks_open_db, sqlite_begin_tx, sqlite_commit_and_begin_tx, sqlite_commit_tx,
    sqlite_get_stmts, sqlite_init_db, BlockStore,
};
use dfiutils::{extract_dfi_addresses, token_id_to_symbol_maybe};
use lang::OptionExt;
use lang::{Error, Result};
use models::{IcxLogData, IcxTxSet, TxType};
use std::collections::HashMap;
use std::{error::request_ref, io::BufRead};
use tracing::{error, info};

fn run(args: Args) -> Result<()> {
    let rocks_db_path = match args.src_rocks_db_path.is_empty() {
        true => None,
        false => Some(args.src_rocks_db_path.as_str()),
    };
    let sqlite_path = match args.sqlite_path.is_empty() {
        true => None,
        false => Some(args.sqlite_path.as_str()),
    };
    let defid_log_path = match args.defid_log_path.is_empty() {
        true => None,
        false => Some(args.defid_log_path.as_str()),
    };
    let defid_log_matcher = args.defid_log_matcher.as_str();

    let start_height = args.start_height;
    let end_height = args.end_height;

    info!("{:?}", args);

    let quit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, std::sync::Arc::clone(&quit))?;

    let mut icx_data_map = HashMap::<String, IcxLogData>::default();

    if let Some(defid_log_path) = defid_log_path {
        let file = std::fs::File::open(defid_log_path)?;
        let reader = std::io::BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            if line.contains(defid_log_matcher) {
                // parse the line only on the valid json
                if let Some(json_start) = line.find('{') {
                    let json_str = &line[json_start..];
                    if let Ok(icx_data) = serde_json::from_str::<IcxLogData>(json_str) {
                        icx_data_map.insert(icx_data.claim_tx.clone(), icx_data);
                    } else {
                        error!("json parse failure: {}", json_str);
                    }
                }
            }
        }
    }

    let rdb = rocks_open_db(rocks_db_path)?;
    let block_store = BlockStore::new(&rdb)?;

    let sconn = sqlite_init_db(sqlite_path)?;
    let mut stmts = sqlite_get_stmts(&sconn)?;

    let start_key = "b/h/".to_owned() + &encode_height(start_height);
    let end_key = "b/h/".to_owned() + &encode_height(end_height);

    let iter = rdb.iterator(rust_rocksdb::IteratorMode::From(
        start_key.as_bytes(),
        rust_rocksdb::Direction::Forward,
    ));

    sqlite_begin_tx(&sconn)?;

    for (i, item) in iter.enumerate() {
        let (key, value) = item?;
        if key.as_ref() >= end_key.as_bytes() {
            break;
        }

        // Well known keys, skip the utf8 checks for perf as
        // every little thing in this loop is a hot stop, as each run millions of times
        let key = unsafe { std::str::from_boxed_utf8_unchecked(key) };
        let hash = unsafe { std::str::from_boxed_utf8_unchecked(value) };

        // Equivalent to contains check and strip - we just do it in the same pass.
        let h = key.strip_prefix("b/h/");
        if h.is_none() {
            info!("key prefix exceeded: {}", &key);
            break;
        }
        let h = h.unwrap();

        let (_l_str, height_str) = h.split_at(1);
        let height = height_str.parse::<u64>()?;

        let b = block_store.get_block_from_hash(&hash)?;
        let block = b.ok_or_else(|| Error::from("block not found"))?;

        // perf focused loop, but still ensure integrity
        if height != block.height as u64 {
            return Err(Error::Message(format!(
                "height mismatch: {} != {}",
                height, block.height
            )));
        }

        // println!("[{}] key: {}, value: {}", height, key, &hash);

        {
            let block_json = serde_json::to_string(&block)?;
            stmts[0].execute(rusqlite::params![height, &hash, &block_json])?;
        }

        for tx in block.tx {
            let tx_data = block_store
                .get_tx_addr_data_from_hash(&tx.txid)?
                .ok_or_else(|| Error::from(format!("tx data: {}", &tx.txid)))?;

            let mut tx_type = tx.vm.as_ref().map(|x| TxType::from(x.txtype.as_ref()));
            let tx_out = tx_data
                .tx_out
                .iter()
                .filter(|x| x.0 != "x") // strip coinbase out
                .collect::<HashMap<_, _>>();

            let mut dvm_addrs = vec![];

            if tx_data.tx_in.is_empty() {
                tx_type = Some(TxType::Coinbase);
            }

            if !matches!(
                &tx_type,
                Some(TxType::Coinbase) | Some(TxType::Unknown) | Some(TxType::Utxo) | None
            ) {
                let dvm_data = tx.vm.as_ref().map(|x| x.msg.to_string()).unwrap();
                dvm_addrs = extract_dfi_addresses(&dvm_data);
            }
            let mut icx_claim_data: Option<IcxTxSet> = None;
            let mut icx_addr: String = empty();
            let mut icx_amt: String = empty();
            let mut swap_from: String = empty();
            let mut swap_to: String = empty();
            let mut swap_amt: String = empty();

            match tx_type {
                //  Some(TxType::CompositeSwap) not enabled < 2m.
                Some(TxType::PoolSwap) => {
                    let swap_data = &tx.vm.as_ref().ok_or_err()?.msg;
                    let from_token = swap_data["fromToken"].as_str().ok_or_err()?;
                    let to_token = swap_data["toToken"].as_str().ok_or_err()?;
                    let amt = swap_data["fromAmount"].as_f64().ok_or_err()?;
                    swap_from = token_id_to_symbol_maybe(from_token).to_string();
                    swap_to = token_id_to_symbol_maybe(to_token).to_string();
                    swap_amt = format!("{:.9}", amt);
                }
                Some(TxType::ICXSubmitEXTHTLC) => {
                    let icx_data = icx_data_map.get(tx.txid.as_str());
                    if let Some(icx_data) = icx_data {
                        icx_claim_data = Some(IcxTxSet {
                            order_tx: &icx_data.order_tx,
                            claim_tx: &icx_data.claim_tx,
                            offer_tx: &icx_data.offer_tx,
                            dfchtlc_tx: &icx_data.dfchtlc_tx,
                        });
                        icx_addr = icx_data.address.clone();
                        icx_amt = icx_data.amount.clone();
                    }
                }
                _ => {}
            }

            // Transform to final strings. Mostly empty strings for non relevant fields

            let tx_type_str = tx_type.clone().unwrap_or(TxType::Unknown).to_string();
            let dvm_addrs_json = if dvm_addrs.is_empty() {
                empty()
            } else {
                serde_json::to_string(&dvm_addrs)?
            };
            let tx_in_json = if tx_data.tx_in.is_empty() {
                empty()
            } else {
                serde_json::to_string(&tx_data.tx_in)?
            };
            let tx_out_json = if tx_data.tx_out.is_empty() {
                empty()
            } else {
                serde_json::to_string(&tx_out)?
            };
            let tx_json = serde_json::to_string(&tx)?;
            let icx_claim_data = if icx_claim_data.is_none() {
                empty()
            } else {
                serde_json::to_string(&icx_claim_data.unwrap())?
            };

            stmts[1].execute(rusqlite::params![
                &tx.txid,
                height,
                &tx_type_str,
                &tx_in_json,
                &tx_out_json,
                &dvm_addrs_json,
                &tx_json,
                &icx_claim_data,
                &icx_addr,
                &icx_amt,
                &swap_from,
                &swap_to,
                &swap_amt,
            ])?;
        }

        if i % 10000 == 0 {
            sqlite_commit_and_begin_tx(&sconn)?;
            info!("processed: [{}] / [{}] // {}", height, end_height, i);
        }
        if quit.load(std::sync::atomic::Ordering::Relaxed) {
            info!("quit signal received");
            break;
        }
    }

    info!("flushing db");
    sqlite_commit_tx(&sconn)?;

    info!("done");
    Ok(())
}

// Just a short convenience alias for internal use.
fn empty() -> String {
    static EMPTY_STR: String = String::new();
    EMPTY_STR.clone()
}

fn main_fallible() -> Result<()> {
    std::env::set_var("RUST_BACKTRACE", "1");
    tracing_subscriber::fmt::fmt().compact().init();

    let args = Args::parse();
    run(args)?;

    Ok(())
}

fn main() {
    let res = main_fallible();
    if let Err(e) = res {
        error!("{e}");
        let bt = request_ref::<std::backtrace::Backtrace>(&e);
        if let Some(bt) = bt {
            error!("{bt}");
        }
    }
}
