use crate::db;
use crate::dfiutils;
use crate::lang;
use crate::logparse::process_log_file;
use crate::models;
use crate::models::LogEntryMap;
use clap::Parser;
use db::{
    sqlite_begin_tx, sqlite_commit_and_begin_tx, sqlite_commit_tx, sqlite_create_index_factory_v2,
    sqlite_get_stmts_v2, SqliteBlockStore,
};
use dfiutils::{extract_all_dfi_addresses, token_id_to_symbol_maybe, CliDriver};
use lang::OptionExt;
use lang::Result;
use models::{Block, IcxTxSet, TxType};
use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;
use tracing::debug;
use tracing::info;

#[derive(Parser, Debug)]
pub struct CliIndexArgs {
    #[arg(long, default_value = "defi-cli")]
    pub defi_cli_path: String,
    #[arg(long, default_value = "data/index.sqlite")]
    pub sqlite_path: String,
    // The path to the debug.log file from defid.
    // This can be both gzipped or raw file. If the file is gzipped
    // it will automatically be decompressed on the fly.
    #[arg(long, default_value = "data/debug.log.gz")]
    pub defid_log_path: String,
    #[arg(long, default_value = "ICX:")]
    pub log_icx_matcher: String,
    #[arg(long, default_value = "ICXCalc:")]
    pub log_icx_calc_matcher: String,
    #[arg(long, default_value = "SwapResult:")]
    pub log_swap_matcher: String,
    #[arg(short = 's', long, default_value_t = 0)]
    pub start_height: i64,
    #[arg(short = 'e', long, default_value_t = 2_000_000)]
    pub end_height: i64,
    #[arg(long, default_value_t = true)]
    pub enable_graph_table: bool,
}

