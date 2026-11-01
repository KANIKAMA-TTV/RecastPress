//! Development-only named dispatcher comparison with modeled value fixtures.
mod common;
use recastpress_wp::{
    dispatcher::{Dispatcher, DispatcherCallback},
    hooks::{CallbackId, HookError},
    value::{PhpKey, PhpValue},
};
use serde_json::{Value, json};
use std::{
    io::{self, Read},
    sync::{Arc, Mutex},
};

fn id(name: &str) -> CallbackId {
    CallbackId {
        module: "fixture".into(),
        callback: name.into(),
    }
}
fn value(text: &str) -> PhpValue {
    PhpValue::Bytes(text.as_bytes().to_vec())
}
fn text(value: &PhpValue) -> String {
    if let PhpValue::Bytes(bytes) = value {
        String::from_utf8(bytes.clone()).unwrap()
    } else {
        panic!("nonstring fixture")
    }
}
fn fixture_value(input: &Value, typed: bool) -> PhpValue {
    if typed {
        common::decode(input).expect("modeled typed fixture value")
    } else {
        value_from_string(input)
    }
}
fn rendered(input: &PhpValue, typed: bool) -> Value {
    if typed {
        common::encode(input).expect("modeled typed fixture result")
    } else {
        json!(text(input))
    }
}
fn arguments(value: &Value, typed: bool) -> Vec<PhpValue> {
    value
        .as_array()
        .map(|args| {
            args.iter()
                .map(|value| fixture_value(value, typed))
                .collect()
        })
        .unwrap_or_default()
}
fn value_from_string(input: &Value) -> PhpValue {
    value(input.as_str().expect("fixture string"))
}
fn name(operation: &Value) -> &str {
    operation["hook"].as_str().unwrap_or("alpha")
}
fn counts(dispatcher: &Dispatcher) -> Value {
    let mut output = serde_json::Map::new();
    for name in ["alpha", "beta", "all", "missing"] {
        output.insert(
            name.into(),
            json!({"action":dispatcher.did_action(name), "filter":dispatcher.did_filter(name)}),
        );
    }
    Value::Object(output)
}

fn callback(
    ident: &str,
    accepted: i64,
    definitions: Arc<Value>,
    trace: Arc<Mutex<Vec<Value>>>,
    typed: bool,
) -> DispatcherCallback {
    let ident = ident.to_string();
    DispatcherCallback::new(id(&ident), accepted, move |dispatcher, args| {
        trace.lock().unwrap().push(json!({"event":"callback", "id":ident, "current":dispatcher.current_hook().map_or(json!(false), |s|json!(s)), "depth":dispatcher.hook_stack_depth(), "doing":dispatcher.doing_hook(None), "alpha_active":dispatcher.doing_hook(Some("alpha")), "counts":counts(dispatcher), "args":args.iter().map(|value|rendered(value,typed)).collect::<Vec<_>>()}));
        if dispatcher.hook_stack_depth()
            == definitions[&ident]["mutate_at_depth"].as_u64().unwrap_or(1) as usize
            && let Some(mutations) = definitions[&ident]["mutations"].as_array()
        {
            for mutation in mutations {
                match mutation["kind"].as_str().unwrap() {
                    "add" => {
                        dispatcher.add(
                            name(mutation),
                            mutation["priority"].as_i64().unwrap(),
                            callback(
                                mutation["id"].as_str().unwrap(),
                                mutation["accepted"].as_i64().unwrap_or(1),
                                Arc::clone(&definitions),
                                Arc::clone(&trace),
                                typed,
                            ),
                        );
                    }
                    "remove" => {
                        dispatcher.remove(
                            name(mutation),
                            mutation["priority"].as_i64().unwrap(),
                            &id(mutation["id"].as_str().unwrap()),
                        );
                    }
                    "clear" => dispatcher.remove_all(name(mutation), mutation["priority"].as_i64()),
                    "filter" => {
                        let output = dispatcher.apply_filters(
                            name(mutation),
                            fixture_value(&mutation["value"], typed),
                            &arguments(&mutation["extra"], typed),
                        )?;
                        trace.lock().unwrap().push(json!({"event":"nested_filter_result", "hook":name(mutation), "value":rendered(&output,typed)}));
                    }
                    "action" => {
                        dispatcher
                            .do_action(name(mutation), &arguments(&mutation["args"], typed))?;
                        trace
                            .lock()
                            .unwrap()
                            .push(json!({"event":"nested_action_result", "hook":name(mutation)}));
                    }
                    other => return Err(HookError::CallbackFailed(other.into())),
                }
            }
        }
        if typed {
            let definition = &definitions[&ident];
            return match definition["operation"].as_str().unwrap_or("first") {
                "first" => Ok(args.first().cloned().unwrap_or(PhpValue::Null)),
                "arg" => {
                    let index = usize::try_from(definition["index"].as_u64().unwrap_or(0))
                        .unwrap_or(usize::MAX);
                    Ok(args.get(index).cloned().unwrap_or(PhpValue::Null))
                }
                "constant" => {
                    common::decode(&definition["return_value"]).map_err(HookError::CallbackFailed)
                }
                "array_put" => {
                    let Some(PhpValue::Array(mut array)) = args.first().cloned() else {
                        return Err(HookError::CallbackFailed(
                            "array_put requires an array".into(),
                        ));
                    };
                    let key = match common::decode(&definition["key"])
                        .map_err(HookError::CallbackFailed)?
                    {
                        PhpValue::Integer(key) => PhpKey::Integer(key),
                        PhpValue::Bytes(key) => PhpKey::from_bytes(&key),
                        _ => {
                            return Err(HookError::CallbackFailed(
                                "invalid fixture array key".into(),
                            ));
                        }
                    };
                    array.insert(
                        key,
                        common::decode(&definition["entry_value"])
                            .map_err(HookError::CallbackFailed)?,
                    );
                    Ok(PhpValue::Array(array))
                }
                _ => Err(HookError::CallbackFailed(
                    "unknown typed fixture operation".into(),
                )),
            };
        }
        let first = args.first().map(text).unwrap_or_default();
        Ok(value(&format!("{first}|{ident}")))
    })
}

