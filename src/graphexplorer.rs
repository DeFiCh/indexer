use crate::lang::{OptionExt, Result, ResultExt};
use crate::{db::SqliteBlockStore, models::TxType};
use anyhow::Context;
use clap::Parser;
use petgraph::visit::{EdgeRef, IntoNeighborsDirected};
use tracing::{debug, error, info, trace};

#[derive(Parser, Debug)]
pub struct GraphExpArgs {
    #[arg(long, default_value = "data/index.sqlite")]
    pub sqlite_path: String,
    #[arg(long, default_value = "data/graph.bin")]
    pub graph_data_path: String,
    #[arg(long, default_value = "data/graph.meta.bin")]
    pub graph_meta_path: String,
    #[arg(short = 's', long, default_value_t = 0)]
    pub start_height: i64,
    #[arg(short = 'e', long, default_value_t = 2_000_000)]
    pub end_height: i64,
}

pub fn run(args: &GraphExpArgs) -> Result<()> {
    debug!("args: {:?}", args);

    let quit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let user_sig = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, std::sync::Arc::clone(&quit))?;
    signal_hook::flag::register(
        signal_hook::consts::SIGUSR1,
        std::sync::Arc::clone(&user_sig),
    )?;

    let sql_store = SqliteBlockStore::new_v2(Some(&args.sqlite_path))?;
    let (g, node_index_map) = load_graph(&args.graph_meta_path, &args.graph_data_path)?;

    info!("build reserve index map..");
    let reversed_index_map: std::collections::HashMap<petgraph::graph::NodeIndex, String> =
        node_index_map
            .iter()
            .map(|(k, v)| (*v, k.clone()))
            .collect();

    // ICX txs
    info!("get all icx txs..");
    let mut icx_txs = std::collections::HashSet::<String>::new();
    let icx_ignore_list = vec![
        "8UpCwMVVLk5BostLmG8wghN6haTcJZksv9", // mapped
        "8Xupw3sD33NKS3n3KXMZUVeQdNbPBJ1ttM", // mapped
        "8cDSPjDe7HqvzmSL33xCrcrvBbUcmkTSpg", // bot
    ];
    let r = sql_store.iter_txs_partial(Some("where tx_type = \"icx-claim\""), |tx| {
        if quit.load(std::sync::atomic::Ordering::Relaxed) {
            info!("int: early exit");
            return Err("interrupted".into());
        }
        let tx = tx?;
        if tx.tx_type == TxType::ICXClaimDFCHTLC.to_string() {
            let icx_addr = tx.icx_addr;
            if !icx_addr.is_empty() {
                if !icx_ignore_list.contains(&icx_addr.as_str()) {
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

    let src_addr1 = "";
    // info!("condense graph..");
    // let gx = petgraph::algo::condensation(g, true);
    // info!(
    //     "done. gx: {} nodes and {} edges",
    //     gx.node_count(),
    //     gx.edge_count()
    // );

    // We can run page rank to find the areas of interest, weekly connected components, etc

    let addr1_index = node_index_map.get(src_addr1).context("node_index_map")?;
    let edges = g.edges(*addr1_index);
    info!("iter edges..");

    for x in edges {
        let txid = g.edge_weight(x.id()).context("edge_weight")?;
        let src = g.node_weight(x.source()).context("node_weight")?;
        let dst = g.node_weight(x.target()).context("node_weight")?;
        info!("edge: {:?} -> {:?} ({})", src, dst, txid);
        let tx = sql_store.get_tx_data(txid)?.ok_or_err()?;
    }

    info!("complete..");
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
        bincode::deserialize_from(f).map_err(|e| {
            error!("{:?}", e);
            "bincode err"
        })?;

    info!("loading graph data from {}..", data_path);
    let f = std::fs::File::open(data_path)?;
    let g: petgraph::Graph<String, String> = bincode::deserialize_from(f).map_err(|e| {
        error!("{:?}", e);
        "bincode err"
    })?;

    info!(
        "loaded graph with {} nodes and {} edges",
        g.node_count(),
        g.edge_count()
    );
    Ok((g, node_index_map))
}
