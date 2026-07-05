use std::sync::{Arc, Mutex};

use hardy_bpa::routing::RoutingSink;

use crate::{
    engine::ShadowEngineConfig,
    projection::ProjectionConfig,
    refresh::{RefreshError, refresh_routes},
    routes::ProjectedRoute,
    topology::TopologySnapshot,
};

pub struct Router {
    pub(crate) source: u16,
    pub(crate) topology: Mutex<TopologySnapshot>,
    pub(crate) engine_config: ShadowEngineConfig,
    pub(crate) projection_config: ProjectionConfig,
    pub(crate) installed: Mutex<Vec<ProjectedRoute>>,
    pub(crate) sink: Mutex<Option<Arc<dyn RoutingSink>>>,
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
            installed: Mutex::new(Vec::new()),
            sink: Mutex::new(None),
        }
    }
    pub fn with_engine_config(mut self, engine_config: ShadowEngineConfig) -> Self {
        self.engine_config = engine_config;
        self
    }

    pub(crate) async fn refresh_with_sink(
        &self,
        sink: &dyn RoutingSink,
        now: f64,
    ) -> Result<(), RefreshError> {
        let topology = self.topology.lock().unwrap().clone();
        let mut installed = self.installed.lock().unwrap().clone();

        refresh_routes(
            sink,
            &mut installed,
            &topology,
            &self.engine_config,
            &self.projection_config,
            self.source,
            now,
        )
        .await?;

        let mut stored_installed = self.installed.lock().unwrap();
        *stored_installed = installed;

        Ok(())
    }
}
