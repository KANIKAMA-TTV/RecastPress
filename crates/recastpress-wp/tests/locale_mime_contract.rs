use recastpress_wp::{
    dispatcher::{Dispatcher, DispatcherCallback},
    hooks::{CallbackId, HookError},
    sanitize_locale_name, sanitize_mime_type,
    value::{PhpArray, PhpKey, PhpValue},
};

type Sanitizer = fn(&mut Dispatcher, &[u8]) -> Result<PhpValue, HookError>;

fn functions() -> [(&'static str, Sanitizer); 2] {
    [
        ("sanitize_locale_name", sanitize_locale_name),
        ("sanitize_mime_type", sanitize_mime_type),
    ]
}

fn id() -> CallbackId {
    CallbackId {
        module: "locale_mime_contract".into(),
        callback: "observer".into(),
    }
}

#[test]
fn locale_and_mime_preserve_only_their_exact_bytes_for_all_256_values() {
    for (name, function) in functions() {
        let mut dispatcher = Dispatcher::new();
        for byte in 0..=255u8 {
            let input = [byte];
            let allowed = matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9')
                || if name == "sanitize_locale_name" {
                    matches!(byte, b'_' | b'-')
                } else {
                    matches!(byte, b'-' | b'+' | b'*' | b'.' | b'/')
                };
            assert_eq!(
                function(&mut dispatcher, &input),
                Ok(PhpValue::Bytes(if allowed {
                    vec![byte]
                } else {
                    Vec::new()
                }))
            );
            assert_eq!(input, [byte]);
        }
        assert_eq!(dispatcher.did_filter(name), 256);
        assert_eq!(dispatcher.current_hook(), None);
    }
}

#[test]
fn whitelist_examples_keep_case_wildcards_and_malformed_shapes_without_validating_them() {
    let mut dispatcher = Dispatcher::new();
    for (input, expected) in [
        (b"".as_slice(), b"".as_slice()),
        (b"en_US.UTF-8@euro", b"en_USUTF-8euro"),
        (b"\0ja\xff-JP\r\n", b"ja-JP"),
        (b"123___---", b"123___---"),
    ] {
        assert_eq!(
            sanitize_locale_name(&mut dispatcher, input),
            Ok(PhpValue::Bytes(expected.to_vec()))
        );
    }
    for (input, expected) in [
        (b"".as_slice(), b"".as_slice()),
        (b"Text/HTML; charset=UTF-8", b"Text/HTMLcharsetUTF-8"),
        (b"application/vnd.x+json", b"application/vnd.x+json"),
        (b"*/*", b"*/*"),
        (b"///**..++--", b"///**..++--"),
        (b"x_y\0/\xffZ", b"xy/Z"),
    ] {
        assert_eq!(
            sanitize_mime_type(&mut dispatcher, input),
            Ok(PhpValue::Bytes(expected.to_vec()))
        );
    }
}

#[test]
fn locale_and_mime_filters_receive_original_bytes_and_keep_typed_null_or_array_results() {
    let mut array = PhpArray::new();
    array.insert(PhpKey::Bytes(b"typed".to_vec()), PhpValue::Integer(17));
    for (name, function) in functions() {
        for expected in [PhpValue::Null, PhpValue::Array(array.clone())] {
            let mut dispatcher = Dispatcher::new();
            let returned = expected.clone();
            dispatcher.add(
                name,
                10,
                DispatcherCallback::new(id(), 2, move |context, args| {
                    assert_eq!(context.current_hook(), Some(name));
                    assert_eq!(
                        args,
                        &[
                            PhpValue::Bytes(b"Ab".to_vec()),
                            PhpValue::Bytes(b"A\0\xff!b".to_vec())
                        ]
                    );
                    Ok(returned.clone())
                }),
            );
            assert_eq!(function(&mut dispatcher, b"A\0\xff!b"), Ok(expected));
            assert_eq!(dispatcher.did_filter(name), 1);
            assert_eq!(dispatcher.current_hook(), None);
        }
    }
}

#[test]
fn locale_and_mime_callback_failures_restore_context_and_allow_dispatcher_reuse() {
    for (name, function) in functions() {
        let mut dispatcher = Dispatcher::new();
        dispatcher.add(
            name,
            10,
            DispatcherCallback::new(id(), 2, move |context, _| {
                assert_eq!(context.current_hook(), Some(name));
                Err(HookError::CallbackFailed("fixture failure".into()))
            }),
        );
        assert_eq!(
            function(&mut dispatcher, b"raw"),
            Err(HookError::CallbackFailed("fixture failure".into()))
        );
        assert_eq!(dispatcher.current_hook(), None);
        assert_eq!(dispatcher.hook_stack_depth(), 0);
        dispatcher.remove_all(name, None);
        assert_eq!(
            function(&mut dispatcher, b"OK!"),
            Ok(PhpValue::Bytes(b"OK".to_vec()))
        );
        assert_eq!(dispatcher.did_filter(name), 2);
    }
}
