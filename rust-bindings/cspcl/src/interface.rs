use std::{ffi::CString, ops::Deref, os::raw::c_uint};

use cspcl_sys::{
    csp_iface_type_CSP_IFACE_CAN, csp_iface_type_CSP_IFACE_LOOPBACK,
    csp_iface_type_CSP_IFACE_ZMQHUB,
};

#[derive(Debug, Clone)]
pub struct InterfaceName(String);

impl InterfaceName {
    pub fn new(name: &str) -> Self {
        Self(name.to_string())
    }
}

impl Deref for InterfaceName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for InterfaceName {
    fn default() -> Self {
        Self("loopback".to_string())
    }
}

impl From<String> for InterfaceName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone)]
pub enum Interface {
    Zmq(InterfaceName),
    Can(InterfaceName),
    Loopback(InterfaceName),
}

impl Deref for Interface {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        let interface_name = match self {
            Interface::Zmq(interface_name) => interface_name,
            Interface::Can(interface_name) => interface_name,
            Interface::Loopback(interface_name) => interface_name,
        };

        &interface_name
    }
}

impl Default for Interface {
    fn default() -> Self {
        Self::Loopback(InterfaceName::default())
    }
}

impl From<Interface> for c_uint {
    fn from(val: Interface) -> Self {
        match val {
            Interface::Zmq(_) => csp_iface_type_CSP_IFACE_ZMQHUB,
            Interface::Can(_) => csp_iface_type_CSP_IFACE_CAN,
            Interface::Loopback(_) => csp_iface_type_CSP_IFACE_LOOPBACK,
        }
    }
}

impl Into<[i8; 64]> for InterfaceName {
    fn into(self) -> [i8; 64] {
        let interface_name: &str = &self;
        let cstr = CString::new(interface_name).expect("string contains null byte");

        let bytes = cstr.as_bytes_with_nul();
        let mut arr = [0i8; 64];
        for (i, &b) in bytes.iter().enumerate().take(64) {
            arr[i] = b as i8;
        }
        arr
    }
}
