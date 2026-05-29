use rstar::{PointDistance, RTree, RTreeObject, AABB};
use serde::{Deserialize, Serialize};

/// Compact highway type tag — 1 byte instead of a String
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum HighwayType {
    Motorway = 0,
    MotorwayLink = 1,
    Trunk = 2,
    TrunkLink = 3,
    Primary = 4,
    PrimaryLink = 5,
    Secondary = 6,
    SecondaryLink = 7,
    Tertiary = 8,
    TertiaryLink = 9,
    Residential = 10,
    LivingStreet = 11,
    Service = 12,
    Unclassified = 13,
    Road = 14,
    Other = 15,
}

impl HighwayType {
    pub fn from_str(s: &str) -> Self {
        match s {
            "motorway" => Self::Motorway,
            "motorway_link" => Self::MotorwayLink,
            "trunk" => Self::Trunk,
            "trunk_link" => Self::TrunkLink,
            "primary" => Self::Primary,
            "primary_link" => Self::PrimaryLink,
            "secondary" => Self::Secondary,
            "secondary_link" => Self::SecondaryLink,
            "tertiary" => Self::Tertiary,
            "tertiary_link" => Self::TertiaryLink,
            "residential" => Self::Residential,
            "living_street" => Self::LivingStreet,
            "service" => Self::Service,
            "unclassified" => Self::Unclassified,
            "road" => Self::Road,
            _ => Self::Other,
        }
    }

    pub fn is_motorway(&self) -> bool {
        matches!(self, Self::Motorway | Self::MotorwayLink)
    }

    pub fn is_trunk(&self) -> bool {
        matches!(self, Self::Trunk | Self::TrunkLink)
    }

    /// Base speed in km/h for this road type (used as fallback)
    pub fn base_speed(&self) -> f32 {
        match self {
            Self::Motorway | Self::MotorwayLink => 110.0,
            Self::Trunk | Self::TrunkLink => 90.0,
            Self::Primary | Self::PrimaryLink => 80.0,
            Self::Secondary | Self::SecondaryLink => 70.0,
            Self::Tertiary | Self::TertiaryLink => 50.0,
            Self::Residential => 30.0,
            Self::LivingStreet => 20.0,
            Self::Service => 30.0,
            Self::Unclassified | Self::Road => 50.0,
            Self::Other => 50.0,
        }
    }
}

/// A node in the road graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub osm_id: i64,
    pub lat: f64,
    pub lon: f64,
}

/// A directed edge in the road graph — compact (40 bytes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadEdge {
    pub to: u32,
    /// Distance in meters (stored as u16 * 5m resolution → up to 327m per edge segment)
    pub length: u16,
    /// Speed in km/h (stored as u8, 2 km/h resolution → max 510 km/h)
    pub speed: u8,
    pub highway: HighwayType,
    pub oneway: bool,
    pub motorroad: bool,
}

impl RoadEdge {
    pub fn length_m(&self) -> f64 {
        self.length as f64 * 5.0
    }
    pub fn speed_kmh(&self) -> f64 {
        self.speed as f64 * 2.0
    }
}

/// The road graph — adjacency list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadGraph {
    pub nodes: Vec<GraphNode>,
    pub adjacency: Vec<Vec<RoadEdge>>,
}

impl RoadGraph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            adjacency: Vec::new(),
        }
    }

    pub fn add_node(&mut self, osm_id: i64, lat: f64, lon: f64) -> u32 {
        let idx = self.nodes.len() as u32;
        self.nodes.push(GraphNode {
            osm_id,
            lat,
            lon,
        });
        self.adjacency.push(Vec::new());
        idx
    }

    pub fn add_edge_raw(&mut self, from: u32, to: u32, length_m: f64, speed_kmh: f32, highway: HighwayType, oneway: bool, motorroad: bool) {
        let length = (length_m / 5.0).round().min(u16::MAX as f64) as u16;
        let speed = (speed_kmh / 2.0).round().min(u8::MAX as f32) as u8;
        self.adjacency[from as usize].push(RoadEdge { to, length, speed, highway, oneway, motorroad });
    }

    pub fn build_spatial_index(&self) -> RTree<IdxPoint> {
        let points: Vec<IdxPoint> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| IdxPoint { idx: i as u32, lon: n.lon, lat: n.lat })
            .collect();
        RTree::bulk_load(points)
    }

    pub fn nearest_node(&self, tree: &RTree<IdxPoint>, lat: f64, lon: f64, max_dist_m: f64) -> Option<u32> {
        let nearest = tree.nearest_neighbor([lon, lat])?;
        let dlat = (lat - nearest.lat) * 111_320.0;
        let dlon = (lon - nearest.lon) * 111_320.0 * 0.72;
        if (dlat * dlat + dlon * dlon).sqrt() > max_dist_m {
            return None;
        }
        Some(nearest.idx)
    }

    pub fn heuristic(&self, from: u32, to: u32, max_speed_kmh: f64) -> f64 {
        let a = &self.nodes[from as usize];
        let b = &self.nodes[to as usize];
        haversine_m(a.lat, a.lon, b.lat, b.lon) / (max_speed_kmh / 3.6)
    }
}

// ---- Spatial index ----

#[derive(Debug, Clone, Copy)]
pub struct IdxPoint {
    pub idx: u32,
    pub lon: f64,
    pub lat: f64,
}

impl RTreeObject for IdxPoint {
    type Envelope = AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        AABB::from_point([self.lon, self.lat])
    }
}

impl PointDistance for IdxPoint {
    fn distance_2(&self, other: &[f64; 2]) -> f64 {
        let dl = self.lon - other[0];
        let dlt = self.lat - other[1];
        dl * dl + dlt * dlt
    }
}

// ---- Haversine ----

pub fn haversine_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6_371_000.0;
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
}
