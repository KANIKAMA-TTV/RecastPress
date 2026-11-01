use recastpress_wp::{
    dispatcher::{Dispatcher, DispatcherCallback},
    hooks::{CallbackId, HookError},
    value::{PhpArray, PhpKey, PhpValue},
};
use std::sync::{Arc, Mutex};

#[test]
fn typed_all_return_cannot_replace_or_mutate_named_filter_value() {
    let mut array = PhpArray::new();
    array.insert(PhpKey::from_bytes(b"x"), PhpValue::Null);
    let original = PhpValue::Array(array);
    let expected = original.clone();
    let mut dispatcher = Dispatcher::new();
    dispatcher.add(
        "all",
        0,
        DispatcherCallback::new(id("all_values"), 0, |_, args| {
            assert_eq!(args[0], value("alpha"));
            let PhpValue::Array(mut changed) = args[1].clone() else {
                panic!("expected typed array")
            };
            changed.insert(PhpKey::from_bytes(b"x"), PhpValue::Bool(true));
            Ok(PhpValue::Array(changed))
        }),
    );
    dispatcher.add(
        "alpha",
        10,
        DispatcherCallback::new(id("named_value"), 2, move |_, args| {
            assert_eq!(args, [expected.clone(), PhpValue::Null]);
            Ok(args[0].clone())
        }),
    );
    assert_eq!(
        dispatcher
            .apply_filters("alpha", original.clone(), &[PhpValue::Null])
            .unwrap(),
        original
    );
    assert_eq!(dispatcher.did_filter("alpha"), 1);
    assert_eq!(dispatcher.did_filter("all"), 0);
    assert_eq!(dispatcher.hook_stack_depth(), 0);
}

fn id(name: &str) -> CallbackId {
    CallbackId {
        module: "fixture".into(),
        callback: name.into(),
    }
}
fn value(text: &str) -> PhpValue {
    PhpValue::Bytes(text.as_bytes().to_vec())
}
fn suffix(name: &str) -> DispatcherCallback {
    let name = name.to_string();
    DispatcherCallback::new(id(&name), 1, move |_, args| {
        let Some(PhpValue::Bytes(first)) = args.first() else {
            return Ok(value(&name));
        };
        let mut body = first.clone();
        body.extend_from_slice(name.as_bytes());
        Ok(PhpValue::Bytes(body))
    })
}

#[test]
fn absent_hooks_still_increment_separate_action_and_filter_counts() {
    let mut dispatcher = Dispatcher::new();
    assert_eq!(
        dispatcher.apply_filters("missing", value("original"), &[]),
        Ok(value("original"))
    );
    dispatcher.do_action("missing", &[]).unwrap();
    assert_eq!(dispatcher.did_filter("missing"), 1);
    assert_eq!(dispatcher.did_action("missing"), 1);
    assert_eq!(dispatcher.did_action("other"), 0);
    assert_eq!(dispatcher.current_hook(), None);
    assert!(!dispatcher.doing_hook(None));
}

#[test]
fn all_hook_receives_unsliced_original_arguments_and_cannot_replace_value() {
    let mut dispatcher = Dispatcher::new();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let capture = Arc::clone(&seen);
    dispatcher.add(
        "all",
        10,
        DispatcherCallback::new(id("all"), 0, move |dispatcher, args| {
            assert_eq!(dispatcher.current_hook(), Some("target"));
            capture.lock().unwrap().push(args.to_vec());
            Ok(value("ignored"))
        }),
    );
    dispatcher.add("target", 0, suffix("tail"));
    assert_eq!(dispatcher.priority_of("target", &id("tail")), Some(0));
    assert_eq!(
        dispatcher.apply_filters("target", value("start"), &[value("extra")]),
        Ok(value("starttail"))
    );
    dispatcher.do_action("target", &[]).unwrap();
    assert_eq!(
        *seen.lock().unwrap(),
        vec![
            vec![value("target"), value("start"), value("extra")],
            vec![value("target")]
        ]
    );
    assert_eq!(dispatcher.current_hook(), None);
}

