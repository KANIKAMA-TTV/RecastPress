use recastpress_wp::{
    dispatcher::{Dispatcher, DispatcherCallback},
    hooks::{CallbackId, HookError},
    is_email,
    value::{PhpArray, PhpKey, PhpValue},
};

fn id() -> CallbackId {
    CallbackId {
        module: "email_contract".into(),
        callback: "observer".into(),
    }
}

fn check_context(input: &[u8], context: Option<&str>) {
    let mut dispatcher = Dispatcher::new();
    let expected_context = context.map_or(PhpValue::Null, |text| {
        PhpValue::Bytes(text.as_bytes().to_vec())
    });
    let expected_value = if context.is_some() {
        PhpValue::Bool(false)
    } else {
        PhpValue::Bytes(input.to_vec())
    };
    let expected_args = vec![
        expected_value.clone(),
        PhpValue::Bytes(input.to_vec()),
        expected_context,
    ];
    dispatcher.add(
        "is_email",
        10,
        DispatcherCallback::new(id(), 3, move |context, args| {
            assert_eq!(context.current_hook(), Some("is_email"));
            assert_eq!(args, expected_args);
            Ok(args[0].clone())
        }),
    );
    assert_eq!(is_email(&mut dispatcher, input), Ok(expected_value));
    assert_eq!(dispatcher.did_filter("is_email"), 1);
    assert_eq!(dispatcher.current_hook(), None);
}

#[test]
fn email_reports_all_eight_rejections_in_source_order_and_null_on_success() {
    for (input, context) in [
        (b"a@b.c".as_slice(), "email_too_short"),
        (b"abcdef", "email_no_at"),
        (b"a b@c.de", "local_invalid_chars"),
        (b"a@b..cd", "domain_period_sequence"),
        (b"a@.b.cd", "domain_period_limits"),
        (b"abc@de", "domain_no_periods"),
        (b"a@-b.cd", "sub_hyphen_limits"),
        (b"a@b_.cd", "sub_invalid_chars"),
    ] {
        check_context(input, Some(context));
    }
    check_context(b"a@b.cd", None);
    check_context(b"@a..b", Some("email_too_short"));
    check_context(b"@ab.cd", Some("email_no_at"));
    check_context(b"@a@b.cd", Some("local_invalid_chars"));
    check_context(b"bad space@.x..y", Some("local_invalid_chars"));
    check_context(b"a@..b.cd", Some("domain_period_sequence"));
    check_context(b"a@b_.-cd", Some("sub_invalid_chars"));
    check_context(b"abcde@", Some("domain_no_periods"));
    check_context(b"a@@b.cd", Some("sub_invalid_chars"));
}

#[test]
fn email_keeps_wordpress_local_part_quirks_and_exact_php_trim_byte_rules() {
    for input in [
        b"a\n@b.cd".as_slice(),
        b"..@b.cd",
        b"'a'@b.cd",
        b"ab@c.d",
        b"UP+tag@B.CD",
        b"AZ09!#$%&'*+/=?^_`{|}~.-@B.Cd",
    ] {
        check_context(input, None);
        let original = input.to_vec();
        assert_eq!(
            is_email(&mut Dispatcher::new(), &original),
            Ok(PhpValue::Bytes(original.clone()))
        );
        assert_eq!(original, input);
    }
    check_context(b"a\n\n@b.cd", Some("local_invalid_chars"));
    check_context(b"\n@b.cd", Some("local_invalid_chars"));
    for byte in [b' ', b'\t', b'\n', b'\r', b'\0', 0x0b] {
        check_context(
            &[b"a@b.cd".as_slice(), &[byte]].concat(),
            Some("domain_period_limits"),
        );
        check_context(
            &[b"a@b.".as_slice(), &[byte], b"cd"].concat(),
            Some("sub_hyphen_limits"),
        );
    }
    check_context(b"a@b.\x0ccd", Some("sub_invalid_chars"));
}

#[test]
fn email_checks_every_byte_in_local_and_domain_positions() {
    for byte in 0..=255u8 {
        let local = [b"a".as_slice(), &[byte], b"@b.cd"].concat();
        let local_valid = matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'/' | b'=' | b'?' | b'^' | b'_' | b'`' | b'{' | b'|' | b'}' | b'~' | b'.' | b'-' | b'\n');
        // Inserting @ changes the first split, so domain validation is reached.
        check_context(
            &local,
            if byte == b'@' {
                Some("sub_invalid_chars")
            } else if local_valid {
                None
            } else {
                Some("local_invalid_chars")
            },
        );
        let domain = [b"a@b".as_slice(), &[byte], b"c.de"].concat();
        let domain_valid = matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.');
        check_context(
            &domain,
            if domain_valid {
                None
            } else {
                Some("sub_invalid_chars")
            },
        );
        let terminal = [b"a@b.c".as_slice(), &[byte]].concat();
        let context = match byte {
            b' ' | b'\t' | b'\n' | b'\r' | b'\0' | 0x0b | b'.' => Some("domain_period_limits"),
            b'-' => Some("sub_hyphen_limits"),
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => None,
            _ => Some("sub_invalid_chars"),
        };
        check_context(&terminal, context);
    }
}

#[test]
fn email_filter_keeps_typed_null_and_array_results_even_on_rejection() {
    let mut array = PhpArray::new();
    array.insert(PhpKey::Bytes(b"typed".to_vec()), PhpValue::Integer(17));
    for expected in [PhpValue::Null, PhpValue::Array(array)] {
        for input in [b"a@b.cd".as_slice(), b"x"] {
            let mut dispatcher = Dispatcher::new();
            let returned = expected.clone();
            dispatcher.add(
                "is_email",
                10,
                DispatcherCallback::new(id(), 3, move |_, _| Ok(returned.clone())),
            );
            assert_eq!(is_email(&mut dispatcher, input), Ok(expected.clone()));
            assert_eq!(dispatcher.did_filter("is_email"), 1);
        }
    }
}

#[test]
fn email_callback_failure_restores_context_and_allows_reuse() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.add(
        "is_email",
        10,
        DispatcherCallback::new(id(), 3, |context, args| {
            assert_eq!(context.current_hook(), Some("is_email"));
            assert_eq!(args[2], PhpValue::Bytes(b"email_too_short".to_vec()));
            Err(HookError::CallbackFailed("fixture failure".into()))
        }),
    );
    assert_eq!(
        is_email(&mut dispatcher, b"x"),
        Err(HookError::CallbackFailed("fixture failure".into()))
    );
    assert_eq!(dispatcher.current_hook(), None);
    assert_eq!(dispatcher.hook_stack_depth(), 0);
    dispatcher.remove_all("is_email", None);
    assert_eq!(
        is_email(&mut dispatcher, b"a@b.cd"),
        Ok(PhpValue::Bytes(b"a@b.cd".to_vec()))
    );
    assert_eq!(dispatcher.did_filter("is_email"), 2);
}
