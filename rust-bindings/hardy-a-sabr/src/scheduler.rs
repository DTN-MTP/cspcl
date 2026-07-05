use std::sync::Arc;

use hardy_async::{CancellationToken, JoinHandle, channel};
use hardy_bpa::routing::RoutingSink;

use crate::topology::TopologySnapshot;

#[derive(Debug)]
pub(crate) enum SchedulerCommand {
    Refresh,
    TopologyChanged(TopologySnapshot),
    Shutdown,
}

struct SchedulerHandle {
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
}

impl Scheduler {
    pub(crate) fn new(sink: Arc<dyn RoutingSink>) -> (Self, SchedulerHandle) {
        let (sender, receiver) = channel::unbounded();
        let cancel = CancellationToken::new();
        let scheduler = Self {
            sink,
            receiver,
            cancel: cancel.clone(),
        };

        let handle = SchedulerHandle { sender, cancel };
        (scheduler, handle)
    }

    pub(crate) fn start(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            self.run().await;
        })
    }

    async fn run(self) {
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => break,
                command = self.receiver.recv() =>{
                    match command{
                        Ok(SchedulerCommand::Refresh)=> {},
                        Ok(SchedulerCommand::TopologyChanged(_topology))=> {},
                        Ok(SchedulerCommand::Shutdown)=> {},
                        Err(_) => break,
                    }
                }
            }
        }
    }
}