fn scenario(case: &Value) -> Value {
    let typed = case["typed"].as_bool().unwrap_or(false);
    let mut dispatcher = Dispatcher::new();
    let definitions = Arc::new(case["callbacks"].clone());
    let trace = Arc::new(Mutex::new(Vec::new()));
    for registration in case["initial"].as_array().unwrap() {
        dispatcher.add(
            name(registration),
            registration["priority"].as_i64().unwrap(),
            callback(
                registration["id"].as_str().unwrap(),
                registration["accepted"].as_i64().unwrap_or(1),
                Arc::clone(&definitions),
                Arc::clone(&trace),
                typed,
            ),
        );
    }
    let mut outputs = Vec::new();
    for operation in case["calls"].as_array().unwrap() {
        let output = match operation["kind"].as_str().unwrap() {
            "filter" => rendered(
                &dispatcher
                    .apply_filters(
                        name(operation),
                        fixture_value(&operation["value"], typed),
                        &arguments(&operation["extra"], typed),
                    )
                    .unwrap(),
                typed,
            ),
            "action" => {
                dispatcher
                    .do_action(name(operation), &arguments(&operation["args"], typed))
                    .unwrap();
                Value::Null
            }
            _ => panic!("invalid fixture operation"),
        };
        outputs.push(json!({"value":output,"current":dispatcher.current_hook().map_or(json!(false), |s|json!(s)),"depth":dispatcher.hook_stack_depth()}));
    }
    let mut priorities = serde_json::Map::new();
    for hook in ["alpha", "beta", "all", "missing"] {
        let callbacks: serde_json::Map<String, Value> = definitions
            .as_object()
            .unwrap()
            .keys()
            .map(|ident| {
                (
                    ident.clone(),
                    json!(dispatcher.priority_of(hook, &id(ident))),
                )
            })
            .collect();
        priorities.insert(hook.into(), Value::Object(callbacks));
    }
    let diagnostics: Vec<_> = dispatcher
        .take_diagnostics()
        .iter()
        .map(|diagnostic| json!({"level":2,"message":diagnostic.message()}))
        .collect();
    json!({"id":case["id"],"outputs":outputs,"trace":trace.lock().unwrap().clone(),"priorities":priorities,"counts":counts(&dispatcher),"diagnostics":diagnostics})
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = Vec::new();
    io::stdin()
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut input)?;
    if input.len() > 16 * 1024 * 1024 {
        return Err("probe input too large".into());
    }
    let cases: Vec<Value> = serde_json::from_slice(&input)?;
    println!(
        "{}",
        serde_json::to_string(&cases.iter().map(scenario).collect::<Vec<_>>())?
    );
    Ok(())
}
