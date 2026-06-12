use crate::{
    csp_conn_t, csp_iface_type_CSP_IFACE_CAN, csp_iface_type_CSP_IFACE_LOOPBACK,
    csp_iface_type_CSP_IFACE_ZMQHUB, cspcl_conn_pool_t, cspcl_error_t, cspcl_error_t_CSPCL_OK,
    cspcl_t, primitive,
};

pub type Result<T> = std::result::Result<T, cspcl_error_t>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterfaceConfig {
    Zmq(String),
    Can(String),
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

impl Default for InterfaceConfig {
    fn default() -> Self {
        Self::Loopback
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CspclConfig {
    pub local_addr: u8,
    pub csp_port: u8,
    pub interface: InterfaceConfig,
}

#[derive(Debug, Clone, Copy)]
pub struct AcceptedConn {
    pub conn: *mut csp_conn_t,
    pub src_addr: u8,
    pub src_port: u8,
}

#[derive(Debug)]
pub struct ReceivedBundle<'a> {
    pub buffer: &'a mut [u8],
    pub len: usize,
    pub src_addr: u8,
    pub src_port: u8,
}

impl<'a> ReceivedBundle<'a> {
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

pub fn send_bundle(cspcl: &mut cspcl_t, bundle: &[u8], dest_addr: u8, dest_port: u8) -> Result<()> {
    ok_or_err(primitive::send_bundle(cspcl, bundle, dest_addr, dest_port))
}

pub fn recv_bundle<'a>(
    cspcl: &mut cspcl_t,
    buffer: &'a mut [u8],
    timeout_ms: u32,
) -> Result<ReceivedBundle<'a>> {
    let (code, len, src_addr, src_port) = primitive::recv_bundle(cspcl, buffer, timeout_ms);
    ok_or_err(code)?;

    Ok(ReceivedBundle {
        buffer,
        len,
        src_addr,
        src_port,
    })
}

pub fn accept_conn(cspcl: &mut cspcl_t, timeout_ms: u32) -> Result<AcceptedConn> {
    let (code, conn, src_addr, src_port) = primitive::accept_conn(cspcl, timeout_ms);
    ok_or_err(code)?;

    Ok(AcceptedConn {
        conn,
        src_addr,
        src_port,
    })
}

pub fn conn_pool_add(
    pool: &mut cspcl_conn_pool_t,
    dest_addr: u8,
    dest_port: u8,
    conn: *mut csp_conn_t,
) -> Result<()> {
    ok_or_err(primitive::conn_pool_add(pool, dest_addr, dest_port, conn))
}

pub fn conn_pool_add_accepted(cspcl: &mut cspcl_t, accepted: AcceptedConn) -> Result<()> {
    conn_pool_add(
        &mut cspcl.conn_pool,
        accepted.src_addr,
        accepted.src_port,
        accepted.conn,
    )
}

pub fn recv_bundle_from_conn<'a>(
    accepted: AcceptedConn,
    buffer: &'a mut [u8],
) -> Result<ReceivedBundle<'a>> {
    let (code, len, src_addr, src_port) = primitive::recv_bundle_from_conn(
        accepted.conn,
        buffer,
        accepted.src_addr,
        accepted.src_port,
    );
    ok_or_err(code)?;

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
