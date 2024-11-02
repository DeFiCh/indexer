use std::str::FromStr;

use crate::{
    db::SqliteBlockStore,
    graphutils,
    lang::{OptionExt, Result},
    models::TxType,
};
use anyhow::Context;
use clap::Parser;
use petgraph::visit::EdgeRef;
use tracing::{debug, error, info, trace};

#[derive(Parser, Debug)]
pub struct IcxSequenceArgs {
    #[arg(long, default_value = "data/index.sqlite")]
    pub sqlite_path: String,
    #[arg(long, default_value = "data/graph.bin")]
    pub graph_data_path: String,
    #[arg(long, default_value = "data/graph.meta.bin")]
    pub graph_meta_path: String,
    /// Address that's the origin (center point) of the graph exploration
    #[arg(long, short = 'a')]
    pub addr: String,
    /// ICX addresses to ignore for co-relation
    #[arg(
        long,
        use_value_delimiter = true,
        value_delimiter = ',',
        default_value = ""
    )]
    pub icx_ignore_addr: Vec<String>,
    /// Graph addresses to ignore for co-relation
    #[arg(
        long,
        use_value_delimiter = true,
        value_delimiter = ',',
        default_value = ""
    )]
    pub graph_ignore_addr: Vec<String>,
    /// Graph addresses to mark for co-relation
    #[arg(
        long,
        use_value_delimiter = true,
        value_delimiter = ',',
        default_value = ""
    )]
    pub graph_mark_addr: Vec<String>,
}

pub fn run(args: &IcxSequenceArgs) -> Result<()> {
    debug!("args: {:?}", args);

    let quit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, std::sync::Arc::clone(&quit))?;
    let user_sig = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    #[cfg(unix)]
    signal_hook::flag::register(
        signal_hook::consts::SIGUSR1,
        std::sync::Arc::clone(&user_sig),
    )?;

    let sql_store = SqliteBlockStore::new_v2(Some(&args.sqlite_path))?;
    let (g, node_index_map) = graphutils::load_graph(&args.graph_meta_path, &args.graph_data_path)?;

    let mut graph_ignore_addr_list = args.graph_ignore_addr.clone();
    graph_ignore_addr_list.sort();

    let mut graph_mark_addr_list = args.graph_mark_addr.clone();
    graph_mark_addr_list.sort();

    todo!("impl pending");

    // ICX txs
    info!("get all icx txs..");
    let mut icx_txs = std::collections::HashSet::<String>::new();
    let icx_ignore_list = args
        .icx_ignore_addr
        .iter()
        .collect::<std::collections::HashSet<_>>();
    let r = sql_store.iter_txs_partial(Some("where tx_type glob \"icx-claim\""), |tx| {
        if quit.load(std::sync::atomic::Ordering::Relaxed) {
            info!("int: early exit");
            return Err("interrupted".into());
        }
        let tx = tx?;
        if tx.tx_type == TxType::ICXClaimDFCHTLC.to_string() {
            let icx_addr = tx.icx_addr;
            if !icx_addr.is_empty() && !icx_ignore_list.contains(&icx_addr) {
                icx_txs.insert(tx.txid.clone());
            }
        }
        Ok(())
    });
    if let Err(e) = r {
        if e.to_string() == "interrupted" {
            info!("{:?}", e);
        } else {
            error!("{:?}", e);
        }
    } else {
        info!("done. icx_txs: {}", icx_txs.len());
        trace!("icx_txs: {:?}", icx_txs);
    }
    info!("complete");
    Ok(())
}
