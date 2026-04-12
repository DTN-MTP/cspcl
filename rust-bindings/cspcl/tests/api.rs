use std::sync::{Mutex, MutexGuard};

use cspcl::{
    Cspcl, CspclConfig, Error, Interface, InterfaceName, addr_to_endpoint, endpoint_to_addr,
};

static TEST_GUARD: Mutex<()> = Mutex::new(());

fn test_lock() -> MutexGuard<'static, ()> {
    TEST_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn loopback_interface() -> Interface {
    Interface::Loopback(InterfaceName::new("loopback"))
}

fn test_instance() -> Cspcl {
    Cspcl::from_config(CspclConfig::new(7).with_interface(loopback_interface()))
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

#[test]
fn endpoint_helpers_cover_valid_and_invalid_inputs() {
    let _guard = test_lock();

    assert_eq!(endpoint_to_addr("ipn:1.0"), Some(1));
    assert_eq!(endpoint_to_addr("dtn://node7/sink"), Some(7));
    assert_eq!(endpoint_to_addr("invalid"), None);
    assert_eq!(endpoint_to_addr("ipn:\0bad"), None);

    assert_eq!(addr_to_endpoint(42).unwrap(), "ipn:42.0");
}

#[test]
fn config_defaults_and_constructors_match() {
    let _guard = test_lock();

    let default_config = CspclConfig::new(9);
    let from_config = Cspcl::from_config(default_config.clone()).unwrap();
    let from_new = Cspcl::new(
        9,
        cspcl::cspcl_sys::CSPCL_PORT_BP as u8,
        Interface::default(),
    )
    .unwrap();

    assert_eq!(from_config.local_addr(), 9);
    assert_eq!(
        from_config.local_port(),
        cspcl::cspcl_sys::CSPCL_PORT_BP as u8
    );
    assert!(from_config.is_initialized());
    assert_eq!(from_new.local_addr(), from_config.local_addr());
    assert_eq!(from_new.local_port(), from_config.local_port());
}

#[test]
fn shutdown_is_explicit_and_idempotent() {
    let _guard = test_lock();
    let cspcl = test_instance();

    cspcl.shutdown().unwrap();
    cspcl.shutdown().unwrap();

    assert!(!cspcl.is_initialized());
}

#[test]
fn send_and_receive_fail_after_shutdown() {
    let _guard = test_lock();
    let cspcl = test_instance();
    let sender = cspcl.sender();
    let receiver = cspcl.receiver();

    cspcl.shutdown().unwrap();

    assert_not_initialized(cspcl.send_bundle(&[1, 2, 3], 12, 10).unwrap_err());
    assert_not_initialized(sender.send_bundle(&[1, 2, 3], 12, 10).unwrap_err());
    assert_not_initialized(cspcl.recv_bundle(5).unwrap_err());
    assert_not_initialized(receiver.recv_bundle(5).unwrap_err());
}

#[test]
fn split_handles_can_send_without_mutable_access() {
    let _guard = test_lock();
    let cspcl = test_instance();
    let (sender, _receiver) = cspcl.split();
    let payload = vec![1_u8, 2, 3, 4, 5];

    sender.send_bundle(&payload, 12, 10).unwrap();
    assert_eq!(cspcl.local_addr(), 7);
}

#[test]
fn convenience_send_method_matches_split_handle_send() {
    let _guard = test_lock();
    let cspcl = test_instance();
    let payload = vec![9_u8, 8, 7];

    cspcl.send_bundle(&payload, 21, 10).unwrap();
    assert!(cspcl.is_initialized());
}

#[test]
fn empty_bundle_is_rejected() {
    let _guard = test_lock();
    let cspcl = test_instance();
    let err = cspcl.send_bundle(&[], 4, 10).unwrap_err();

    assert_eq!(
        err.code(),
        cspcl::cspcl_sys::cspcl_error_t_CSPCL_ERR_INVALID_PARAM
    );
}

#[test]
fn recv_times_out_when_no_bundle_is_pending() {
    let _guard = test_lock();
    let cspcl = test_instance();
    let err = cspcl.recv_bundle(5).unwrap_err();

    assert_timeout(err);
}

#[test]
fn cloned_sender_handles_can_send_sequentially() {
    let _guard = test_lock();
    let cspcl = test_instance();
    let sender = cspcl.sender();
    let sender_clone = sender.clone();

    sender.send_bundle(&[1, 2, 3], 40, 10).unwrap();
    sender_clone.send_bundle(&[4, 5], 41, 10).unwrap();
}

#[test]
fn cloned_receivers_time_out_sequentially() {
    let _guard = test_lock();
    let cspcl = test_instance();
    let receiver = cspcl.receiver();
    let receiver_clone = receiver.clone();

    assert_timeout(receiver.recv_bundle(5).unwrap_err());
    assert_timeout(receiver_clone.recv_bundle(5).unwrap_err());
}
