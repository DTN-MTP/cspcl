use std::{cell::RefCell, rc::Rc};

use a_sabr::{
    contact_manager::legacy::evl::EVLManager, node_manager::none::NoManagement,
    route_storage::cache::TreeCache, routing::aliases::SpsnHybridParenting,
};

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
