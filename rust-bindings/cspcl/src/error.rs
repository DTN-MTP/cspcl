use crate::cspcl_sys;
use std::ffi::CStr;
use std::fmt;

/// Rust wrapper for CSPCL errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error(cspcl_sys::cspcl_error_t);

impl Error {
    /// Create error from error code
    pub fn from_code(code: cspcl_sys::cspcl_error_t) -> std::result::Result<(), Self> {
        if code == cspcl_sys::cspcl_error_t_CSPCL_OK {
            Ok(())
        } else {
            Err(Error(code))
        }
    }

    /// Get human-readable error message
    pub fn message(&self) -> &'static str {
        unsafe {
            CStr::from_ptr(cspcl_sys::cspcl_strerror(self.0))
                .to_str()
                .unwrap_or("Unknown error")
        }
    }

    /// Get the raw error code
    pub fn code(&self) -> cspcl_sys::cspcl_error_t {
        self.0
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CSPCL Error: {}", self.message())
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
