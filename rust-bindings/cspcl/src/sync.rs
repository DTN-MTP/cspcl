//! Locking that ignores poisoning: the state guarded in this crate stays
//! consistent even if a holder panicked.

use std::{
    sync::{Condvar, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// [`Condvar::wait_while`], ignoring poisoning.
pub(crate) fn wait_while<'a, T>(
    condvar: &Condvar,
    guard: MutexGuard<'a, T>,
    condition: impl FnMut(&mut T) -> bool,
) -> MutexGuard<'a, T> {
    condvar
        .wait_while(guard, condition)
        .unwrap_or_else(PoisonError::into_inner)
}

/// [`Condvar::wait_timeout`], ignoring poisoning and whether it timed out.
pub(crate) fn wait_timeout<'a, T>(
    condvar: &Condvar,
    guard: MutexGuard<'a, T>,
    timeout: Duration,
) -> MutexGuard<'a, T> {
    condvar
        .wait_timeout(guard, timeout)
        .unwrap_or_else(PoisonError::into_inner)
        .0
}

/// [`Condvar::wait_timeout_while`], ignoring poisoning and whether it timed
/// out.
pub(crate) fn wait_timeout_while<'a, T>(
    condvar: &Condvar,
    guard: MutexGuard<'a, T>,
    timeout: Duration,
    condition: impl FnMut(&mut T) -> bool,
) -> MutexGuard<'a, T> {
    condvar
        .wait_timeout_while(guard, timeout, condition)
        .unwrap_or_else(PoisonError::into_inner)
        .0
}
