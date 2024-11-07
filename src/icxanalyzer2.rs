use crate::db::{SqliteBlockStore, TxRow};
use crate::lang::Result;
use crate::models::TxType;
use clap::Parser;
use std::collections::HashSet;
use tracing::{debug, error, info};

#[derive(Parser, Debug)]
pub struct IcxAnalyze2Args {
    #[arg(long, default_value = "data/index.sqlite")]
    pub sqlite_path: String,
    #[arg(short = 's', long, default_value_t = 0)]
    pub start_height: i64,
    #[arg(short = 'e', long, default_value_t = 2_000_000)]
    pub end_height: i64,
    #[arg(long, default_value_t = 1)]
    pub icx_addr: i64,
}

pub fn run(args: &IcxAnalyze2Args) -> Result<()> {
    debug!("args: {:?}", args);

    let quit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, std::sync::Arc::clone(&quit))?;

    let sql_store = SqliteBlockStore::new_v2(Some(&args.sqlite_path))?;
    let tracked_tx_types: HashSet<_> = [
        TxType::Unknown,
        // TxType::Coinbase,
        TxType::Utxo,
        TxType::CreateMasternode,
        TxType::ResignMasternode,
        TxType::PoolSwap,
        // TxType::CompositeSwap,
        TxType::AddPoolLiquidity,
        TxType::RemovePoolLiquidity,
        TxType::UtxosToAccount,
        TxType::AccountToUtxos,
        TxType::AccountToAccount,
        TxType::WithdrawFromVault,
        // TxType::SetOracleData,
        TxType::DepositToVault,
        TxType::PaybackLoan,
        TxType::TakeLoan,
        TxType::AutoAuth,
        TxType::Vault,
        TxType::AnyAccountsToAccounts,
        TxType::ICXCreateOrder,
        TxType::ICXMakeOffer,
        TxType::ICXSubmitDFCHTLC,
        TxType::ICXSubmitEXTHTLC,
        TxType::ICXClaimDFCHTLC,
        TxType::ICXCloseOrder,
        TxType::ICXCloseOffer,
        // TxType::Other(String::new()),
    ]
    .iter()
    .map(|x| x.to_string())
    .collect();

    let stop_tracking_predicate = |tx: &TxRow, tracked_info: &TrackedInfo| -> bool {
        if tx.tx_type == TxType::PoolSwap.to_string()
            && tx.swap_from == "btc"
            && tracked_info.current_swapped >= (tracked_info.btc_minted - 0.00000001)
        {
            debug!("{:?}", tracked_info);
            return true;
        }
        false
    };

    let update_tracking_info = |tx: &TxRow, tracked_info: &mut TrackedInfo| -> Result<()> {
        if tx.tx_type == TxType::PoolSwap.to_string() && tx.swap_from == "btc" {
            tracked_info.current_swapped += str::parse::<f64>(&tx.swap_amt)?;
        }
        Ok(())
    };

    #[derive(Debug)]
    #[allow(dead_code)]
    struct TrackedInfo {
        origin_txid: String,
        addr: String,
        btc_minted: f64,
        // state
        current_swapped: f64,
    }

    let mut count = 0;
    let mut this_addr_icx_claims = 0;
    let mut txiter = 0;
    let mut tracked = HashSet::new();
    let mut tracked_info: Option<TrackedInfo> = Option::None;

    let r = sql_store.iter_txs(None, |tx| {
        if quit.load(std::sync::atomic::Ordering::Relaxed) {
            info!("int: early exit");
            return Err("interrupted".into());
        }

        txiter += 1;
        if txiter % 100000 == 0 {
            info!(
                "txiter: {} (tracking: {}, tracked addrs: {})",
                txiter,
                tracked_info.is_some(),
                tracked.len()
            );
        }
        let tx = tx?;
        let mut is_tracked_tx = false;

        // println!("{:?}", tx);
        if !tx.icx_addr.is_empty() {
            count += 1;
            if let Some(tr) = tracked_info.as_mut() {
                if tr.addr == tx.icx_addr {
                    debug!("icx tx: {} // {}", &tx.txid, &tx.icx_addr);
                    tr.btc_minted += str::parse::<f64>(&tx.icx_btc_exp_amt)?;
                    this_addr_icx_claims += 1;
                }
            }
            if count == args.icx_addr {
                debug!("icx tx: {} // {}", &tx.txid, &tx.icx_addr);
                is_tracked_tx = true;
                tracked_info = Some(TrackedInfo {
                    origin_txid: tx.txid.clone(),
                    addr: tx.icx_addr.clone(),
                    btc_minted: str::parse(&tx.icx_btc_exp_amt)?,
                    current_swapped: 0.,
                });
                this_addr_icx_claims += 1;
            }
        }

        if !tracked_tx_types.contains(&tx.tx_type) {
            return Ok(());
        }

        if !is_tracked_tx {
            for x in tx
                .tx_in
                .iter()
                .map(|x| x.0)
                .chain(tx.tx_out.iter().map(|x| x.0).chain(tx.dvm_out.iter()))
            {
                if tracked.contains(x) {
                    is_tracked_tx = true;
                    break;
                }
            }
        }

        if !is_tracked_tx {
            return Ok(());
        }

        match tx.tx_type.as_str() {
            "ps" => {
                println!(
                    "{}: {} ({} -> {}: {})",
                    &tx.tx_type, tx.txid, tx.swap_from, tx.swap_to, tx.swap_amt
                );
            }
            "icx-claim" => {
                println!(
                    "{}: {} ({} / {})",
                    &tx.tx_type, tx.txid, tx.icx_btc_exp_amt, tx.icx_addr
                );
            }
            _ => {
                println!("{}: {}", &tx.tx_type, tx.txid);
            }
        };

        if let Some(t) = tracked_info.as_mut() {
            update_tracking_info(&tx, t)?;
            if stop_tracking_predicate(&tx, t) {
                return Err("stop track crieteria hit".into());
            }
        }

        tracked.extend(tx.tx_out.iter().map(|x| x.0.clone()));
        tracked.extend(tx.tx_in.iter().map(|x| x.0.clone()));
        tracked.extend(tx.dvm_out.iter().cloned());

        Ok(())
    });

    if let Err(e) = r {
        if e.to_string() == "stop track crieteria hit" {
            info!("{:?}", e);
        } else {
            error!("{:?}", e);
        }
    }

    debug!("tracked addresses: {:?}", tracked);
    debug!("summary: no. tracked addresses: {:?}", tracked.len());
    if let Some(tracked_info) = tracked_info {
        info!("summary: {:?}", tracked_info);
    }
    info!(
        "summary: total icx-claims from addr: {}",
        this_addr_icx_claims
    );
    info!("summary: scanned icx-claims: {}", count);
    Ok(())
}
