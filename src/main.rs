mod api;
mod graph;
mod pbf;
mod profile;
mod routing;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "scooter-routing", about = "Routeur pour scooter 50cc & voiture sans permis (Rust)")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Import OSM PBF → build road graph and serialize to binary
    Import {
        /// Path to .osm.pbf file
        input: PathBuf,
        /// Output binary file
        #[arg(short, long, default_value = "region.graph")]
        output: PathBuf,
    },
    /// Start the HTTP routing server
    Serve {
        /// Path to pre-built graph file
        #[arg(short, long, default_value = "region.graph")]
        graph: PathBuf,
        /// Listen address
        #[arg(short, long, default_value = "0.0.0.0:3000")]
        bind: String,
    },
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info,scooter_routing=debug")
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Import { input, output } => {
            tracing::info!("Importing OSM data from {}", input.display());

            let road_graph = pbf::build_graph(&input);
            let num_nodes = road_graph.nodes.len();
            let num_edges: usize = road_graph.adjacency.iter().map(|v| v.len()).sum();

            let bytes = postcard::to_stdvec(&road_graph)
                .expect("serialization failed");
            std::fs::write(&output, &bytes).expect("write failed");

            let mb = bytes.len() as f64 / 1_000_000.0;
            tracing::info!(
                "Graph: {num_nodes} nodes, {num_edges} edges, {mb:.1} MB (postcard) → {}",
                output.display()
            );
        }
        Commands::Serve { graph, bind } => {
            tracing::info!("Loading graph from {}…", graph.display());
            let bytes = std::fs::read(&graph).expect("read graph failed");
            let road_graph: graph::RoadGraph =
                postcard::from_bytes(&bytes)
                    .expect("deserialize graph failed");
            tracing::info!(
                "Loaded: {} nodes, {} edges",
                road_graph.nodes.len(),
                road_graph
                    .adjacency
                    .iter()
                    .map(|v| v.len())
                    .sum::<usize>()
            );

            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            rt.block_on(api::serve(road_graph, &bind));
        }
    }
}
