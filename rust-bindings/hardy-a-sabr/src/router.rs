use std::sync::Mutex;

use crate::{
    engine::ShadowEngineConfig, projection::ProjectionConfig, scheduler::SchedulerHandle,
    topology::TopologySnapshot,
};

pub struct Router {
    pub(crate) source: u16,
    pub(crate) topology: Mutex<TopologySnapshot>,
    pub(crate) engine_config: ShadowEngineConfig,
    pub(crate) projection_config: ProjectionConfig,
    pub(crate) scheduler: Mutex<Option<SchedulerHandle>>,
}

impl Router {
    pub fn new(
        source: u16,
        topology: TopologySnapshot,
        projection_config: ProjectionConfig,
    ) -> Self {
        Self {
            source,
            topology: Mutex::new(topology),
            engine_config: ShadowEngineConfig::default(),
            projection_config,
            scheduler: Mutex::new(None),
        }
    }
    pub fn with_engine_config(mut self, engine_config: ShadowEngineConfig) -> Self {
        self.engine_config = engine_config;
        self
    }

    pub async fn update_topology(&self, topology: TopologySnapshot) {
        {
            let mut stored_topology = self.topology.lock().unwrap();
            *stored_topology = topology.clone();
        }
        let scheduler = self.scheduler.lock().unwrap().clone();
        if let Some(scheduler) = scheduler {
            scheduler.update_topology(topology).await
        }
    }
}
