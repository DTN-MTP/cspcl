use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio_stream::Stream;
use tracing::trace;

use crate::Bundle;

pub struct InboundStream {
    rx: mpsc::UnboundedReceiver<Bundle>,
}

impl Stream for InboundStream {
    type Item = Bundle;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        trace!("Polling inbound stream");
        let poll = self.rx.poll_recv(cx);

        match &poll {
            Poll::Ready(Some(bundle)) => trace!(
                src_addr = bundle.src_addr,
                src_port = bundle.src_port,
                bundle_len = bundle.data.len(),
                "Inbound stream yielded bundle"
            ),
            Poll::Ready(None) => trace!("Inbound stream closed"),
            Poll::Pending => trace!("Inbound stream pending"),
        }

        poll
    }
}

impl InboundStream {
    pub fn new(rx: UnboundedReceiver<Bundle>) -> Self {
        Self { rx }
    }
}
