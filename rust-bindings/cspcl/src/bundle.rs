use crate::addressing::RemotePeer;

/// Metadata and payload for a bundle received from CSPCL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedBundle {
    pub data: Vec<u8>,
    pub src_addr: u8,
    pub src_port: u8,
}

/// Metadata for a bundle written into a caller-provided buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReceivedBundleView {
    pub len: usize,
    pub src_addr: u8,
    pub src_port: u8,
}

impl ReceivedBundle {
    pub fn remote_peer(&self) -> RemotePeer {
        RemotePeer::new(self.src_addr, self.src_port)
    }
}

impl ReceivedBundleView {
    pub fn remote_peer(&self) -> RemotePeer {
        RemotePeer::new(self.src_addr, self.src_port)
    }
}
