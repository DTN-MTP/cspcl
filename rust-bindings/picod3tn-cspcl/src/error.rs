//! What a failed send means for the bundle.

/// Largest bundle cspcl sends or receives (`CSPCL_MAX_BUNDLE_SIZE`).
pub const MAX_BUNDLE_SIZE: usize = 65535;

/// How a send failure should be handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// Worth retrying later: defer the bundle.
    Retryable,
    /// Never succeeds as is: retrying it is pointless.
    Permanent,
    /// The runtime or the link is shutting down.
    Cancelled,
}

/// A bundle of `len` bytes is a [`FailureKind::Permanent`] failure if it is
/// over [`MAX_BUNDLE_SIZE`]: cspcl would refuse it on every retry.
pub const fn check_size(len: usize) -> Result<(), FailureKind> {
    if len > MAX_BUNDLE_SIZE {
        Err(FailureKind::Permanent)
    } else {
        Ok(())
    }
}

#[cfg(feature = "tokio")]
mod send {
    use super::FailureKind;

    impl FailureKind {
        /// Classifies a cspcl error.
        ///
        /// Everything not known to be permanent or a cancellation is
        /// retryable, like C's `cspcl_error_is_retryable` for the transport
        /// errors. A peer that is unreachable or restarted fails with
        /// `Timeout`, then `CspSend` or `Connection` until it is back, and is
        /// reached again once it is.
        pub fn of(error: &cspcl::Error) -> Self {
            use cspcl::Error::*;
            match error {
                BundleTooLarge | InvalidParam => Self::Permanent,
                Cancelled | NotInitialized => Self::Cancelled,
                _ => Self::Retryable,
            }
        }
    }

    /// Why [`CspclLink::send`](crate::CspclLink::send) failed.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
    pub enum SendError {
        /// Worth retrying later: defer the bundle.
        #[error("retryable cspcl send failure: {0}")]
        Retryable(#[source] cspcl::Error),
        /// Never succeeds as is (e.g. a bundle over
        /// [`MAX_BUNDLE_SIZE`](super::MAX_BUNDLE_SIZE)): retrying it is
        /// pointless.
        #[error("permanent cspcl send failure: {0}")]
        Permanent(#[source] cspcl::Error),
        /// The runtime or the link is shutting down. The bundle may still
        /// have been delivered, if its send had already started.
        #[error("cspcl send cancelled")]
        Cancelled,
    }

    impl SendError {
        /// How this failure should be handled.
        pub fn kind(&self) -> FailureKind {
            match self {
                Self::Retryable(_) => FailureKind::Retryable,
                Self::Permanent(_) => FailureKind::Permanent,
                Self::Cancelled => FailureKind::Cancelled,
            }
        }
    }

    impl From<cspcl::Error> for SendError {
        fn from(error: cspcl::Error) -> Self {
            match FailureKind::of(&error) {
                FailureKind::Retryable => Self::Retryable(error),
                FailureKind::Permanent => Self::Permanent(error),
                FailureKind::Cancelled => Self::Cancelled,
            }
        }
    }
}

#[cfg(feature = "tokio")]
pub use send::SendError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_size_limit() {
        assert_eq!(check_size(0), Ok(()));
        assert_eq!(check_size(MAX_BUNDLE_SIZE), Ok(()));
        assert_eq!(check_size(MAX_BUNDLE_SIZE + 1), Err(FailureKind::Permanent));
    }

    #[test]
    fn max_bundle_size_matches_c() {
        assert_eq!(MAX_BUNDLE_SIZE, cspcl_sys::CSPCL_MAX_BUNDLE_SIZE as usize);
    }

    #[cfg(feature = "tokio")]
    mod classification {
        use super::super::*;
        use cspcl::Error;

        #[test]
        fn permanent_errors() {
            for error in [Error::BundleTooLarge, Error::InvalidParam] {
                assert_eq!(FailureKind::of(&error), FailureKind::Permanent);
                assert_eq!(SendError::from(error), SendError::Permanent(error));
            }
        }

        #[test]
        fn cancellations() {
            for error in [Error::Cancelled, Error::NotInitialized] {
                assert_eq!(FailureKind::of(&error), FailureKind::Cancelled);
                assert_eq!(SendError::from(error), SendError::Cancelled);
            }
        }

        #[test]
        fn retryable_errors() {
            for error in [
                Error::Timeout,
                Error::CspSend,
                Error::CspRecv,
                Error::Connection,
                Error::PoolFull,
                Error::Sfp,
                Error::NoMemory,
                Error::UnknownError(999),
            ] {
                assert_eq!(FailureKind::of(&error), FailureKind::Retryable);
                assert_eq!(SendError::from(error), SendError::Retryable(error));
            }
        }

        #[test]
        fn kind_matches_variant() {
            assert_eq!(
                SendError::Retryable(Error::Timeout).kind(),
                FailureKind::Retryable
            );
            assert_eq!(
                SendError::Permanent(Error::BundleTooLarge).kind(),
                FailureKind::Permanent
            );
            assert_eq!(SendError::Cancelled.kind(), FailureKind::Cancelled);
        }
    }
}
