#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContactWindow {
    pub tx_node_id: u16,
    pub rx_node_id: u16,
    pub start: f64,
    pub end: f64,
    pub rate: f64,
    pub delay: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TopologySnapshot {
    pub contacts: Vec<ContactWindow>,
}

impl TopologySnapshot {
    pub fn next_boundary_after(&self, now: f64) -> Option<f64> {
        self.contacts
            .iter()
            .flat_map(|contact| [contact.start, contact.end])
            .filter(|boundary| *boundary > now)
            .min_by(|a, b| a.total_cmp(b))
    }
}
