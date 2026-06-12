use std::ffi::{CStr, c_char};

use crate::{
    csp_conn_t, cspcl_accept_conn, cspcl_addr_to_endpoint, cspcl_cleanup, cspcl_close_rx_socket,
    cspcl_endpoint_to_addr, cspcl_error_t, cspcl_init, cspcl_recv_bundle,
    cspcl_recv_bundle_from_conn, cspcl_send_bundle, cspcl_strerror, cspcl_t,
};

pub fn init(cspcl: &mut cspcl_t) -> cspcl_error_t {
    unsafe { cspcl_init(cspcl) }
}

pub fn cleanup(cspcl: &mut cspcl_t) {
    unsafe {
        cspcl_cleanup(cspcl);
    }
}

pub fn close_rx_socket(cspcl: &mut cspcl_t) {
    unsafe {
        cspcl_close_rx_socket(cspcl);
    }
}

pub fn send_bundle(
    cspcl: &mut cspcl_t,
    bundle: &[u8],
    dest_addr: u8,
    dest_port: u8,
) -> cspcl_error_t {
    unsafe { cspcl_send_bundle(cspcl, bundle.as_ptr(), bundle.len(), dest_addr, dest_port) }
}

pub fn recv_bundle(
    cspcl: &mut cspcl_t,
    buffer: &mut [u8],
    timeout_ms: u32,
) -> (cspcl_error_t, usize, u8, u8) {
    let mut len = buffer.len();
    let mut src_addr = 0;
    let mut src_port = 0;
    let code = unsafe {
        cspcl_recv_bundle(
            cspcl,
            buffer.as_mut_ptr(),
            &mut len,
            &mut src_addr,
            &mut src_port,
            timeout_ms,
        )
    };
    (code, len, src_addr, src_port)
}

pub fn accept_conn(
    cspcl: &mut cspcl_t,
    timeout_ms: u32,
) -> (cspcl_error_t, *mut csp_conn_t, u8, u8) {
    let mut conn = std::ptr::null_mut();
    let mut src_addr = 0;
    let mut src_port = 0;
    let code =
        unsafe { cspcl_accept_conn(cspcl, &mut conn, &mut src_addr, &mut src_port, timeout_ms) };
    (code, conn, src_addr, src_port)
}

pub fn recv_bundle_from_conn(
    conn: *mut csp_conn_t,
    buffer: &mut [u8],
    pkt_src_addr: u8,
    pkt_src_port: u8,
) -> (cspcl_error_t, usize, u8, u8) {
    let mut len = buffer.len();
    let mut src_addr = 0;
    let mut src_port = 0;
    let code = unsafe {
        cspcl_recv_bundle_from_conn(
            conn,
            buffer.as_mut_ptr(),
            &mut len,
            &mut src_addr,
            &mut src_port,
            pkt_src_addr,
            pkt_src_port,
        )
    };
    (code, len, src_addr, src_port)
}

pub fn endpoint_to_addr(endpoint: &CStr) -> u8 {
    unsafe { cspcl_endpoint_to_addr(endpoint.as_ptr()) }
}

pub fn addr_to_endpoint(addr: u8, buffer: &mut [u8]) -> cspcl_error_t {
    unsafe { cspcl_addr_to_endpoint(addr, buffer.as_mut_ptr() as *mut c_char, buffer.len()) }
}

pub fn error_message(code: cspcl_error_t) -> &'static str {
    unsafe {
        CStr::from_ptr(cspcl_strerror(code))
            .to_str()
            .unwrap_or("Unknown error")
    }
}
