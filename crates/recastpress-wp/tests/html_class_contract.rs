use recastpress_wp::sanitize_html_class;
use recastpress_wp::{
    dispatcher::{Dispatcher, DispatcherCallback},
    hooks::{CallbackId, HookError},
    value::{PhpArray, PhpKey, PhpValue},
};

fn id() -> CallbackId {
    CallbackId {
        module: "html_class_contract".into(),
        callback: "observer".into(),
    }
}

#[test]
fn html_class_percent_removal_is_single_pass_and_preserves_ascii_case() {
    let mut dispatcher = Dispatcher::new();
    for (input, expected) in [
        (b"A_z-09".as_slice(), b"A_z-09".as_slice()),
        (b"%41A%ff_z", b"A_z"),
        (b"%2541", b"41"),
        (b"%a%41", b"a"),
        (b"%%41", b""),
        (b"%zz%0", b"zz0"),
        (b"a\0\xff\xc3\xa9!B", b"aB"),
    ] {
        assert_eq!(
            sanitize_html_class(&mut dispatcher, input, b""),
            Ok(PhpValue::Bytes(expected.to_vec()))
        );
    }
    for byte in 0..=255u8 {
        let raw = [byte];
        let expected = if matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-') {
            vec![byte]
        } else {
            Vec::new()
        };
        assert_eq!(
            sanitize_html_class(&mut dispatcher, &raw, b""),
            Ok(PhpValue::Bytes(expected))
        );
        let encoded = format!("%{byte:02X}").into_bytes();
        assert_eq!(
            sanitize_html_class(&mut dispatcher, &encoded, b""),
            Ok(PhpValue::Bytes(Vec::new()))
        );
        assert_eq!(raw, [byte]);
    }
}

#[test]
fn html_class_fallback_truthiness_distinguishes_zero_from_empty_clean_result() {
    let mut dispatcher = Dispatcher::new();
    for (input, fallback, expected) in [
        (b"".as_slice(), b"".as_slice(), b"".as_slice()),
        (b"!!!", b"0", b""),
        (b"!!!", b"00", b"00"),
        (b"0", b"fallback", b"0"),
        (b"%41", b"A.b", b"Ab"),
        (b"", b"!!!", b""),
        (b"", b"%41", b""),
    ] {
        let before = dispatcher.did_filter("sanitize_html_class");
        assert_eq!(
            sanitize_html_class(&mut dispatcher, input, fallback),
            Ok(PhpValue::Bytes(expected.to_vec()))
        );
        assert_eq!(dispatcher.did_filter("sanitize_html_class"), before + 1);
    }
}

#[test]
fn html_class_terminal_filter_receives_exact_raw_and_fallback_arguments_once() {
    for (input, fallback, clean, raw, terminal_fallback) in [
        (
            b"A.!".as_slice(),
            b"F.!".as_slice(),
            b"A".as_slice(),
            b"A.!".as_slice(),
            b"F.!".as_slice(),
        ),
        (b"!!!", b"0", b"", b"!!!", b"0"),
        (b"!!!", b"F.!", b"F", b"F.!", b""),
        (b"!!!", b"%41", b"", b"%41", b""),
    ] {
        let mut dispatcher = Dispatcher::new();
        let expected_args = vec![
            PhpValue::Bytes(clean.to_vec()),
            PhpValue::Bytes(raw.to_vec()),
            PhpValue::Bytes(terminal_fallback.to_vec()),
        ];
        dispatcher.add(
            "sanitize_html_class",
            10,
            DispatcherCallback::new(id(), 3, move |context, args| {
                assert_eq!(context.current_hook(), Some("sanitize_html_class"));
                assert_eq!(args, expected_args);
                Ok(args[0].clone())
            }),
        );
        assert_eq!(
            sanitize_html_class(&mut dispatcher, input, fallback),
            Ok(PhpValue::Bytes(clean.to_vec()))
        );
        assert_eq!(dispatcher.did_filter("sanitize_html_class"), 1);
        assert_eq!(dispatcher.current_hook(), None);
    }
}

#[test]
fn html_class_filter_returns_keep_null_and_array_types() {
    let mut array = PhpArray::new();
    array.insert(PhpKey::Bytes(b"typed".to_vec()), PhpValue::Integer(17));
    for expected in [PhpValue::Null, PhpValue::Array(array)] {
        let mut dispatcher = Dispatcher::new();
        let returned = expected.clone();
        dispatcher.add(
            "sanitize_html_class",
            10,
            DispatcherCallback::new(id(), 3, move |_, _| Ok(returned.clone())),
        );
        assert_eq!(
            sanitize_html_class(&mut dispatcher, b"!!!", b"fallback"),
            Ok(expected)
        );
        assert_eq!(dispatcher.did_filter("sanitize_html_class"), 1);
    }
}

#[test]
fn html_class_callback_errors_restore_dispatcher_context_and_allow_reuse() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.add(
        "sanitize_html_class",
        10,
        DispatcherCallback::new(id(), 3, |context, args| {
            assert_eq!(context.current_hook(), Some("sanitize_html_class"));
            assert_eq!(args[1], PhpValue::Bytes(b"fallback".to_vec()));
            Err(HookError::CallbackFailed("fixture failure".into()))
        }),
    );
    assert_eq!(
        sanitize_html_class(&mut dispatcher, b"!!!", b"fallback"),
        Err(HookError::CallbackFailed("fixture failure".into()))
    );
    assert_eq!(dispatcher.did_filter("sanitize_html_class"), 1);
    assert_eq!(dispatcher.current_hook(), None);
    assert_eq!(dispatcher.hook_stack_depth(), 0);
    dispatcher.remove_all("sanitize_html_class", None);
    assert_eq!(
        sanitize_html_class(&mut dispatcher, b"OK!", b""),
        Ok(PhpValue::Bytes(b"OK".to_vec()))
    );
    assert_eq!(dispatcher.did_filter("sanitize_html_class"), 2);
}
