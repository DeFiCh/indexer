use std::str::FromStr;

use crate::{
    db::SqliteBlockStore,
    lang::{OptionExt, Result},
    models::TxType,
};
use anyhow::Context;
use clap::Parser;
use petgraph::visit::EdgeRef;
use tracing::{debug, error, info, trace};

#[derive(Parser, Debug)]
pub struct GraphWalkArgs {
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

pub fn run(args: &GraphWalkArgs) -> Result<()> {
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
    let (g, node_index_map) = load_graph(&args.graph_meta_path, &args.graph_data_path)?;

    let mut graph_ignore_addr_list = args.graph_ignore_addr.clone();
    graph_ignore_addr_list.sort();

    let mut graph_mark_addr_list = args.graph_mark_addr.clone();
    graph_mark_addr_list.sort();

    // ICX txs
    info!("get all icx txs..");
    let mut icx_txs = std::collections::HashSet::<String>::new();
    let icx_ignore_list = args
        .icx_ignore_addr
        .iter()
        .collect::<std::collections::HashSet<_>>();
    let r = sql_store.iter_txs_partial(Some("where tx_type = \"icx-claim\""), |tx| {
        if quit.load(std::sync::atomic::Ordering::Relaxed) {
            info!("int: early exit");
            return Err("interrupted".into());
        }
        let tx = tx?;
        if tx.tx_type == TxType::ICXClaimDFCHTLC.to_string() {
            let icx_addr = tx.icx_addr;
            if !icx_addr.is_empty() {
                if !icx_ignore_list.contains(&icx_addr) {
                    icx_txs.insert(tx.txid.clone());
                }
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

    // info!("condense graph..");
    // let gx = petgraph::algo::condensation(g, true);
    // info!(
    //     "done. gx: {} nodes and {} edges",
    //     gx.node_count(),
    //     gx.edge_count()
    // );

    // Can now run exploratory algorithms like page rank to find the areas of interest, weekly connected components, etc
    // Already have many of these mapped out through both runs of several algo as well as inferences generated
    // from the icxanalyzer. We can just plug some of these addresses in to find the paths.

    let src_addr1 = &args.addr;

    let addr1_index = node_index_map.get(src_addr1).context("node_index_map")?;
    info!("iter edges..");

    let mut total_icx = bigdecimal::BigDecimal::from(0);
    let mut total_btc_swaps = bigdecimal::BigDecimal::from(0);

    let max_levels = 20;
    let mut visited = std::collections::HashSet::new();
    let mut current_level = vec![*addr1_index];

    for level in 0..max_levels {
        info!("running level: {}", level);
        let mut next_level = Vec::new();

        if quit.load(std::sync::atomic::Ordering::Relaxed) {
            info!("int: early exit");
            return Err("interrupted".into());
        }

        for &current_node in current_level.iter() {
            if quit.load(std::sync::atomic::Ordering::Relaxed) {
                info!("int: early exit");
                return Err("interrupted".into());
            }

            if visited.contains(&current_node) {
                continue;
            }
            visited.insert(current_node);

            let mut edges = g.edges(current_node);
            while let Some(x) = edges.next() {
                let txid = g.edge_weight(x.id()).context("edge_weight")?;
                let src = g.node_weight(x.source()).context("node_weight")?;
                let dst = g.node_weight(x.target()).context("node_weight")?;

                // info!("edge: {:?} -> {:?} ({})", src, dst, txid);
                let tx = sql_store.get_tx_data(txid)?.ok_or_err()?;
                let tx_type = TxType::from_display(tx.tx_type.as_str());

                if graph_mark_addr_list.binary_search(&dst).is_ok() {
                    info!(
                        "MARK: found: lvl:{}, height: {}, tx: {}, src: {}, dst: {}, txtype: {}",
                        level, tx.height, txid, src, dst, tx.tx_type
                    );
                }

                if graph_ignore_addr_list.binary_search(&dst).is_ok() {
                    continue;
                }

                match tx_type {
                    TxType::PoolSwap => {
                        if tx.swap_from == "btc" {
                            let v = bigdecimal::BigDecimal::from_str(&tx.swap_amt).unwrap();
                            total_btc_swaps += v;
                            info!(
                                "btc-swap: lvl: {}, height: {}, from: {}, to: {} / {}, amt: {} // btc_sum: {}",
                                level, tx.height, src, dst, tx.swap_to, tx.swap_amt, total_btc_swaps
                            );
                        }
                    }
                    TxType::ICXClaimDFCHTLC => {
                        let v = bigdecimal::BigDecimal::from_str(&tx.icx_btc_exp_amt);
                        match v {
                            Ok(v) => {
                                total_icx += v;
                            }
                            Err(e) => {
                                error!(
                                    "icx_btc_exp_amt: {:?} // {}, {}",
                                    e, tx.txid, tx.icx_btc_exp_amt
                                );
                            }
                        }
                        info!(
                            "icx: lvl: {}, height: {}, from: {}, to: {}, tx: {}, icx_to: {}, amt: {} // icx_sum: {}",
                            level, tx.height, src, dst, tx.txid, tx.icx_addr, tx.icx_btc_exp_amt, total_icx
                        );
                    }
                    _ => {}
                }
                next_level.push(x.target());
            }
        }

        if next_level.is_empty() {
            break;
        }
        current_level = next_level;
    }
    info!("complete");
    Ok(())
}

pub fn load_graph(
    meta_path: &str,
    data_path: &str,
) -> crate::lang::Result<(
    petgraph::Graph<String, String>,
    std::collections::HashMap<String, petgraph::graph::NodeIndex>,
)> {
    info!("loading graph metadata from {}..", meta_path);
    let f = std::fs::File::open(meta_path)?;
    let node_index_map: std::collections::HashMap<String, petgraph::graph::NodeIndex> =
        bincode::deserialize_from(f).context("meta bincode err")?;

    info!("loading graph data from {}..", data_path);
    let f = std::fs::File::open(data_path)?;
    let g: petgraph::Graph<String, String> =
        bincode::deserialize_from(f).context("g bincode err")?;

    info!(
        "loaded graph with {} nodes and {} edges",
        g.node_count(),
        g.edge_count()
    );
    Ok((g, node_index_map))
}