#[test]
fn nested_different_hooks_restore_the_call_stack() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.add(
        "outer",
        10,
        DispatcherCallback::new(id("outer"), 1, |dispatcher, args| {
            assert_eq!(dispatcher.current_hook(), Some("outer"));
            let output = dispatcher.apply_filters("inner", args[0].clone(), &[])?;
            assert_eq!(dispatcher.current_hook(), Some("outer"));
            Ok(output)
        }),
    );
    dispatcher.add(
        "inner",
        10,
        DispatcherCallback::new(id("inner"), 1, |dispatcher, args| {
            assert_eq!(dispatcher.current_hook(), Some("inner"));
            assert!(dispatcher.doing_hook(Some("outer")));
            Ok(args[0].clone())
        }),
    );
    assert_eq!(
        dispatcher.apply_filters("outer", value("value"), &[]),
        Ok(value("value"))
    );
    assert_eq!(dispatcher.current_hook(), None);
    assert_eq!(dispatcher.did_filter("inner"), 1);
}

#[test]
fn deleting_and_readding_an_active_binding_keeps_old_and_new_objects_separate() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.add(
        "target",
        10,
        DispatcherCallback::new(id("replace"), 1, |dispatcher, args| {
            dispatcher.remove_all("target", None);
            dispatcher.add("target", 20, suffix("new"));
            assert_eq!(
                dispatcher.apply_filters("target", value("nested"), &[]),
                Ok(value("nestednew"))
            );
            Ok(args[0].clone())
        }),
    );
    dispatcher.add("target", 10, suffix("old_peer"));
    dispatcher.add("target", 30, suffix("old_future"));
    assert_eq!(
        dispatcher.apply_filters("target", value("first"), &[]),
        Ok(value("firstold_peer"))
    );
    assert_eq!(
        dispatcher.apply_filters("target", value("second"), &[]),
        Ok(value("secondnew"))
    );
    assert!(!dispatcher.has_at("target", 30, &id("old_future")));
}

#[test]
fn all_can_register_a_previously_absent_target_before_dispatch() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.add(
        "all",
        10,
        DispatcherCallback::new(id("register"), 0, |dispatcher, _| {
            dispatcher.add("target", 10, suffix("new"));
            Ok(value("ignored"))
        }),
    );
    assert_eq!(
        dispatcher.apply_filters("target", value("value"), &[]),
        Ok(value("valuenew"))
    );
    assert!(dispatcher.remove("target", 10, &id("new")));
    assert!(!dispatcher.remove("target", 10, &id("new")));
    assert!(!dispatcher.has_callbacks("target"));
}

#[test]
fn recursion_limit_and_callback_failure_restore_request_stack() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.add(
        "loop",
        10,
        DispatcherCallback::new(id("loop"), 1, |dispatcher, args| {
            dispatcher.apply_filters("loop", args[0].clone(), &[])
        }),
    );
    assert_eq!(
        dispatcher.apply_filters("loop", value("value"), &[]),
        Err(HookError::NestingLimit)
    );
    assert_eq!(dispatcher.current_hook(), None);
    dispatcher.remove_all("loop", None);
    dispatcher.add("loop", 10, suffix("recovered"));
    assert_eq!(
        dispatcher.apply_filters("loop", value("value"), &[]),
        Ok(value("valuerecovered"))
    );
}

#[test]
fn all_priority_mutation_reports_reference_warning_before_object_collection() {
    use recastpress_wp::hooks::HookDiagnostic;
    let mut dispatcher = Dispatcher::new();
    dispatcher.add(
        "all",
        10,
        DispatcherCallback::new(id("mutate_all"), 0, |dispatcher, _| {
            dispatcher.add("all", 0, suffix("new"));
            dispatcher.remove_all("all", None);
            Ok(value("ignored"))
        }),
    );
    assert_eq!(
        dispatcher.apply_filters("alpha", value("value"), &[]),
        Ok(value("value"))
    );
    assert_eq!(
        dispatcher.take_diagnostics(),
        vec![HookDiagnostic::UndefinedCurrentPriority { iteration: 0 }]
    );
    assert!(dispatcher.take_diagnostics().is_empty());
}