pub fn run(args: &CliIndexArgs) -> Result<()> {
    let db_path = match args.sqlite_path.is_empty() {
        true => None,
        false => Some(args.sqlite_path.as_str()),
    };
    let defid_log_path = match args.defid_log_path.is_empty() {
        true => None,
        false => Some(args.defid_log_path.as_str()),
    };
    let enable_addr_graph = args.enable_graph_table;
    let start_height = args.start_height;
    let end_height = args.end_height;

    info!("{:?}", args);

    let quit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, std::sync::Arc::clone(&quit))?;

    let mut log_entry_map = LogEntryMap::new();

    if let Some(defid_log_path) = defid_log_path {
        info!("ingesting log file: {}", defid_log_path);

        process_log_file(
            defid_log_path,
            args.log_icx_matcher.as_str(),
            args.log_icx_calc_matcher.as_str(),
            args.log_swap_matcher.as_str(),
            &mut log_entry_map,
        )?;

        info!(
            "log file ingested:\n\
            \tTotal transactions:     {}\n\
            \tTotal ICX entries:      {}\n\
            \tTotal ICX calc entries: {}\n\
            \tTotal Swap entries:     {}",
            log_entry_map.data.len(),
            log_entry_map.icx_count,
            log_entry_map.icx_calc_count,
            log_entry_map.swap_count,
        );
    }

    let mut cli = CliDriver::with_cli_path(args.defi_cli_path.clone());
    let sql_store = SqliteBlockStore::new_v2(db_path)?;

    let chain_height = cli.get_block_count()?;
    let iter_end_height = if chain_height < end_height {
        chain_height
    } else {
        end_height
    };

    let sconn = &sql_store.conn;
    for (name, _) in sqlite_create_index_factory_v2(sconn) {
        if quit.load(std::sync::atomic::Ordering::Relaxed) {
            info!("int: early exit indexes");
            break;
        }
        info!("drop index: {}..", name);
        let q = format!("DROP INDEX IF EXISTS {}", name);
        sconn.execute(&q, [])?;
    }

    let mut stmts = sqlite_get_stmts_v2(sconn)?;
    sqlite_begin_tx(sconn)?;

    let mut err = Option::None;
    for height in start_height..=iter_end_height {
        if quit.load(std::sync::atomic::Ordering::Relaxed) {
            info!("int: early exit");
            break;
        }

        // May be abstract this out to a fn so error control is better. For now, handle cli errors
        // Reason: Ctrl + C will send SIGHUP to the child process and that'll exit with err
        // returning upward instead of breaking on the loop and flushing. This is a workaround.
        let hash = match cli.get_block_hash(height) {
            Ok(hash) => hash,
            Err(e) => {
                err = Some(e);
                break;
            }
        };
        let block_out = match cli.get_block(&hash, Some(4)) {
            Ok(block) => block,
            Err(e) => {
                err = Some(e);
                break;
            }
        };
        let block_json_str = block_out.str()?;
        let block: Block = block_out.json()?;

        debug!("[{}] hash: {}", height, &hash);
        {
            stmts[0].execute(rusqlite::params![height, &hash, block_json_str])?;
        }

        for tx in block.tx {
            let tx_in_addrs = dfiutils::get_txin_addr_val_list(&tx.vin, &sql_store)?;
            let tx_out_addrs = dfiutils::get_txout_addr_val_list(&tx, &tx.vout);

            let tx_in_addrs = dfiutils::fold_addr_val_map(&tx_in_addrs);
            let tx_out = dfiutils::fold_addr_val_map(&tx_out_addrs)
                .into_iter()
                .filter(|x| *x.0 != *"x") // strip coinbase out
                .collect::<HashMap<_, _>>();

            let mut tx_type = tx.vm.as_ref().map(|x| TxType::from(&*x.txtype));
            let mut dvm_addrs = HashSet::new();

            if tx_in_addrs.is_empty() {
                tx_type = Some(TxType::Coinbase);
            }

            if !matches!(
                &tx_type,
                Some(TxType::Coinbase) | Some(TxType::Unknown) | Some(TxType::Utxo) | None
            ) {
                let dvm_data = tx.vm.as_ref().map(|x| x.msg.to_string()).unwrap();
                dvm_addrs = extract_all_dfi_addresses(&dvm_data);
            }
            let mut icx_claim_data: Option<IcxTxSet> = None;
            let mut icx_addr = empty();
            let mut icx_amt = empty();
            let mut swap_from = empty();
            let mut swap_to = empty();
            let mut swap_amt = empty();

            match tx_type {
                Some(TxType::PoolSwap) | Some(TxType::CompositeSwap) => {
                    let swap_data = &tx.vm.as_ref().ok_or_err()?.msg;
                    let swap_data: models::PoolSwapMsg = serde_json::from_value(swap_data.clone())?;
                    swap_from = token_id_to_symbol_maybe(&swap_data.from_token).to_string();
                    swap_to = token_id_to_symbol_maybe(&swap_data.to_token).to_string();
                    swap_amt = format!("{:.9}", &swap_data.from_amount);
                }
                Some(TxType::ICXClaimDFCHTLC) => {
                    if let Some(log_entry) = &log_entry_map.data.get(&tx.txid) {
                        if let Some(icx_data) = &log_entry.icx_data {
                            icx_claim_data = Some(IcxTxSet {
                                order_tx: icx_data.order_tx.clone(),
                                claim_tx: icx_data.claim_tx.clone(),
                                offer_tx: icx_data.offer_tx.clone(),
                                dfchtlc_tx: icx_data.dfchtlc_tx.clone(),
                            });
                            icx_addr = icx_data.address.to_string();
                            icx_amt = icx_data.amount.to_string();
                        }
                    }
                }
                _ => {}
            }

            let (dvm_in_addrs, _): (Vec<_>, Vec<_>) = dvm_addrs
                .iter()
                .cloned()
                .partition(|addr| tx_in_addrs.iter().any(|(in_addr, _)| *in_addr == *addr));

            if enable_addr_graph {
                // DVM addresses are parsed for all matching addresses inside the
                // DVM data. There is no clean in and out: this requires specific
                // knowledge of each message and there's no clear convention of this.
                // So instead, we workaround this as we know that if tx in and dvm addr
                // is the same, they were _likely_ source.
                // We partition these out first. For out, we take the whole list
                // to err on the side of caution to add more edges.

                let mut changeset = HashMap::<[Rc<str>; 2], i64>::new();

                for (out_addr, _) in tx_out.iter() {
                    for (in_addr, _) in tx_in_addrs.iter() {
                        let k = [in_addr.clone(), (*out_addr).clone()];
                        changeset.insert(k, 0);
                    }
                }

                for out_addr in dvm_addrs.iter() {
                    for in_addr in dvm_in_addrs.iter() {
                        let k = [in_addr.clone(), out_addr.clone()];
                        let v = changeset.get_mut(&k);
                        if let Some(v) = v {
                            // we set to DVM + UTXO
                            if *v == 0 {
                                *v = 2;
                            }
                        } else {
                            // we set this with DVM only
                            changeset.insert(k, 1);
                        }
                    }
                }

                for ([edge_in, edge_out], c_flags) in &changeset {
                    stmts[2].execute(rusqlite::params![&tx.txid, &edge_in, &edge_out, c_flags])?;
                }
            }

            // Transform to final strings. Mostly empty strings for non relevant fields

            let tx_type_str = tx_type.clone().unwrap_or(TxType::Unknown).to_string();
            let dvm_in_addrs_json = if dvm_in_addrs.is_empty() {
                empty()
            } else {
                serde_json::to_string(&dvm_in_addrs)?
            };
            let dvm_addrs_json = if dvm_addrs.is_empty() {
                empty()
            } else {
                serde_json::to_string(&dvm_addrs)?
            };
            let tx_in_json = if tx_in_addrs.is_empty() {
                empty()
            } else {
                serde_json::to_string(&tx_in_addrs)?
            };
            let tx_out_json = if tx_out.is_empty() {
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
                &dvm_in_addrs_json,
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

        if height % 10000 == 0 {
            sqlite_commit_and_begin_tx(sconn)?;
            info!("processed: [{}] / [{}]", height, end_height);
        }
    }

    info!("flushing db");
    sqlite_commit_tx(sconn)?;

    for (name, indexer) in sqlite_create_index_factory_v2(sconn) {
        if quit.load(std::sync::atomic::Ordering::Relaxed) {
            info!("int: early exit indexes");
            break;
        }
        info!("creating index: {}..", name);
        indexer()?;
    }

    if let Some(e) = err {
        return Err(e);
    }

    info!("done");
    Ok(())
}

// Just a short convenience alias for internal use.
fn empty() -> String {
    String::new()
}
