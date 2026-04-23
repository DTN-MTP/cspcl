use std::cell::RefCell;
use std::env;
use std::ffi::CString;
use std::rc::Rc;

use a_sabr::contact_manager::{
    ContactManager,
    legacy::{eto::ETOManager, evl::EVLManager, qd::QDManager},
    segmentation::seg::SegmentationManager,
};
use a_sabr::contact_plan::{asabr_file_lexer::FileLexer, from_asabr_lexer::ASABRContactPlan};
use a_sabr::node_manager::none::NoManagement;
use a_sabr::parsing::{ContactMarkerMap, coerce_cm};
use a_sabr::route_storage::cache::TreeCache;
use a_sabr::routing::{Router, aliases::SpsnHybridParenting};

use crate::types::{cspcl_route_error_t, cspcl_route_next_hop_t};

pub(crate) struct AdapterState {
    pub(crate) router: Box<dyn Router<NoManagement, Box<dyn ContactManager>>>,
    pub(crate) next_hops: Vec<cspcl_route_next_hop_t>,
    pub(crate) diagnostic: CString,
}

thread_local! {
    static STATE: RefCell<Option<AdapterState>> = const { RefCell::new(None) };
}

fn get_env_string(name: &str) -> Option<String> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => None,
    }
}

fn get_default_contact_plan_path() -> Result<String, cspcl_route_error_t> {
    get_env_string("CSPCL_ASABR_CONTACT_PLAN_PATH")
        .ok_or(cspcl_route_error_t::CSPCL_ROUTE_ERR_PROVIDER_FAILED)
}

fn build_contact_dispatch() -> ContactMarkerMap<'static> {
    let mut contact_dispatch = ContactMarkerMap::new();
    contact_dispatch.add("evl", coerce_cm::<EVLManager>);
    contact_dispatch.add("qd", coerce_cm::<QDManager>);
    contact_dispatch.add("eto", coerce_cm::<ETOManager>);
    contact_dispatch.add("seg", coerce_cm::<SegmentationManager>);
    contact_dispatch
}

fn load_router()
-> Result<Box<dyn Router<NoManagement, Box<dyn ContactManager>>>, cspcl_route_error_t> {
    let cp_path = get_default_contact_plan_path()?;
    let mut lexer = FileLexer::new(&cp_path)
        .map_err(|_| cspcl_route_error_t::CSPCL_ROUTE_ERR_PROVIDER_FAILED)?;
    let contact_dispatch = build_contact_dispatch();

    let contact_plan = ASABRContactPlan::parse::<NoManagement, Box<dyn ContactManager>>(
        &mut lexer,
        None,
        Some(&contact_dispatch),
    )
    .map_err(|_| cspcl_route_error_t::CSPCL_ROUTE_ERR_PROVIDER_FAILED)?;

    let table = Rc::new(RefCell::new(TreeCache::new(true, false, 10)));
    let router = SpsnHybridParenting::<NoManagement, Box<dyn ContactManager>>::new(
        contact_plan,
        table,
        false,
    )
    .map_err(|_| cspcl_route_error_t::CSPCL_ROUTE_ERR_PROVIDER_FAILED)?;

    Ok(Box::new(router))
}

pub(crate) fn init_state() -> Result<(), cspcl_route_error_t> {
    STATE.with(|state_cell| {
        let mut state_ref = state_cell.borrow_mut();
        if state_ref.is_some() {
            return Ok(());
        }

        let router = load_router()?;
        *state_ref = Some(AdapterState {
            router,
            next_hops: Vec::new(),
            diagnostic: CString::new("asabr-adapter-initialized").unwrap(),
        });
        Ok(())
    })
}

pub(crate) fn with_state_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut AdapterState) -> R,
{
    STATE.with(|state_cell| {
        let mut state_ref = state_cell.borrow_mut();
        let state = state_ref.as_mut().expect("adapter state initialized");
        f(state)
    })
}

pub(crate) fn reset_state() {
    STATE.with(|state_cell| {
        *state_cell.borrow_mut() = None;
    });
}
