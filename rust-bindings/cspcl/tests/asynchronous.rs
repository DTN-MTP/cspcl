#![cfg(feature = "async-tokio")]

use std::sync::{Mutex, MutexGuard};

use cspcl::asynchronous;
use cspcl::{Cspcl, CspclConfig, Error, Interface, InterfaceName};

static TEST_GUARD: Mutex<()> = Mutex::new(());

fn test_lock() -> MutexGuard<'static, ()> {
    TEST_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn test_instance() -> Cspcl {
    Cspcl::from_config(
        CspclConfig::new(7).with_interface(Interface::Loopback(InterfaceName::new("loopback"))),
    )
    .expect("failed to initialize test cspcl instance")
}

fn assert_timeout(err: Error) {
    assert_eq!(
        err.code(),
        cspcl::cspcl_sys::cspcl_error_t_CSPCL_ERR_TIMEOUT
    );
}

fn assert_not_initialized(err: Error) {
    assert_eq!(
        err.code(),
        cspcl::cspcl_sys::cspcl_error_t_CSPCL_ERR_NOT_INITIALIZED
    );
}

fn assert_invalid_param(err: Error) {
    assert_eq!(
        err.code(),
        cspcl::cspcl_sys::cspcl_error_t_CSPCL_ERR_INVALID_PARAM
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn async_wrappers_can_be_constructed_from_sync_runtime() {
    let _guard = test_lock();
    let cspcl = test_instance();
    let async_cspcl = asynchronous::Cspcl::from_sync(cspcl.clone());
    let (_sender, _receiver) = async_cspcl.split();

    assert!(async_cspcl.is_initialized());
    assert_eq!(async_cspcl.local_addr(), cspcl.local_addr());
    assert_eq!(async_cspcl.connection_stats(), cspcl.connection_stats());
}

#[tokio::test(flavor = "multi_thread")]
async fn async_receiver_times_out_without_pending_bundle() {
    let _guard = test_lock();
    let cspcl = test_instance();
    let async_receiver = asynchronous::Cspcl::from_sync(cspcl).receiver();

    assert_timeout(async_receiver.recv_bundle(5).await.unwrap_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn async_receiver_into_times_out_without_pending_bundle() {
    let _guard = test_lock();
    let cspcl = test_instance();
    let async_receiver = asynchronous::Cspcl::from_sync(cspcl).receiver();
    let mut buffer = [0_u8; 64];

    assert_timeout(
        async_receiver
            .recv_bundle_into(&mut buffer, 5)
            .await
            .unwrap_err(),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn async_receive_observes_sync_shutdown() {
    let _guard = test_lock();
    let cspcl = test_instance();
    let async_receiver = asynchronous::Cspcl::from_sync(cspcl.clone()).receiver();
    let mut buffer = [0_u8; 64];

    cspcl.shutdown().unwrap();

    assert_not_initialized(async_receiver.recv_bundle(5).await.unwrap_err());
    assert_not_initialized(
        async_receiver
            .recv_bundle_into(&mut buffer, 5)
            .await
            .unwrap_err(),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn async_sender_can_send_and_report_stats() {
    let _guard = test_lock();
    let cspcl = test_instance();
    let async_cspcl = asynchronous::Cspcl::from_sync(cspcl.clone());
    let sender = async_cspcl.sender();
    let sender_clone = sender.clone();

    sender.send_bundle(&[1, 2, 3], 42, 10).await.unwrap();
    sender_clone.send_bundle(&[4, 5, 6], 42, 10).await.unwrap();

    let stats = async_cspcl.connection_stats();
    assert!(stats.misses >= 1);
    assert!(stats.hits >= 1);
    assert_eq!(sender.connection_stats(), stats);
}

#[tokio::test(flavor = "multi_thread")]
async fn async_sender_rejects_invalid_input_and_shutdown() {
    let _guard = test_lock();
    let cspcl = test_instance();
    let async_cspcl = asynchronous::Cspcl::from_sync(cspcl.clone());
    let sender = async_cspcl.sender();

    assert_invalid_param(sender.send_bundle(&[], 42, 10).await.unwrap_err());

    async_cspcl.shutdown().await.unwrap();

    assert_not_initialized(sender.send_bundle(&[1, 2, 3], 42, 10).await.unwrap_err());
    assert!(!cspcl.is_initialized());
}
