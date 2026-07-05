use std::sync::Arc;

use hardy_bpa::{
    async_trait,
    routing::{RoutingAgent, RoutingSink},
};
use hardy_bpv7::eid::NodeId;

use crate::{refresh::refresh_routes, router::Router};

#[async_trait]
impl RoutingAgent for Router {
    async fn on_register(&self, sink: Box<dyn RoutingSink>, _node_ids: &[NodeId]) {
        let sink: Arc<dyn RoutingSink> = sink.into();
        {
            let mut stored_sink = self.sink.lock().unwrap();
            *stored_sink = Some(sink.clone());
        }

        let topology = self.topology.lock().unwrap().clone();
        let mut installed = self.installed.lock().unwrap().clone();

        if let Err(error) = refresh_routes(
            &*sink,
            &mut installed,
            &topology,
            &self.engine_config,
            &self.projection_config,
            self.source,
            0.0,
        )
        .await
        {
            eprintln!(
                "A-SABR initial route refresh failed:
              {error:?}"
            );
            return;
        }
        let mut stored_installed = self.installed.lock().unwrap();
        *stored_installed = installed;
    }

    async fn on_unregister(&self) {
        let mut stored_sink = self.sink.lock().unwrap();
        *stored_sink = None;
    }
}
