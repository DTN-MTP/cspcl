use crate::cspcl_sys;

/// Get current time in milliseconds
///
/// # Returns
/// Current time in milliseconds since some reference point (implementation-dependent)
pub fn get_time_ms() -> u64 {
    unsafe { cspcl_sys::cspcl_get_time_ms() }
}

// TODO: Add helper functions for timeout calculation if needed
// pub fn elapsed_ms(start_time: u64) -> u64 { get_time_ms() - start_time }
