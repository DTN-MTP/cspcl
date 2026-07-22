use a_sabr::{
    bundle::Bundle,
    contact::{Contact, ContactInfo},
    contact_manager::legacy::evl::EVLManager,
    contact_plan::{ContactPlan, RealNode},
    errors::ASABRError,
    multigraph::RoutableNodeRef,
    node::{Node, NodeInfo},
    node_manager::none::NoManagement,
    pathfinding::top_level::aliases::SpsnHybridParenting,
    utils::{Router, make_guard},
};

use crate::{projection::RepresentativeBundle, topology::TopologySnapshot};

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShadowEngineConfig {
    pub max_entries: usize,
}

impl Default for ShadowEngineConfig {
    fn default() -> Self {
        Self { max_entries: 10 }
    }
}

pub fn build_contact_plan(
    topology: &TopologySnapshot,
) -> Result<ContactPlan<NoManagement, EVLManager>, ASABRError> {
    let mut nodes = topology
        .nodes
        .iter()
        .filter_map(|node| {
            Node::try_new(
                NodeInfo {
                    id: (node.asabr_node_id as usize).into(),
                    name: node.hardy_node_id.to_string().into(),
                    excluded: false,
                },
                NoManagement {},
            )
            .map(|inode| (node.asabr_node_id, RealNode::Inode(inode)))
        })
        .collect::<Vec<_>>();

    nodes.sort_by_key(|(id, _)| *id);

    debug_assert!(
        nodes
            .iter()
            .enumerate()
            .all(|(index, (id, _))| index as u16 == *id),
        "asabr_node_id values must be contiguous starting at 0"
    );

    let realnodes = nodes.into_iter().map(|(_, node)| node).collect::<Vec<_>>();

    let contacts = topology
        .contacts
        .iter()
        .filter_map(|contact| {
            Contact::try_new(
                ContactInfo::new(
                    (contact.tx_node_id as usize).into(),
                    (contact.rx_node_id as usize).into(),
                    contact.start,
                    contact.end,
                ),
                EVLManager::new(contact.rate, contact.delay),
            )
        })
        .collect::<Vec<_>>();

    Ok(ContactPlan::new(realnodes, Vec::new(), contacts))
}

pub fn compute_first_hop(
    topology: &TopologySnapshot,
    config: &ShadowEngineConfig,
    source: u16,
    destination: u16,
    now: i64,
    representative: &RepresentativeBundle,
) -> Result<Option<u16>, ASABRError> {
    let contact_plan = build_contact_plan(topology)?;

    make_guard!(id);
    let mut router = Router::<_, _, SpsnHybridParenting<1, _, _, _>, RoutableNodeRef>::build(
        id,
        contact_plan,
        (config.max_entries, ()),
    )?;

    let bundle = Bundle {
        priority: representative.priority,
        size: representative.size,
        expiration: now + representative.expiration_horizon,
    };

    let Some(source_ref) = router.node_id_ref((source as usize).into())?.internal() else {
        return Ok(None);
    };
    let destination_ref = router
        .node_id_ref((destination as usize).into())?
        .routable()?;

    let output = router.route(destination_ref, now, source_ref, &bundle, None)?;

    Ok(output.map(|(_path, first_hop)| {
        usize::from(
            first_hop
                .rx_node
                .internal()
                .expect("A-SABR first-hop rx_node is always an internal node"),
        ) as u16
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topology::{ContactWindow, NodeMapping, TopologySnapshot};
    use hardy_bpv7::eid::NodeId;

    fn node(asabr_node_id: u16, eid: &str) -> NodeMapping {
        NodeMapping {
            asabr_node_id,
            hardy_node_id: eid.parse::<NodeId>().expect("valid node id"),
        }
    }

    fn contact(tx: u16, rx: u16, start: i64, end: i64) -> ContactWindow {
        ContactWindow {
            tx_node_id: tx,
            rx_node_id: rx,
            start,
            end,
            rate: 1_000_000,
            delay: 1,
        }
    }

    #[test]
    fn first_hop_follows_linear_plan() {
        // 0 -> 1 -> 2, all contacts open for the whole horizon.
        let topology = TopologySnapshot {
            nodes: vec![node(0, "ipn:1.0"), node(1, "ipn:2.0"), node(2, "ipn:3.0")],
            contacts: vec![contact(0, 1, 0, 100_000), contact(1, 2, 0, 100_000)],
        };
        let config = ShadowEngineConfig::default();
        let representative = RepresentativeBundle::default();

        let hop = compute_first_hop(&topology, &config, 0, 2, 0, &representative)
            .expect("routing succeeds");
        assert_eq!(hop, Some(1));
    }

    #[test]
    fn no_path_returns_none() {
        // 0 -> 1 only; node 2 is unreachable.
        let topology = TopologySnapshot {
            nodes: vec![node(0, "ipn:1.0"), node(1, "ipn:2.0"), node(2, "ipn:3.0")],
            contacts: vec![contact(0, 1, 0, 100_000)],
        };
        let config = ShadowEngineConfig::default();
        let representative = RepresentativeBundle::default();

        let hop = compute_first_hop(&topology, &config, 0, 2, 0, &representative)
            .expect("routing succeeds");
        assert_eq!(hop, None);
    }
}
