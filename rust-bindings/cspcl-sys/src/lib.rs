#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unsafe_op_in_unsafe_fn)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

unsafe impl Send for cspcl_t {}
unsafe impl Sync for cspcl_t {}

pub mod primitive;
pub mod types;
