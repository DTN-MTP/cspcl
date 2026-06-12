use cspcl_sys::types::Result as SysResult;
use std::fmt;

/// Rust wrapper for CSPCL errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Invalid parameter
    InvalidParam,
    /// Memory allocation failed
    NoMemory,
    /// Bundle exceeds maximum size
    BundleTooLarge,
    /// CSP send failed
    CspSend,
    /// CSP receive failed
    CspRecv,
    /// Operation timed out
    Timeout,
    /// SFP fragmentation/reassembly error
    Sfp,
    /// CSPCL not initialized
    NotInitialized,
    /// CSP connection error
    Connection,
    /// CSP init error
    CspInit,
    /// csp_init() failed
    CspStackInit,
    /// ZMQ hub interface init failed
    CspZmqhubInit,
    /// CAN interface init failed
    CspCanInit,
    /// CAN support not compiled in
    CspCanNotSupported,
    /// CSP router task start failed
    CspRouter,
    /// Connection pool full, LRU eviction was forced
    PoolFull,
    /// Error code returned by CSPCL that this crate does not know yet
    UnknownError(cspcl_sys::cspcl_error_t),
}

impl Error {
    /// Create error from a raw CSPCL error code.
    pub fn from_raw(code: cspcl_sys::cspcl_error_t) -> Self {
        match code {
            cspcl_sys::cspcl_error_t_CSPCL_ERR_INVALID_PARAM => Self::InvalidParam,
            cspcl_sys::cspcl_error_t_CSPCL_ERR_NO_MEMORY => Self::NoMemory,
            cspcl_sys::cspcl_error_t_CSPCL_ERR_BUNDLE_TOO_LARGE => Self::BundleTooLarge,
            cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_SEND => Self::CspSend,
            cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_RECV => Self::CspRecv,
            cspcl_sys::cspcl_error_t_CSPCL_ERR_TIMEOUT => Self::Timeout,
            cspcl_sys::cspcl_error_t_CSPCL_ERR_SFP => Self::Sfp,
            cspcl_sys::cspcl_error_t_CSPCL_ERR_NOT_INITIALIZED => Self::NotInitialized,
            cspcl_sys::cspcl_error_t_CSPCL_ERR_CONNECTION => Self::Connection,
            cspcl_sys::cspcl_error_t_CSPCL_ERR_CSPINIT => Self::CspInit,
            cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_STACK_INIT => Self::CspStackInit,
            cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_ZMQHUB_INIT => Self::CspZmqhubInit,
            cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_CAN_INIT => Self::CspCanInit,
            cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_CAN_NOT_SUPPORTED => Self::CspCanNotSupported,
            cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_ROUTER => Self::CspRouter,
            cspcl_sys::cspcl_error_t_CSPCL_ERR_POOL_FULL => Self::PoolFull,
            unknown => Self::UnknownError(unknown),
        }
    }

    /// Create error from error code
    pub fn from_code(code: cspcl_sys::cspcl_error_t) -> std::result::Result<(), Self> {
        if code == cspcl_sys::cspcl_error_t_CSPCL_OK {
            Ok(())
        } else {
            Err(Self::from_raw(code))
        }
    }

