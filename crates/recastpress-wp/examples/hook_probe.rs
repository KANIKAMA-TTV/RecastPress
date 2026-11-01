//! Development-only scenario probe for the request-local Hook subset.
mod common;
use recastpress_wp::{
    hooks::{Callback, CallbackId, Hook, HookError},
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
        String::from_utf8(bytes.clone()).expect("UTF-8 fixture")
    } else {
        panic!("nonstring hook fixture")
    }
}
fn fixture_value(input: &Value, typed: bool) -> PhpValue {
    if typed {
        common::decode(input).expect("modeled typed fixture value")
    } else {
        value(input.as_str().expect("string fixture value"))
    }
}
fn rendered(input: &PhpValue, typed: bool) -> Value {
    if typed {
        common::encode(input).expect("modeled typed fixture result")
    } else {
        json!(text(input))
    }
}
fn arguments(values: &Value, typed: bool) -> Vec<PhpValue> {
    values
        .as_array()
        .map(|values| values.iter().map(|v| fixture_value(v, typed)).collect())
        .unwrap_or_default()
}

fn callback(
    name: &str,
    accepted: i64,
    definitions: Arc<Value>,
    trace: Arc<Mutex<Vec<Value>>>,
    typed: bool,
) -> Callback {
    let name = name.to_string();
    Callback::new(id(&name), accepted, move |hook, args| {
        trace.lock().unwrap().push(json!({"event":"callback", "id":name, "priority":hook.current_priority().map_or(json!(false), |priority| json!(priority)), "depth":hook.nesting_depth(), "args":args.iter().map(|value| rendered(value,typed)).collect::<Vec<_>>()}));
        if hook.nesting_depth()
            == definitions[&name]["mutate_at_depth"].as_u64().unwrap_or(1) as usize
            && let Some(mutations) = definitions[&name]["mutations"].as_array()
        {
            for mutation in mutations {
                match mutation["kind"].as_str().unwrap() {
                    "add" => {
                        let target = mutation["id"].as_str().unwrap();
                        hook.add(
                            mutation["priority"].as_i64().unwrap(),
                            callback(
                                target,
                                mutation["accepted"].as_i64().unwrap_or(1),
                                Arc::clone(&definitions),
                                Arc::clone(&trace),
                                typed,
                            ),
                        );
                    }
                    "remove" => {
                        hook.remove(
                            mutation["priority"].as_i64().unwrap(),
                            &id(mutation["id"].as_str().unwrap()),
                        );
                    }
                    "clear" => {
                        hook.remove_all(mutation["priority"].as_i64());
                    }
                    "filter" => {
                        let output = hook.apply_filters(
                            fixture_value(&mutation["value"], typed),
                            &arguments(&mutation["extra"], typed),
                        )?;
                        trace
                            .lock()
                            .unwrap()
                            .push(json!({"event":"nested_filter_result", "value":rendered(&output,typed)}));
                    }
                    "action" => {
                        hook.do_action(&arguments(&mutation["args"], typed))?;
                        trace
                            .lock()
                            .unwrap()
                            .push(json!({"event":"nested_action_result"}));
                    }
                    other => return Err(HookError::CallbackFailed(other.into())),
                }
            }
        }
        if typed {
            let definition = &definitions[&name];
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
        Ok(value(&format!("{first}|{name}")))
    })
}

fn scenario(case: &Value) -> Value {
    let typed = case["typed"].as_bool().unwrap_or(false);
    let mut hook = Hook::new();
    let definitions = Arc::new(case["callbacks"].clone());
    let trace = Arc::new(Mutex::new(Vec::new()));
    for registration in case["initial"].as_array().unwrap() {
        hook.add(
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
        match operation["kind"].as_str().unwrap() {
            "filter" => {
                let output = hook
                    .apply_filters(
                        fixture_value(&operation["value"], typed),
                        &arguments(&operation["extra"], typed),
                    )
                    .unwrap();
                outputs.push(rendered(&output, typed));
            }
            "action" => {
                hook.do_action(&arguments(&operation["args"], typed))
                    .unwrap();
                outputs.push(Value::Null);
            }
            _ => panic!("invalid fixture operation"),
        }
    }
    let priorities: serde_json::Map<String, Value> = definitions
        .as_object()
        .unwrap()
        .keys()
        .map(|name| (name.clone(), json!(hook.priority_of(&id(name)))))
        .collect();
    json!({"id":case["id"], "outputs":outputs, "trace":trace.lock().unwrap().clone(), "priorities":priorities, "active_depth":hook.nesting_depth()})
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
