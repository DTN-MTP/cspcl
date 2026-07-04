use hardy_bpa::{
    async_trait,
    routing::{RoutingAgent, RoutingSink},
};
use hardy_bpv7::eid::NodeId;

use crate::router::Router;

#[async_trait]
impl RoutingAgent for Router {
    async fn on_register(&self, sink: Box<dyn RoutingSink>, node_ids: &[NodeId]) {}

    async fn on_unregister(&self) {}
}
