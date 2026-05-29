use crate::graph::RoadGraph;
use crate::profile::Profile;
use std::collections::BinaryHeap;

#[derive(Debug, Clone, PartialEq)]
struct State {
    estimated_cost: f64,
    node: u32,
}
impl Eq for State {}
impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        other.estimated_cost.partial_cmp(&self.estimated_cost)
    }
}
impl Ord for State {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(std::cmp::Ordering::Equal)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RouteResult {
    pub found: bool,
    pub distance_km: f64,
    pub duration_min: u64,
    pub path: Vec<[f64; 2]>,
}

/// A* shortest path
pub fn route(
    graph: &RoadGraph,
    from: u32,
    to: u32,
    profile: &Profile,
    _spatial: &rstar::RTree<crate::graph::IdxPoint>,
) -> Option<RouteResult> {
    let n = graph.nodes.len();
    let mut came_from = vec![u32::MAX; n];
    let mut cost_so_far = vec![f64::MAX; n];

    let mut heap = BinaryHeap::new();
    cost_so_far[from as usize] = 0.0;
    heap.push(State { estimated_cost: graph.heuristic(from, to, profile.max_speed_kmh), node: from });

    while let Some(State { node, .. }) = heap.pop() {
        let nu = node as usize;
        if node == to {
            return Some(reconstruct(graph, came_from, from, to));
        }
        let cur_cost = cost_so_far[nu];

        for edge in &graph.adjacency[nu] {
            if profile.is_road_blocked(&edge.highway, edge.motorroad) {
                continue;
            }

            let speed = match profile.allowed_speed(&edge.highway, edge.speed_kmh()) {
                Some(s) => s,
                None => continue,
            };
            if speed <= 0.0 { continue; }

            let time_s = edge.length_m() / (speed / 3.6);
            let new_cost = cur_cost + time_s;

            if new_cost < cost_so_far[edge.to as usize] {
                cost_so_far[edge.to as usize] = new_cost;
                came_from[edge.to as usize] = node;
                heap.push(State {
                    estimated_cost: new_cost + graph.heuristic(edge.to, to, profile.max_speed_kmh),
                    node: edge.to,
                });
            }
        }
    }
    None
}

fn reconstruct(graph: &RoadGraph, came_from: Vec<u32>, start: u32, goal: u32) -> RouteResult {
    let mut path_nodes = Vec::new();
    let mut current = goal;
    while current != u32::MAX {
        path_nodes.push(current);
        if current == start { break; }
        current = came_from[current as usize];
    }
    path_nodes.reverse();

    let path: Vec<[f64; 2]> = path_nodes.iter()
        .map(|&n| { let gn = &graph.nodes[n as usize]; [gn.lon, gn.lat] })
        .collect();

    let mut total_dist = 0.0;
    for win in path_nodes.windows(2) {
        for e in &graph.adjacency[win[0] as usize] {
            if e.to == win[1] {
                total_dist += e.length_m();
                break;
            }
        }
    }

    let distance_km = (total_dist / 1000.0 * 10.0).round() / 10.0;
    let duration_min = ((total_dist / (45.0 / 3.6)) / 60.0).round() as u64;

    RouteResult { found: true, distance_km, duration_min, path }
}