    /// Get human-readable error message
    pub fn message(&self) -> &'static str {
        match self {
            Self::InvalidParam => "Invalid parameter",
            Self::NoMemory => "Memory allocation failed",
            Self::BundleTooLarge => "Bundle too large",
            Self::CspSend => "CSP send failed",
            Self::CspRecv => "CSP receive failed",
            Self::Timeout => "Operation timed out",
            Self::Sfp => "SFP fragmentation/reassembly error",
            Self::NotInitialized => "CSPCL not initialized",
            Self::Connection => "CSP connection error",
            Self::CspInit => "CSP init error",
            Self::CspStackInit => "CSP stack init failed",
            Self::CspZmqhubInit => "ZMQ hub interface init failed",
            Self::CspCanInit => "CAN interface init failed",
            Self::CspCanNotSupported => "CAN support not compiled in",
            Self::CspRouter => "CSP router task start failed",
            Self::PoolFull => "Connection pool full",
            Self::UnknownError(_) => "Unknown error",
        }
    }

    /// Get the raw error code
    pub fn code(&self) -> cspcl_sys::cspcl_error_t {
        match self {
            Self::InvalidParam => cspcl_sys::cspcl_error_t_CSPCL_ERR_INVALID_PARAM,
            Self::NoMemory => cspcl_sys::cspcl_error_t_CSPCL_ERR_NO_MEMORY,
            Self::BundleTooLarge => cspcl_sys::cspcl_error_t_CSPCL_ERR_BUNDLE_TOO_LARGE,
            Self::CspSend => cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_SEND,
            Self::CspRecv => cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_RECV,
            Self::Timeout => cspcl_sys::cspcl_error_t_CSPCL_ERR_TIMEOUT,
            Self::Sfp => cspcl_sys::cspcl_error_t_CSPCL_ERR_SFP,
            Self::NotInitialized => cspcl_sys::cspcl_error_t_CSPCL_ERR_NOT_INITIALIZED,
            Self::Connection => cspcl_sys::cspcl_error_t_CSPCL_ERR_CONNECTION,
            Self::CspInit => cspcl_sys::cspcl_error_t_CSPCL_ERR_CSPINIT,
            Self::CspStackInit => cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_STACK_INIT,
            Self::CspZmqhubInit => cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_ZMQHUB_INIT,
            Self::CspCanInit => cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_CAN_INIT,
            Self::CspCanNotSupported => cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_CAN_NOT_SUPPORTED,
            Self::CspRouter => cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_ROUTER,
            Self::PoolFull => cspcl_sys::cspcl_error_t_CSPCL_ERR_POOL_FULL,
            Self::UnknownError(code) => *code,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CSPCL Error: {}", self.message())
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn from_sys_result<T>(value: SysResult<T>) -> Result<T> {
    value.map_err(Error::from_raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_sys_errors_to_rust_variants() {
        assert_eq!(
            Error::from_raw(cspcl_sys::cspcl_error_t_CSPCL_ERR_INVALID_PARAM),
            Error::InvalidParam
        );
        assert_eq!(
            Error::from_raw(cspcl_sys::cspcl_error_t_CSPCL_ERR_NO_MEMORY),
            Error::NoMemory
        );
        assert_eq!(
            Error::from_raw(cspcl_sys::cspcl_error_t_CSPCL_ERR_BUNDLE_TOO_LARGE),
            Error::BundleTooLarge
        );
        assert_eq!(
            Error::from_raw(cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_SEND),
            Error::CspSend
        );
        assert_eq!(
            Error::from_raw(cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_RECV),
            Error::CspRecv
        );
        assert_eq!(
            Error::from_raw(cspcl_sys::cspcl_error_t_CSPCL_ERR_TIMEOUT),
            Error::Timeout
        );
        assert_eq!(
            Error::from_raw(cspcl_sys::cspcl_error_t_CSPCL_ERR_SFP),
            Error::Sfp
        );
        assert_eq!(
            Error::from_raw(cspcl_sys::cspcl_error_t_CSPCL_ERR_NOT_INITIALIZED),
            Error::NotInitialized
        );
        assert_eq!(
            Error::from_raw(cspcl_sys::cspcl_error_t_CSPCL_ERR_CONNECTION),
            Error::Connection
        );
        assert_eq!(
            Error::from_raw(cspcl_sys::cspcl_error_t_CSPCL_ERR_CSPINIT),
            Error::CspInit
        );
        assert_eq!(
            Error::from_raw(cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_STACK_INIT),
            Error::CspStackInit
        );
        assert_eq!(
            Error::from_raw(cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_ZMQHUB_INIT),
            Error::CspZmqhubInit
        );
        assert_eq!(
            Error::from_raw(cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_CAN_INIT),
            Error::CspCanInit
        );
        assert_eq!(
            Error::from_raw(cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_CAN_NOT_SUPPORTED),
            Error::CspCanNotSupported
        );
        assert_eq!(
            Error::from_raw(cspcl_sys::cspcl_error_t_CSPCL_ERR_CSP_ROUTER),
            Error::CspRouter
        );
        assert_eq!(
            Error::from_raw(cspcl_sys::cspcl_error_t_CSPCL_ERR_POOL_FULL),
            Error::PoolFull
        );
    }

    #[test]
    fn maps_ok_code_to_success() {
        assert_eq!(Error::from_code(cspcl_sys::cspcl_error_t_CSPCL_OK), Ok(()));
    }

    #[test]
    fn preserves_unknown_sys_error_code() {
        let code = 999 as cspcl_sys::cspcl_error_t;

        assert_eq!(Error::from_raw(code), Error::UnknownError(code));
        assert_eq!(Error::from_raw(code).code(), code);
    }
}
