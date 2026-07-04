use std::{cell::RefCell, rc::Rc};

use a_sabr::{
    bundle::Bundle,
    contact::{Contact, ContactInfo},
    contact_manager::legacy::evl::EVLManager,
    contact_plan::ContactPlan,
    errors::ASABRError,
    node::{Node, NodeInfo},
    node_manager::none::NoManagement,
    route_storage::cache::TreeCache,
    routing::{Router, aliases::SpsnHybridParenting},
    vertex::Vertex,
};

use crate::{projection::RepresentativeBundle, topology::TopologySnapshot};

pub type ShadowRouter = SpsnHybridParenting<NoManagement, EVLManager>;

#[derive(Debug, Clone, PartialEq)]
pub struct ShadowEngineConfig {
    pub check_size: bool,
    pub check_priority: bool,
    pub max_entries: usize,
}

impl Default for ShadowEngineConfig {
    fn default() -> Self {
        Self {
            check_size: false,
            check_priority: false,
            max_entries: 10,
        }
    }
}

pub fn new_route_cache(
    config: &ShadowEngineConfig,
) -> Rc<RefCell<TreeCache<NoManagement, EVLManager>>> {
    Rc::new(RefCell::new(TreeCache::new(
        config.check_size,
        config.check_priority,
        config.max_entries,
    )))
}

pub fn build_contact_plan(
    topology: &TopologySnapshot,
) -> Result<ContactPlan<NoManagement, EVLManager>, ASABRError> {
    let mut vertices = topology
        .nodes
        .iter()
        .filter_map(|node| {
            Node::try_new(
                NodeInfo {
                    id: node.asabr_node_id,
                    name: node.hardy_node_id.to_string().into(),
                    excluded: false,
                },
                NoManagement {},
            )
            .map(Vertex::INode)
        })
        .collect::<Vec<_>>();

    vertices.sort_by(|left, right| {
        let left_id = match left {
            Vertex::INode(node) | Vertex::ENode(node) => node.info.id,
            Vertex::VNode((_, node_id)) => *node_id,
        };
        let right_id = match right {
            Vertex::INode(node) | Vertex::ENode(node) => node.info.id,
            Vertex::VNode((_, node_id)) => *node_id,
        };

        left_id.cmp(&right_id)
    });

    let contacts = topology
        .contacts
        .iter()
        .filter_map(|contact| {
            Contact::try_new(
                ContactInfo::new(
                    contact.tx_node_id,
                    contact.rx_node_id,
                    contact.start,
                    contact.end,
                ),
                EVLManager::new(contact.rate, contact.delay),
            )
        })
        .collect::<Vec<_>>();

    Ok(ContactPlan::new(vertices, contacts, None))
}

pub fn build_shadow_router(
    topology: &TopologySnapshot,
    config: &ShadowEngineConfig,
) -> Result<ShadowRouter, ASABRError> {
    let contact_plan = build_contact_plan(topology)?;
    let route_cache = new_route_cache(config);

    ShadowRouter::new(contact_plan, route_cache, config.check_priority)
}

pub fn compute_first_hop(
    topology: &TopologySnapshot,
    config: &ShadowEngineConfig,
    source: u16,
    destination: u16,
    now: f64,
    representative: &RepresentativeBundle,
) -> Result<Option<u16>, ASABRError> {
    let mut router = build_shadow_router(topology, config)?;

    let bundle = Bundle {
        source,
        destinations: vec![destination],
        priority: representative.priority,
        size: representative.size,
        expiration: now + representative.expiration_horizon,
    };

    let excluded_nodes = Vec::new();

    Ok(router
        .route(source, &bundle, now, &excluded_nodes)?
        .and_then(|output| {
            output
                .lazy_get_for_unicast(destination)
                .map(|(contact, _route)| contact.borrow().info.rx_node_id)
        }))
}
