use recastpress_wp::{
    dispatcher::{Dispatcher, DispatcherCallback},
    hooks::{CallbackId, HookError},
    sanitize_key,
    value::{PhpArray, PhpKey, PhpValue},
    wp_checkdate,
};

fn id() -> CallbackId {
    CallbackId {
        module: "test".into(),
        callback: "filter".into(),
    }
}

#[test]
fn unregistered_filters_keep_defaults_and_request_counters() {
    let mut dispatcher = Dispatcher::new();
    assert_eq!(
        sanitize_key(&mut dispatcher, b"RAW_!\0\xff"),
        Ok(PhpValue::Bytes(b"raw_".to_vec()))
    );
    assert_eq!(
        wp_checkdate(&mut dispatcher, 2, 29, 2024, b"not parsed"),
        Ok(PhpValue::Bool(true))
    );
    assert_eq!(
        wp_checkdate(&mut dispatcher, 2, 29, 1900, b"not parsed"),
        Ok(PhpValue::Bool(false))
    );
    assert_eq!(dispatcher.did_filter("sanitize_key"), 1);
    assert_eq!(dispatcher.did_filter("wp_checkdate"), 2);
    assert_eq!(dispatcher.current_hook(), None);
}

#[test]
fn named_filters_keep_raw_arguments_and_typed_returns() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.add(
        "sanitize_key",
        10,
        DispatcherCallback::new(id(), 2, |context, args| {
            assert_eq!(context.current_hook(), Some("sanitize_key"));
            assert_eq!(
                args,
                &[
                    PhpValue::Bytes(b"raw_".to_vec()),
                    PhpValue::Bytes(b"RAW_!\0\xff".to_vec())
                ]
            );
            Ok(PhpValue::Null)
        }),
    );
    assert_eq!(
        sanitize_key(&mut dispatcher, b"RAW_!\0\xff"),
        Ok(PhpValue::Null)
    );
    dispatcher.add(
        "wp_checkdate",
        10,
        DispatcherCallback::new(id(), 2, |context, args| {
            assert_eq!(context.current_hook(), Some("wp_checkdate"));
            assert_eq!(
                args,
                &[
                    PhpValue::Bool(false),
                    PhpValue::Bytes(b"source\0\xff".to_vec())
                ]
            );
            let mut array = PhpArray::new();
            array.insert(PhpKey::from_bytes(b"override"), PhpValue::Integer(17));
            Ok(PhpValue::Array(array))
        }),
    );
    let mut expected = PhpArray::new();
    expected.insert(PhpKey::from_bytes(b"override"), PhpValue::Integer(17));
    assert_eq!(
        wp_checkdate(&mut dispatcher, 13, 1, 2024, b"source\0\xff"),
        Ok(PhpValue::Array(expected))
    );
}

#[test]
fn callback_errors_restore_dispatcher_context() {
    let mut dispatcher = Dispatcher::new();
    for name in ["sanitize_key", "wp_checkdate"] {
        dispatcher.add(
            name,
            10,
            DispatcherCallback::new(id(), 0, |_, _| {
                Err(HookError::CallbackFailed("fixture failure".into()))
            }),
        );
    }
    let expected = Err(HookError::CallbackFailed("fixture failure".into()));
    assert_eq!(sanitize_key(&mut dispatcher, b"input"), expected);
    assert_eq!(dispatcher.hook_stack_depth(), 0);
    assert_eq!(
        wp_checkdate(&mut dispatcher, 1, 1, 2024, b"source"),
        expected
    );
    assert_eq!(dispatcher.hook_stack_depth(), 0);
    dispatcher.remove_all("sanitize_key", None);
    assert_eq!(
        sanitize_key(&mut dispatcher, b"OK"),
        Ok(PhpValue::Bytes(b"ok".to_vec()))
    );
}
