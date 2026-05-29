use crate::graph::{self, HighwayType, RoadGraph};
use indicatif::{ProgressBar, ProgressStyle};
use osmpbf::{Element, ElementReader};
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

fn tag_map<'a>(iter: impl Iterator<Item = (&'a str, &'a str)>) -> HashMap<String, String> {
    iter.map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// Build a road graph from an OSM PBF file.
pub fn build_graph(path: &Path) -> RoadGraph {
    let start = Instant::now();

    // Phase 1: collect node coordinates
    let pb = ProgressBar::new(5000);
    pb.set_style(ProgressStyle::default_bar().template("[{elapsed_precise}] {bar:20} {pos} {msg}").unwrap());
    pb.set_message("collecting nodes");

    let mut nodes: HashMap<i64, (f64, f64)> = HashMap::new();
    let mut element_count = 0u64;

    let reader1 = ElementReader::from_path(path).expect("open PBF");
    let _ = reader1.for_each(|element| {
        match element {
            Element::Node(n) => { nodes.insert(n.id(), (n.lat(), n.lon())); }
            Element::DenseNode(n) => { nodes.insert(n.id(), (n.lat(), n.lon())); }
            _ => {}
        }
        element_count += 1;
        if element_count % 100_000 == 0 { pb.set_position(element_count.min(5000)); }
    });
    pb.finish_with_message(format!("{} nodes", nodes.len()));

    // Phase 2: process ways
    let pb2 = ProgressBar::new(5000);
    pb2.set_style(ProgressStyle::default_bar().template("[{elapsed_precise}] {bar:20} {pos} ways {msg}").unwrap());
    pb2.set_message("building graph");

    let mut graph = RoadGraph::new();
    let mut node_id_map: HashMap<i64, u32> = HashMap::new();
    let mut way_count = 0u64;
    let _skip_node = 0i64;

    let reader2 = ElementReader::from_path(path).expect("open PBF");
    let _ = reader2.for_each(|element| {
        if let Element::Way(way) = element {
            let tags = tag_map(way.tags());
            let highway_str = match tags.get("highway") {
                Some(h) => h.as_str(),
                None => return,
            };

            // Skip non-drivable
            if matches!(highway_str, "footway"|"pedestrian"|"path"|"cycleway"|"track"
                |"bridleway"|"steps"|"corridor"|"elevator"|"proposed"|"construction"|"raceway") {
                return;
            }

            let access = tags.get("access").map(|s| s.as_str()).unwrap_or("");
            let vehicle = tags.get("vehicle").map(|s| s.as_str()).unwrap_or("");
            let mv = tags.get("motor_vehicle").map(|s| s.as_str()).unwrap_or("");
            if access == "no" || vehicle == "no" || mv == "no" { return; }

            let highway = HighwayType::from_str(highway_str);
            let motorroad = tags.get("motorroad") == Some(&"yes".to_string());
            let oneway = tags.get("oneway") == Some(&"yes".to_string());
            let maxspeed: Option<f32> = tags.get("maxspeed").and_then(|s| s.parse::<f32>().ok());

            // FRENCH ROAD HEURISTIC: Voie Mathis (M6210) and similar "voies rapides" in Nice
            // are often tagged highway=trunk + bicycle=no + foot=no WITHOUT motorroad=yes.
            // Treat these as motorroad=yes to block 50cc/voiturettes.
            let bicycle_tag = tags.get("bicycle").map(|s| s.as_str()).unwrap_or("");
            let foot_tag = tags.get("foot").map(|s| s.as_str()).unwrap_or("");
            // In France, highway=trunk + bicycle=no + foot=no is a dead giveaway
            // for a voie rapide (even without motorroad=yes or sidewalk=no tags).
            // Also catch elevated roads (bridge/viaduct/layer).
            // Also catch trunk roads with speed limit >= 80 km/h (50cc illegal).
            let is_voie_rapide = highway == HighwayType::Trunk
                && ((foot_tag == "no" && bicycle_tag == "no")
                    || tags.get("bridge").is_some()
                    || tags.get("layer").is_some()
                    || maxspeed.map_or(false, |s| s >= 80.0));

            // Elevate motorroad flag if this is a disguised voie rapide
            let effective_motorroad = motorroad || is_voie_rapide;

            let refs: Vec<i64> = way.refs().collect();
            if refs.len() < 2 { return; }

            // Get or create node indices
            let indices: Vec<Option<u32>> = refs.iter().map(|id| {
                nodes.get(id).map(|&(lat, lon)| {
                    if let Some(&idx) = node_id_map.get(id) { idx }
                    else {
                        let idx = graph.add_node(*id, lat, lon);
                        node_id_map.insert(*id, idx);
                        idx
                    }
                })
            }).collect();

            for i in 0..indices.len().saturating_sub(1) {
                let (from, to) = match (indices[i], indices[i + 1]) {
                    (Some(f), Some(t)) => (f, t),
                    _ => continue,
                };

                let p_from = nodes[&refs[i]];
                let p_to = match nodes.get(&refs[i + 1]) { Some(p) => *p, None => continue };

                let dist = graph::haversine_m(p_from.0, p_from.1, p_to.0, p_to.1);
                if dist < 0.5 { continue; }

                let speed = maxspeed.unwrap_or_else(|| highway.base_speed());

                graph.add_edge_raw(from, to, dist, speed, highway, oneway, effective_motorroad);

                if !oneway {
                    graph.add_edge_raw(to, from, dist, speed, highway, false, effective_motorroad);
                }
            }

            way_count += 1;
            if way_count % 10_000 == 0 {
                pb2.set_position(way_count.min(5000));
            }
        }
    });

    pb2.finish_with_message(format!(
        "{} nodes, {} edges",
        graph.nodes.len(),
        graph.adjacency.iter().map(|v| v.len()).sum::<usize>()
    ));

    tracing::info!("Import completed in {:.1}s", start.elapsed().as_secs_f64());
    graph
}
