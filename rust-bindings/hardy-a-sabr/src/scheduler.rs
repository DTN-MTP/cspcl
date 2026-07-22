use std::{sync::Arc, time::Duration};

use hardy_async::{CancellationToken, JoinHandle, channel};
use hardy_bpa::routing::RoutingSink;
use tracing::warn;

use crate::{
    engine::ShadowEngineConfig,
    projection::ProjectionConfig,
    refresh::refresh_routes,
    routes::{ProjectedRoute, apply_route_diff, diff_routes},
    topology::TopologySnapshot,
};

#[derive(Clone, Debug)]
pub(crate) enum SchedulerCommand {
    Refresh,
    TopologyChanged(TopologySnapshot),
    Shutdown,
}

#[derive(Clone)]
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
    started_at: tokio::time::Instant,
    safety_tick: Duration,
    start_time: i64,
}

impl Scheduler {
    pub(crate) fn new(
        sink: Arc<dyn RoutingSink>,
        source: u16,
        topology: TopologySnapshot,
        engine_config: ShadowEngineConfig,
        projection_config: ProjectionConfig,
        start_time: i64,
        safety_tick: Duration,
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
            started_at: tokio::time::Instant::now(),
            safety_tick,
            start_time,
        };

        let handle = SchedulerHandle { sender, cancel };
        (scheduler, handle)
    }

    fn now(&self) -> i64 {
        self.start_time + self.started_at.elapsed().as_millis() as i64
    }

    fn next_boundary_delay(&self, now: i64) -> Option<Duration> {
        self.topology
            .next_boundary_after(now)
            .map(|boundary| Duration::from_millis((boundary - now).max(0) as u64))
    }

    fn next_wakeup_delay(&self, now: i64) -> Duration {
        self.next_boundary_delay(now)
            .map(|boundary_delay| boundary_delay.min(self.safety_tick))
            .unwrap_or(self.safety_tick)
    }

    pub(crate) fn start(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            self.run().await;
        })
    }

    async fn run(mut self) {
        loop {
            let delay = self.next_wakeup_delay(self.now());

            tokio::select! {
                _ = self.cancel.cancelled() => break,
                _ = tokio::time::sleep(delay) =>{
                self.refresh(self.now()).await;
            }
            command = self.receiver.recv() => {
                    if !self.handle_command(command).await {
                        break;
                    }
                }
            }
        }
        self.withdraw().await;
    }

    async fn refresh(&mut self, now: i64) {
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
            warn!(?error, "A-SABR scheduled route refresh failed");
        }
    }

    async fn withdraw(&mut self) {
        let diff = diff_routes(&self.installed, &[]);
        if let Err(error) = apply_route_diff(&*self.sink, &diff).await {
            warn!(?error, "A-SABR scheduled route withdrawal failed");
            return;
        };
        self.installed.clear();
    }

    async fn handle_command(
        &mut self,
        command: Result<SchedulerCommand, channel::RecvError>,
    ) -> bool {
        match command {
            Ok(SchedulerCommand::Refresh) => {
                self.refresh(self.now()).await;
                true
            }
            Ok(SchedulerCommand::TopologyChanged(topology)) => {
                self.topology = topology;
                self.refresh(self.now()).await;
                true
            }
            Ok(SchedulerCommand::Shutdown) | Err(_) => false,
        }
    }
}
