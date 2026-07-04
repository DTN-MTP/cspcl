use hardy_eid_patterns::EidPattern;

#[derive(Debug, Clone, PartialEq)]
pub struct RepresentativeBundle {
    pub size: f64,
    pub priority: i8,
    pub expiration_horizon: f64,
}

impl Default for RepresentativeBundle {
    fn default() -> Self {
        Self {
            size: 1.0,
            priority: 0,
            expiration_horizon: 3600.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DestinationProjection {
    pub pattern: EidPattern,
    pub asabr_destination: u16,
    pub route_priority: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectionConfig {
    pub bundle: RepresentativeBundle,
    pub destinations: Vec<DestinationProjection>,
}
