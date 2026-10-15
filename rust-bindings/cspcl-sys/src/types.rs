use crate::{
    csp_conn_t, csp_iface_type_CSP_IFACE_CAN, csp_iface_type_CSP_IFACE_LOOPBACK,
    csp_iface_type_CSP_IFACE_ZMQHUB, cspcl_conn_pool_t, cspcl_error_t, cspcl_error_t_CSPCL_OK,
    cspcl_t, primitive,
};

pub type Result<T> = std::result::Result<T, cspcl_error_t>;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum InterfaceConfig {
    Zmq(String),
    Can(String),
    #[default]
    Loopback,
}

impl InterfaceConfig {
    pub fn zmq(interface_name: impl Into<String>) -> Self {
        Self::Zmq(interface_name.into())
    }

    pub fn can(interface_name: impl Into<String>) -> Self {
        Self::Can(interface_name.into())
    }

    pub fn loopback() -> Self {
        Self::Loopback
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CspclConfig {
    pub local_addr: u8,
    pub csp_port: u8,
    pub interface: InterfaceConfig,
}

/// An accepted inbound CSP connection.
///
/// The connection is owned by this value and closed when it is dropped.
///
/// It cannot be copied, so it cannot be read after it is closed:
///
/// ```compile_fail
/// use cspcl_sys::types::{AcceptedConn, recv_bundle_from_conn};
///
/// fn read_after_close(conn: AcceptedConn) {
///     let copy = conn;
///     drop(conn);
///     let _ = recv_bundle_from_conn(&copy);
/// }
/// ```
///
/// Nor can two threads receive on it at once:
///
/// ```compile_fail
/// use cspcl_sys::types::{AcceptedConn, recv_bundle_from_conn};
///
/// fn concurrent_reads(conn: &AcceptedConn) {
///     std::thread::scope(|s| {
///         s.spawn(|| recv_bundle_from_conn(conn));
///         let _ = recv_bundle_from_conn(conn);
///     });
/// }
/// ```
#[derive(Debug)]
pub struct AcceptedConn {
    conn: *mut csp_conn_t,
    pub src_addr: u8,
    pub src_port: u8,
}

impl AcceptedConn {
    /// The underlying CSP connection, still owned by `self`.
    pub fn as_ptr(&self) -> *mut csp_conn_t {
        self.conn
    }

    /// Release ownership of the connection without closing it.
    pub fn into_raw(self) -> *mut csp_conn_t {
        std::mem::ManuallyDrop::new(self).conn
    }
}

impl Drop for AcceptedConn {
    fn drop(&mut self) {
        // SAFETY: `conn` is a valid connection owned by this value.
        unsafe { primitive::close_conn(self.conn) }
    }
}

// SAFETY: the connection is owned by this value and may move between
// threads. It is not `Sync`, so only one thread can receive on it at a time.
unsafe impl Send for AcceptedConn {}

#[derive(Debug)]
pub struct ReceivedBundle {
    pub buffer: Vec<u8>,
    pub len: usize,
    pub src_addr: u8,
    pub src_port: u8,
}

impl ReceivedBundle {
    pub fn data(&self) -> &[u8] {
        &self.buffer[..self.len]
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.buffer[..self.len]
    }
}

fn ok_or_err(code: cspcl_error_t) -> Result<()> {
    if code == cspcl_error_t_CSPCL_OK {
        Ok(())
    } else {
        Err(code)
    }
}

fn copy_c_string<const N: usize>(dest: &mut [i8; N], src: &str) {
    *dest = [0; N];
    for (idx, byte) in src.bytes().take(N.saturating_sub(1)).enumerate() {
        if byte == 0 {
            break;
        }
        dest[idx] = byte as i8;
    }
}

pub fn configure(config: &CspclConfig) -> cspcl_t {
    let mut cspcl = cspcl_t {
        local_addr: config.local_addr,
        csp_port: config.csp_port,
        ..Default::default()
    };

    match &config.interface {
        InterfaceConfig::Zmq(interface_name) => {
            cspcl.iface_type = csp_iface_type_CSP_IFACE_ZMQHUB;
            copy_c_string(&mut cspcl.zmqhub_addr, interface_name);
        }
        InterfaceConfig::Can(interface_name) => {
            cspcl.iface_type = csp_iface_type_CSP_IFACE_CAN;
            copy_c_string(&mut cspcl.can_iface, interface_name);
        }
        InterfaceConfig::Loopback => {
            cspcl.iface_type = csp_iface_type_CSP_IFACE_LOOPBACK;
        }
    }

    cspcl
}

pub fn init_from_config(config: &CspclConfig) -> Result<cspcl_t> {
    let mut cspcl = configure(config);
    init(&mut cspcl)?;
    Ok(cspcl)
}

pub fn init(cspcl: &mut cspcl_t) -> Result<()> {
    ok_or_err(primitive::init(cspcl))
}

pub fn cleanup(cspcl: &mut cspcl_t) {
    primitive::cleanup(cspcl);
}

pub fn close_rx_socket(cspcl: &mut cspcl_t) {
    primitive::close_rx_socket(cspcl);
}

/// # Safety
/// See [`primitive::close_rx_socket_raw`].
pub unsafe fn close_rx_socket_raw(cspcl: *mut cspcl_t) {
    unsafe { primitive::close_rx_socket_raw(cspcl) }
}

pub fn send_bundle(cspcl: &mut cspcl_t, bundle: &[u8], dest_addr: u8, dest_port: u8) -> Result<()> {
    ok_or_err(primitive::send_bundle(cspcl, bundle, dest_addr, dest_port))
}

/// # Safety
/// See [`primitive::send_bundle_raw`].
pub unsafe fn send_bundle_raw(
    cspcl: *mut cspcl_t,
    bundle: &[u8],
    dest_addr: u8,
    dest_port: u8,
) -> Result<()> {
    ok_or_err(unsafe { primitive::send_bundle_raw(cspcl, bundle, dest_addr, dest_port) })
}

pub fn recv_bundle(
    cspcl: &mut cspcl_t,
    buffer: &mut [u8],
    timeout_ms: u32,
) -> Result<ReceivedBundle> {
    let (code, len, src_addr, src_port) = primitive::recv_bundle(cspcl, buffer, timeout_ms);
    ok_or_err(code)?;

    Ok(ReceivedBundle {
        buffer: buffer.to_vec(),
        len,
        src_addr,
        src_port,
    })
}

pub fn accept_conn(cspcl: &mut cspcl_t, timeout_ms: u32) -> Result<AcceptedConn> {
    unsafe { accept_conn_raw(cspcl, timeout_ms) }
}

/// # Safety
/// See [`primitive::accept_conn_raw`].
pub unsafe fn accept_conn_raw(cspcl: *mut cspcl_t, timeout_ms: u32) -> Result<AcceptedConn> {
    let (code, conn, src_addr, src_port) = unsafe { primitive::accept_conn_raw(cspcl, timeout_ms) };
    ok_or_err(code)?;

    Ok(AcceptedConn {
        conn,
        src_addr,
        src_port,
    })
}

/// # Safety
/// See [`primitive::conn_pool_add`].
pub unsafe fn conn_pool_add(
    pool: &mut cspcl_conn_pool_t,
    dest_addr: u8,
    dest_port: u8,
    conn: *mut csp_conn_t,
) -> Result<()> {
    ok_or_err(unsafe { primitive::conn_pool_add(pool, dest_addr, dest_port, conn) })
}

pub fn conn_pool_add_accepted(cspcl: &mut cspcl_t, accepted: AcceptedConn) -> Result<()> {
    let (src_addr, src_port) = (accepted.src_addr, accepted.src_port);
    let conn = accepted.into_raw();
    // SAFETY: ownership of `conn` passes to the pool on success.
    let result = unsafe { conn_pool_add(&mut cspcl.conn_pool, src_addr, src_port, conn) };
    if result.is_err() {
        // The pool did not take the connection; close it.
        unsafe { primitive::close_conn(conn) }
    }
    result
}

pub fn recv_bundle_from_conn(accepted: &AcceptedConn) -> Result<ReceivedBundle> {
    let mut buffer = vec![0u8; crate::CSPCL_MAX_BUNDLE_SIZE as usize];
    let (code, len, src_addr, src_port) = unsafe {
        // SAFETY: `accepted` owns an open connection, and `&AcceptedConn` is not
        // `Send`, so no other thread receives on it concurrently.
        primitive::recv_bundle_from_conn(
            accepted.conn,
            &mut buffer,
            accepted.src_addr,
            accepted.src_port,
        )
    };
    ok_or_err(code)?;
    buffer.truncate(len);

    Ok(ReceivedBundle {
        buffer,
        len,
        src_addr,
        src_port,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        csp_iface_type_CSP_IFACE_CAN, csp_iface_type_CSP_IFACE_LOOPBACK,
        csp_iface_type_CSP_IFACE_ZMQHUB,
    };

    #[test]
    fn configure_sets_basic_fields() {
        let config = CspclConfig {
            local_addr: 4,
            csp_port: 10,
            interface: InterfaceConfig::Loopback,
        };

        let cspcl = configure(&config);

        assert_eq!(cspcl.local_addr, 4);
        assert_eq!(cspcl.csp_port, 10);
        assert_eq!(cspcl.iface_type, csp_iface_type_CSP_IFACE_LOOPBACK);
    }

    #[test]
    fn configure_sets_interface_name() {
        let config = CspclConfig {
            local_addr: 1,
            csp_port: 10,
            interface: InterfaceConfig::Zmq("localhost".to_string()),
        };

        let cspcl = configure(&config);

        assert_eq!(cspcl.iface_type, csp_iface_type_CSP_IFACE_ZMQHUB);
        assert_eq!(cspcl.zmqhub_addr[0], b'l' as i8);
        assert_eq!(cspcl.zmqhub_addr[9], 0);

        let config = CspclConfig {
            local_addr: 1,
            csp_port: 10,
            interface: InterfaceConfig::Can("vcan0".to_string()),
        };

        let cspcl = configure(&config);

        assert_eq!(cspcl.iface_type, csp_iface_type_CSP_IFACE_CAN);
        assert_eq!(cspcl.can_iface[0], b'v' as i8);
        assert_eq!(cspcl.can_iface[5], 0);
    }
}
