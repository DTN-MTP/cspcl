use crate::{ConnectionStats, ReceivedBundle, ReceivedBundleView, Result};
use crate::{Cspcl as SyncCspcl, Receiver as SyncReceiver, Sender as SyncSender};
use tokio::task;

#[derive(Clone)]
pub struct Cspcl {
    inner: SyncCspcl,
}

#[derive(Clone)]
pub struct Sender {
    inner: SyncSender,
}

#[derive(Clone)]
pub struct Receiver {
    inner: SyncReceiver,
}

impl Cspcl {
    pub fn from_sync(cspcl: SyncCspcl) -> Self {
        Self { inner: cspcl }
    }

    pub fn sender(&self) -> Sender {
        Sender {
            inner: self.inner.sender(),
        }
    }

    pub fn receiver(&self) -> Receiver {
        Receiver {
            inner: self.inner.receiver(),
        }
    }

    pub fn split(&self) -> (Sender, Receiver) {
        (self.sender(), self.receiver())
    }

    pub fn is_initialized(&self) -> bool {
        self.inner.is_initialized()
    }

    pub fn local_addr(&self) -> u8 {
        self.inner.local_addr()
    }

    pub fn connection_stats(&self) -> ConnectionStats {
        self.inner.connection_stats()
    }

    pub async fn shutdown(&self) -> crate::Result<()> {
        let cspcl = self.inner.clone();
        task::spawn_blocking(move || cspcl.shutdown())
            .await
            .expect("async shutdown task panicked")
    }
}

impl Sender {
    pub fn from_sync(sender: SyncSender) -> Self {
        Self { inner: sender }
    }

    pub async fn send_bundle(&self, bundle: &[u8], dest_addr: u8, dest_port: u8) -> Result<()> {
        let sender = self.inner.clone();
        let bundle = bundle.to_vec();
        task::spawn_blocking(move || sender.send_bundle(&bundle, dest_addr, dest_port))
            .await
            .expect("async send task panicked")
    }

    pub fn connection_stats(&self) -> ConnectionStats {
        self.inner.connection_stats()
    }
}

impl Receiver {
    pub fn from_sync(receiver: SyncReceiver) -> Self {
        Self { inner: receiver }
    }

    pub async fn recv_bundle(&self, timeout_ms: u32) -> Result<ReceivedBundle> {
        let receiver = self.inner.clone();
        task::spawn_blocking(move || receiver.recv_bundle(timeout_ms))
            .await
            .expect("async receive task panicked")
    }

    pub async fn recv_bundle_into(
        &self,
        buffer: &mut [u8],
        timeout_ms: u32,
    ) -> Result<ReceivedBundleView> {
        let receiver = self.inner.clone();
        let capacity = buffer.len();
        let (received, tmp) = task::spawn_blocking(move || {
            let mut tmp = vec![0_u8; capacity];
            let received = receiver.recv_bundle_into(&mut tmp, timeout_ms)?;
            Ok::<_, crate::Error>((received, tmp))
        })
        .await
        .expect("async receive-into task panicked")?;

        buffer[..received.len].copy_from_slice(&tmp[..received.len]);
        Ok(received)
    }
}
