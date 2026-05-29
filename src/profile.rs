use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VehicleType {
    Scooter50,
    Voiturette,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub max_speed_kmh: f64,
    pub block_motorway: bool,
    pub block_motorroad: bool,
}

impl Profile {
    pub fn for_vehicle(v: VehicleType) -> Self {
        match v {
            VehicleType::Scooter50 => Self {
                name: "scooter".into(),
                max_speed_kmh: 45.0,
                block_motorway: true,
                block_motorroad: true,
            },
            VehicleType::Voiturette => Self {
                name: "voiturette".into(),
                max_speed_kmh: 70.0,
                block_motorway: true,
                block_motorroad: true,
            },
        }
    }

    /// Is this road blocked? Checks highway type + motorroad flag.
    pub fn is_road_blocked(&self, highway: &crate::graph::HighwayType, motorroad: bool) -> bool {
        if self.block_motorway && highway.is_motorway() {
            return true;
        }
        // KEY: block any road with motorroad=yes, regardless of highway type
        // Voie Mathis is trunk + motorroad=yes → blocked
        if self.block_motorroad && motorroad {
            return true;
        }
        false
    }

    /// Get speed limit for this highway type under this profile.
    /// Returns None if the road type is not usable.
    pub fn allowed_speed(&self, highway: &crate::graph::HighwayType, road_speed: f64) -> Option<f64> {
        let base: f64 = match highway {
            crate::graph::HighwayType::Motorway | crate::graph::HighwayType::MotorwayLink => return None,
            crate::graph::HighwayType::Trunk | crate::graph::HighwayType::TrunkLink => 45.0,
            crate::graph::HighwayType::Primary | crate::graph::HighwayType::PrimaryLink => 45.0,
            crate::graph::HighwayType::Secondary | crate::graph::HighwayType::SecondaryLink => 40.0,
            crate::graph::HighwayType::Tertiary | crate::graph::HighwayType::TertiaryLink => 35.0,
            crate::graph::HighwayType::Residential => 30.0,
            crate::graph::HighwayType::LivingStreet => 20.0,
            crate::graph::HighwayType::Service => 20.0,
            crate::graph::HighwayType::Unclassified | crate::graph::HighwayType::Road => 30.0,
            crate::graph::HighwayType::Other => 30.0,
        };
        let limit = base.min(self.max_speed_kmh);
        // Also cap at road's own speed limit if it's slower
        if road_speed > 0.0 { Some(limit.min(road_speed)) } else { Some(limit) }
    }
}
