#![feature(error_generic_member_access)]
#![feature(vec_pop_if)]

// Reduce the output of the graph by deduplicating edges and counting occurrences

#[path = "../args.rs"]
mod args;
#[path = "../lang.rs"]
mod lang;

use crate::lang::Result;

use args::Args;
use clap::Parser;
use lang::Error;
use tracing::error;

fn run(_args: Args, file_path: String) -> Result<()> {
    use std::collections::{HashMap, HashSet};
    use std::fs::File;
    use std::io::{self, BufRead};
    use std::path::Path;

    // Define structures to hold node and edge data
    #[derive(Debug)]
    struct Node {
        id: usize,
        label: String,
    }

    #[derive(Debug, Eq, PartialEq, Hash)]
    struct Edge {
        from: usize,
        to: usize,
        label: String,
    }

    // Function to parse the file
    fn parse_file(file_path: &str) -> Result<(Vec<Node>, Vec<Edge>)> {
        let path = Path::new(file_path);
        let file = File::open(path)?;
        let reader = io::BufReader::new(file);

        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut node_ids_with_edges = HashSet::new();

        for line in reader.lines() {
            let line = line?;
            if line.contains("->") {
                // Parse edge
                let parts: Vec<&str> = line.split("->").collect();
                let from: usize = parts[0].trim().parse().unwrap();
                let rest: Vec<&str> = parts[1].split("[").collect();
                let to: usize = rest[0].trim().parse().unwrap();
                let label = rest[1].trim().trim_end_matches(']').trim().to_string();
                edges.push(Edge { from, to, label });
                node_ids_with_edges.insert(from);
                node_ids_with_edges.insert(to);
            } else if line.contains("[") {
                // Parse node
                let parts: Vec<&str> = line.split("[").collect();
                let id: usize = parts[0].trim().parse().unwrap();
                let label = parts[1].trim().trim_end_matches(']').trim().to_string();
                nodes.push(Node { id, label });
            }
        }

        // Filter nodes to only include those that have edges
        nodes.retain(|node| node_ids_with_edges.contains(&node.id));
        // Filter edges that are repeated
        edges.dedup_by(|a, b| a.from == b.from && a.to == b.to);

        Ok((nodes, edges))
    }

    // Read and parse the file
    let (nodes, edges) = parse_file(&file_path)?;

    // Create a mapping from old node IDs to new sequential IDs
    let mut id_mapping = HashMap::new();
    let mut label_mapping = HashMap::new();
    let mut label_reverse_mapping = HashMap::new();
    let mut label_counter = 1;

    for (new_id, node) in nodes.iter().enumerate() {
        id_mapping.insert(node.id, new_id + 1); // new IDs start from 1
        label_mapping.insert(label_counter, node.label.clone());
        label_reverse_mapping.insert(node.label.clone(), label_counter);
        label_counter += 1;
    }

    // Aggregate edges by (from, to, label) and count occurrences
    let mut edge_count = HashMap::new();
    for edge in &edges {
        let stripped_label = edge.label.split('|').next().unwrap_or("").to_string();
        let key = (edge.from, edge.to, stripped_label.clone());
        *edge_count.entry(key).or_insert(0) += 1;
    }

    // Debug output to verify parsing
    println!("digraph {{ ");

    for node in nodes {
        let new_id = id_mapping[&node.id];
        let l = "label = \"".to_string()
            + label_reverse_mapping
                .get(&node.label)
                .unwrap()
                .to_string()
                .as_str()
            + "\"";
        println!("{} [{}]", new_id, l);
    }

    for ((from, to, label), count) in edge_count {
        let new_from = id_mapping[&from];
        let new_to = id_mapping[&to];
        let edge_label = format!("{} ({})\"", label, count);
        println!("{} -> {} [{}]", new_from, new_to, edge_label);
    }

    println!("}}");

    // Print out the label mappings
    // println!("\nLabel Mappings:");
    // for (id, label) in &label_mapping {
    //  println!("{}: {}", id, label);
    // }

    Ok(())
}

fn main_fallible() -> Result<()> {
    std::env::set_var("RUST_BACKTRACE", "1");
    tracing_subscriber::fmt::fmt().compact().init();

    let e = std::env::args_os();
    let mut e = e.collect::<Vec<_>>();

    let f = e
        .pop_if(|x| !x.to_str().unwrap().starts_with("-"))
        .ok_or("No file path provided")?;
    let f = f.into_string().map_err(|_| Error::from("err str"))?;

    let args = Args::try_parse_from(e)?;
    run(args, f)?;

    Ok(())
}

fn main() {
    let res = main_fallible();
    if let Err(e) = res {
        error!("{e}");
        let bt = std::error::request_ref::<std::backtrace::Backtrace>(&e);
        if let Some(bt) = bt {
            error!("{bt}");
        }
    }
}
