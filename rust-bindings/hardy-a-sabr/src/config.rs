use std::time::Duration;

use crate::{
    Router, engine::ShadowEngineConfig, projection::ProjectionConfig, topology::TopologySnapshot,
};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RuntimeConfig {
    pub source: u16,
    #[cfg_attr(feature = "serde", serde(default))]
    pub start_time: f64,
    #[cfg_attr(feature = "serde", serde(default = "default_safety_tick_secs"))]
    pub safety_tick_secs: u64,
    pub topology: TopologySnapshot,
    pub projection: ProjectionConfig,
    #[cfg_attr(feature = "serde", serde(default))]
    pub engine: ShadowEngineConfig,
}

impl RuntimeConfig {
    pub fn safety_tick(&self) -> Duration {
        Duration::from_secs(self.safety_tick_secs)
    }
    pub fn into_router(self) -> Router {
        let safety_tick = self.safety_tick();
        Router::new(self.source, self.topology, self.projection)
            .with_start_time(self.start_time)
            .with_safety_tick(safety_tick)
            .with_engine_config(self.engine)
    }
}

#[cfg(feature = "serde")]
fn default_safety_tick_secs() -> u64 {
    60
}
