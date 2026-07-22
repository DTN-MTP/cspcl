use hardy_bpv7::eid::{Eid, NodeId};

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ContactWindow {
    pub tx_node_id: u16,
    pub rx_node_id: u16,
    pub start: i64,
    pub end: i64,
    pub rate: i64,
    pub delay: i64,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NodeMapping {
    pub asabr_node_id: u16,
    pub hardy_node_id: NodeId,
}

#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TopologySnapshot {
    pub nodes: Vec<NodeMapping>,
    pub contacts: Vec<ContactWindow>,
}

impl TopologySnapshot {
    pub fn hardy_eid_for(&self, asabr_node_id: u16) -> Option<Eid> {
        self.nodes
            .iter()
            .find(|node| node.asabr_node_id == asabr_node_id)
            .map(|node| node.hardy_node_id.clone().into())
    }

    pub fn next_boundary_after(&self, now: i64) -> Option<i64> {
        self.contacts
            .iter()
            .flat_map(|contact| [contact.start, contact.end])
            .filter(|boundary| *boundary > now)
            .min()
    }
}
