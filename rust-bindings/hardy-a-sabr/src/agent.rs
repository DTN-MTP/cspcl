use std::sync::Arc;

use hardy_bpa::{
    async_trait,
    routing::{RoutingAgent, RoutingSink},
};
use hardy_bpv7::eid::NodeId;

use crate::{
    router::Router,
    routes::{apply_route_diff, diff_routes},
};

#[async_trait]
impl RoutingAgent for Router {
    async fn on_register(&self, sink: Box<dyn RoutingSink>, _node_ids: &[NodeId]) {
        let sink: Arc<dyn RoutingSink> = sink.into();

        {
            let mut stored_sink = self.sink.lock().unwrap();
            *stored_sink = Some(sink.clone());
        }

        if let Err(error) = self.refresh_with_sink(&*sink, 0.0).await {
            eprintln!(
                "A-SABR initial route refresh failed:
          {error:?}"
            );
            return;
        }
    }

    async fn on_unregister(&self) {
        let sink = self.sink.lock().unwrap().clone();
        let installed = self.installed.lock().unwrap().clone();

        if let Some(sink) = sink {
            let diff = diff_routes(&installed, &[]);

            if let Err(error) = apply_route_diff(&*sink, &diff).await {
                eprintln!(
                    "A-SABR route withdrawal failed:
              {error:?}"
                );
            }
        }

        let mut stored_installed = self.installed.lock().unwrap();
        stored_installed.clear();

        let mut stored_sink = self.sink.lock().unwrap();
        *stored_sink = None;
    }
}
