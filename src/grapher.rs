use crate::db::SqliteBlockStore;
use crate::lang::Result;
use anyhow::Context;
use clap::Parser;
use std::collections::HashSet;
use tracing::{debug, error, info};

#[derive(Parser, Debug)]
pub struct GrapherArgs {
    #[arg(long, default_value = "data/index.sqlite")]
    pub sqlite_path: String,
    #[arg(long, default_value = "data/graph.json")]
    pub graph_data_path: String,
    #[arg(long, default_value = "data/graph.meta.json")]
    pub graph_meta_path: String,
    #[arg(short = 's', long, default_value_t = 0)]
    pub start_height: i64,
    #[arg(short = 'e', long, default_value_t = 2_000_000)]
    pub end_height: i64,
}

pub fn run(args: &GrapherArgs) -> Result<()> {
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
    let mut txiter = 0;

    let mut g = petgraph::Graph::new();
    let mut node_index_map = std::collections::HashMap::<String, _>::new();

    let r = sql_store.iter_txs(None, |tx| {
        if quit.load(std::sync::atomic::Ordering::Relaxed) {
            info!("int: early exit");
            return Err("interrupted".into());
        }

        txiter += 1;
        let tx = tx?;

        let tx_ins = tx
            .tx_in
            .iter()
            .map(|x| x.0)
            .cloned()
            .chain(tx.dvm_in.iter().cloned())
            .collect::<HashSet<_>>();

        let tx_outs = tx
            .tx_out
            .iter()
            .map(|x| x.0)
            .cloned()
            .chain(tx.dvm_out.iter().cloned())
            .collect::<HashSet<_>>();

        // Create nodes for any new addresses
        for addr in tx_ins.iter().chain(tx_outs.iter()) {
            if !node_index_map.contains_key(addr) {
                let node_idx = g.add_node(addr.clone());
                node_index_map.insert(addr.clone(), node_idx);
            }
        }

        // Add edges between inputs and outputs
        for to_addr in &tx_outs {
            for from_addr in &tx_ins {
                let from_idx = node_index_map[from_addr];
                let to_idx = node_index_map[to_addr];
                g.add_edge(from_idx, to_idx, tx.txid.clone());
            }
        }

        if txiter % 100000 == 0 {
            info!(
                "txiter: {}, nodes: {}, edges: {}",
                txiter,
                g.node_count(),
                g.edge_count()
            );
        }

        if user_sig.load(std::sync::atomic::Ordering::Relaxed) {
            info!("sig received: dumping memory");
            user_sig.store(false, std::sync::atomic::Ordering::Release);
            dump_graph_data(
                txiter,
                &g,
                &node_index_map,
                &args.graph_meta_path,
                &args.graph_data_path,
            )?;
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
        dump_graph_data(
            txiter,
            &g,
            &node_index_map,
            &args.graph_meta_path,
            &args.graph_data_path,
        )?;
    }

    info!("summary: scanned txs: {}", txiter);
    Ok(())
}

fn dump_graph_data(
    txiter: i32,
    g: &petgraph::Graph<String, String>,
    node_index_map: &std::collections::HashMap<String, petgraph::graph::NodeIndex>,
    meta_path: &str,
    data_path: &str,
) -> crate::lang::Result<()> {
    info!(
        "txiter: {}, nodes: {}, edges: {}",
        txiter,
        g.node_count(),
        g.edge_count()
    );
    info!("writing graph metadata to {}..", meta_path);
    let f = std::fs::File::create(meta_path)?;
    bincode::serialize_into(f, &node_index_map).context("meta bincode ser err")?;
    // serde_json::to_writer(f, &node_index_map)?;
    info!("writing graph data to {}..", data_path);
    let f = std::fs::File::create(data_path)?;
    bincode::serialize_into(f, &g).context("g bincode ser err")?;
    // serde_json::to_writer(f, &g)?;
    info!("done");
    Ok(())
}
