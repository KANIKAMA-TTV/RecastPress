use recastpress_wp::{
    CompatibilityError,
    dispatcher::{Dispatcher, DispatcherCallback},
    filtered::FilteredError,
    hooks::{CallbackId, HookError},
    query::parse_flat_query,
    value::{PhpArray, PhpKey, PhpValue},
    wp_parse_args, wp_parse_str,
};

fn bytes(value: &[u8]) -> PhpValue {
    PhpValue::Bytes(value.to_vec())
}

#[test]
fn flat_queries_keep_php_key_normalization_order_and_arbitrary_value_bytes() {
    let parsed =
        parse_flat_query(b" a.b=first&a+b=second&0=%00%FF&00=kept&bare&=ignored&broken=%2G+%41")
            .unwrap();
    let entries: Vec<_> = parsed
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    assert_eq!(
        entries,
        vec![
            (PhpKey::from_bytes(b"a_b"), bytes(b"second")),
            (PhpKey::Integer(0), bytes(b"\0\xff")),
            (PhpKey::from_bytes(b"00"), bytes(b"kept")),
            (PhpKey::from_bytes(b"bare"), bytes(b"")),
            (PhpKey::from_bytes(b"broken"), bytes(b"%2G A")),
        ]
    );
}

#[test]
fn unsupported_query_refusals_never_invoke_filters_on_a_partial_result() {
    let mut dispatcher = Dispatcher::new();
    for input in [
        b"first=ok&a[=value".as_slice(),
        b"first=ok&a%5Bx%5Dsuffix=value",
        b"a%00b=value",
        b"first=ok\0other=bad",
    ] {
        assert_eq!(
            wp_parse_str(&mut dispatcher, input),
            Err(FilteredError::Unsupported(
                CompatibilityError::UnsupportedValue
            ))
        );
    }
    let thousand = std::iter::repeat_n("a=value", 1000)
        .collect::<Vec<_>>()
        .join("&");
    assert_eq!(parse_flat_query(thousand.as_bytes()).unwrap().len(), 1);
    let excessive = format!("{thousand}&b=excess");
    assert_eq!(
        wp_parse_str(&mut dispatcher, excessive.as_bytes()),
        Err(FilteredError::Unsupported(
            CompatibilityError::UnsupportedValue
        ))
    );
    assert_eq!(dispatcher.did_filter("wp_parse_str"), 0);
}

#[test]
fn parse_args_only_filters_strings_and_preserves_the_filtered_result_type() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.add(
        "wp_parse_str",
        10,
        DispatcherCallback::new(
            CallbackId {
                module: "test".into(),
                callback: "constant".into(),
            },
            1,
            |context, args| {
                assert_eq!(context.current_hook(), Some("wp_parse_str"));
                assert_eq!(args.len(), 1);
                assert!(matches!(args[0], PhpValue::Array(_)));
                Ok(PhpValue::Null)
            },
        ),
    );
    let defaults = PhpArray::new();
    assert_eq!(
        wp_parse_args(&mut dispatcher, &bytes(b"a=b"), &defaults),
        Ok(PhpValue::Null)
    );
    assert_eq!(dispatcher.did_filter("wp_parse_str"), 1);
    assert_eq!(
        wp_parse_args(
            &mut dispatcher,
            &PhpValue::Array(defaults.clone()),
            &defaults
        ),
        Ok(PhpValue::Array(defaults.clone()))
    );
    assert_eq!(dispatcher.did_filter("wp_parse_str"), 1);
    let mut nonempty = defaults;
    nonempty.insert(PhpKey::from_bytes(b"default"), bytes(b"value"));
    assert_eq!(
        wp_parse_args(&mut dispatcher, &bytes(b"a=b"), &nonempty),
        Err(FilteredError::Unsupported(
            CompatibilityError::UnsupportedValue
        ))
    );
    assert_eq!(dispatcher.current_hook(), None);
}

#[test]
fn parse_string_callback_failure_propagates_and_restores_context() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.add(
        "wp_parse_str",
        10,
        DispatcherCallback::new(
            CallbackId {
                module: "test".into(),
                callback: "fail".into(),
            },
            0,
            |_, _| Err(HookError::CallbackFailed("fixture".into())),
        ),
    );
    assert_eq!(
        wp_parse_str(&mut dispatcher, b"a=b"),
        Err(FilteredError::Callback(HookError::CallbackFailed(
            "fixture".into()
        )))
    );
    assert_eq!(dispatcher.hook_stack_depth(), 0);
}

#[test]
fn balanced_arrays_keep_negative_append_keys_literal_inner_names_and_order() {
    use recastpress_wp::query::parse_query_bytes;
    let parsed =
        parse_query_bytes(b"a[-5]=first&a[]=second&a[01]=third&a[x.y]=fourth&a[]=fifth").unwrap();
    let PhpValue::Array(array) = parsed.get(&PhpKey::from_bytes(b"a")).unwrap() else {
        panic!("expected array")
    };
    assert_eq!(
        array.iter().map(|(key, _)| key.clone()).collect::<Vec<_>>(),
        [
            PhpKey::Integer(-5),
            PhpKey::Integer(-4),
            PhpKey::from_bytes(b"01"),
            PhpKey::from_bytes(b"x.y"),
            PhpKey::Integer(-3)
        ]
    );
    assert_eq!(array.get(&PhpKey::Integer(-3)), Some(&bytes(b"fifth")));
    assert!(parse_flat_query(b"a[]=value").is_err());
}

#[test]
fn scalar_and_array_collisions_replace_the_type_without_merging_old_values() {
    use recastpress_wp::query::parse_query_bytes;
    let parsed = parse_query_bytes(b"a=old&a[b]=next&a[b][]=last&a[]=tail").unwrap();
    let PhpValue::Array(a) = parsed.get(&PhpKey::from_bytes(b"a")).unwrap() else {
        panic!("expected array")
    };
    let PhpValue::Array(b) = a.get(&PhpKey::from_bytes(b"b")).unwrap() else {
        panic!("expected child array")
    };
    assert_eq!(b.get(&PhpKey::Integer(0)), Some(&bytes(b"last")));
    assert_eq!(a.get(&PhpKey::Integer(0)), Some(&bytes(b"tail")));
    let replaced = parse_query_bytes(b"a[]=old&a=new").unwrap();
    assert_eq!(
        replaced.get(&PhpKey::from_bytes(b"a")),
        Some(&bytes(b"new"))
    );
    let nested = parse_query_bytes(b"a[][b]=x&a[][c]=y").unwrap();
    let PhpValue::Array(a) = nested.get(&PhpKey::from_bytes(b"a")).unwrap() else {
        panic!("expected array")
    };
    assert_eq!(a.len(), 2);
}

#[test]
fn unsupported_array_paths_depth_and_append_overflow_return_no_partial_result() {
    use recastpress_wp::query::parse_query_bytes;
    for query in [
        b"ok=x&a[b]tail=y".as_slice(),
        b"a[ ]=x",
        b"a[x y]=x",
        b"a[[x]]=y",
        b"[]=x",
        b"a[9223372036854775807]=x&a[]=y",
    ] {
        assert!(parse_query_bytes(query).is_err());
    }
    let permitted = format!("a{}=value", "[x]".repeat(64));
    assert!(parse_query_bytes(permitted.as_bytes()).is_ok());
    let excessive = format!("a{}=value", "[x]".repeat(65));
    assert_eq!(
        parse_query_bytes(excessive.as_bytes()),
        Err(CompatibilityError::RecursionLimit)
    );
}
