use std::sync::Arc;

use hardy_async::{CancellationToken, JoinHandle, channel};
use hardy_bpa::routing::RoutingSink;

use crate::{
    engine::ShadowEngineConfig,
    projection::ProjectionConfig,
    refresh::refresh_routes,
    routes::{ProjectedRoute, apply_route_diff, diff_routes},
    topology::TopologySnapshot,
};

#[derive(Debug)]
pub(crate) enum SchedulerCommand {
    Refresh,
    TopologyChanged(TopologySnapshot),
    Shutdown,
}

pub(crate) struct SchedulerHandle {
    sender: channel::Sender<SchedulerCommand>,
    cancel: CancellationToken,
}

impl SchedulerHandle {
    pub(crate) async fn refresh(&self) {
        let _ = self.sender.send(SchedulerCommand::Refresh).await;
    }

    pub(crate) async fn update_topology(&self, topology: TopologySnapshot) {
        let _ = self
            .sender
            .send(SchedulerCommand::TopologyChanged(topology))
            .await;
    }

    pub(crate) async fn shutdown(&self) {
        let _ = self.sender.send(SchedulerCommand::Shutdown).await;
        self.cancel.cancel();
    }
}

pub(crate) struct Scheduler {
    sink: Arc<dyn RoutingSink>,
    receiver: channel::Receiver<SchedulerCommand>,
    cancel: CancellationToken,
    source: u16,
    topology: TopologySnapshot,
    engine_config: ShadowEngineConfig,
    projection_config: ProjectionConfig,
    installed: Vec<ProjectedRoute>,
}

impl Scheduler {
    pub(crate) fn new(
        sink: Arc<dyn RoutingSink>,

        source: u16,
        topology: TopologySnapshot,
        engine_config: ShadowEngineConfig,
        projection_config: ProjectionConfig,
    ) -> (Self, SchedulerHandle) {
        let (sender, receiver) = channel::unbounded();
        let cancel = CancellationToken::new();
        let scheduler = Self {
            sink,
            receiver,
            cancel: cancel.clone(),
            source,
            topology,
            engine_config,
            projection_config,
            installed: Vec::new(),
        };

        let handle = SchedulerHandle { sender, cancel };
        (scheduler, handle)
    }

    pub(crate) fn start(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            self.run().await;
        })
    }

    async fn run(mut self) {
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => break,
                command = self.receiver.recv() =>{
                    match command{
                        Ok(SchedulerCommand::Refresh)=> {
                            self.refresh(0.0).await;
                        },
                        Ok(SchedulerCommand::TopologyChanged(topology))=> {
                            self.topology = topology;
                            self.refresh(0.0).await;
                        },
                        Ok(SchedulerCommand::Shutdown)=> break,
                        Err(_) => break,
                    }
                }
            }
        }
        self.withdraw().await;
    }

    async fn refresh(&mut self, now: f64) {
        if let Err(error) = refresh_routes(
            &*self.sink,
            &mut self.installed,
            &self.topology,
            &self.engine_config,
            &self.projection_config,
            self.source,
            now,
        )
        .await
        {
            eprintln!(
                "A-SABR scheduled route refresh failed:
          {error:?}"
            );
        }
    }

    async fn withdraw(&mut self) {
        let diff = diff_routes(&self.installed, &[]);
        if let Err(error) = apply_route_diff(&*self.sink, &diff).await {
            eprintln!(
                "A-SABR scheduled route withdrawal failed:
          {error:?}"
            );
            return;
        };
        self.installed.clear();
    }
}
