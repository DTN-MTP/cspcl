use std::sync::Arc;

use hardy_bpa::{
    async_trait,
    routing::{RoutingAgent, RoutingSink},
};
use hardy_bpv7::eid::NodeId;

use crate::{router::Router, scheduler::Scheduler};

#[async_trait]
impl RoutingAgent for Router {
    async fn on_register(&self, sink: Box<dyn RoutingSink>, _node_ids: &[NodeId]) {
        let sink: Arc<dyn RoutingSink> = sink.into();

        let topology = self.topology.lock().unwrap().clone();

        let (scheduler, handle) = Scheduler::new(
            sink.clone(),
            self.source,
            topology,
            self.engine_config.clone(),
            self.projection_config.clone(),
            self.start_time,
            self.safety_tick,
        );

        scheduler.start();

        {
            let mut stored_scheduler = self.scheduler.lock().unwrap();
            *stored_scheduler = Some(handle.clone());
        }

        handle.refresh().await
    }

    async fn on_unregister(&self) {
        let scheduler = {
            let mut stored_scheduler = self.scheduler.lock().unwrap();
            stored_scheduler.take()
        };
        if let Some(scheduler) = scheduler {
            scheduler.shutdown().await;
        }
    }
}
