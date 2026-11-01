//! Named plugin.php-style dispatch for supported request-local native callbacks.

use crate::{
    hooks::{
        self, CallbackId, Context, ContextCallback, HookDiagnostic, HookError, MAX_NESTING, Mode,
        Storage,
    },
    value::PhpValue,
};
use std::collections::BTreeMap;

pub type DispatcherCallback = ContextCallback<Dispatcher>;

#[derive(Debug, Default)]
pub struct Dispatcher {
    bindings: BTreeMap<String, usize>,
    slots: Vec<Option<Storage<Dispatcher>>>,
    current: Vec<String>,
    actions: BTreeMap<String, u64>,
    filters: BTreeMap<String, u64>,
    active_calls: usize,
    diagnostics: Vec<HookDiagnostic>,
}

impl Context for Dispatcher {
    fn storage(&mut self, slot: usize) -> &mut Storage<Self> {
        self.slots[slot]
            .as_mut()
            .expect("active or registered hook object")
    }
}

impl Dispatcher {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, name: &str, priority: i64, callback: DispatcherCallback) {
        let slot = match self.bindings.get(name) {
            Some(&slot) => slot,
            None => {
                let slot = self
                    .slots
                    .iter()
                    .position(Option::is_none)
                    .unwrap_or(self.slots.len());
                if slot == self.slots.len() {
                    self.slots.push(Some(Storage::default()));
                } else {
                    self.slots[slot] = Some(Storage::default());
                }
                self.bindings.insert(name.into(), slot);
                slot
            }
        };
        self.storage(slot).add(priority, callback);
        self.capture_diagnostics(slot);
    }
    pub fn remove(&mut self, name: &str, priority: i64, id: &CallbackId) -> bool {
        let Some(&slot) = self.bindings.get(name) else {
            return false;
        };
        let removed = self.storage(slot).remove(priority, id);
        self.capture_diagnostics(slot);
        self.unbind_empty(name, slot);
        self.collect_unused();
        removed
    }
    pub fn remove_all(&mut self, name: &str, priority: Option<i64>) {
        if let Some(&slot) = self.bindings.get(name) {
            self.storage(slot).remove_all(priority);
            self.capture_diagnostics(slot);
            self.unbind_empty(name, slot);
            self.collect_unused();
        }
    }
    fn unbind_empty(&mut self, name: &str, slot: usize) {
        if !self.storage(slot).has_callbacks() {
            self.bindings.remove(name);
        }
    }
    fn collect_unused(&mut self) {
        for (slot, state) in self.slots.iter_mut().enumerate() {
            if let Some(state) = state.as_mut() {
                self.diagnostics.extend(state.take_diagnostics());
            }
            if state.as_ref().is_some_and(|state| !state.is_active())
                && !self.bindings.values().any(|bound| *bound == slot)
            {
                *state = None;
            }
        }
    }
    fn capture_diagnostics(&mut self, slot: usize) {
        let pending = self.storage(slot).take_diagnostics();
        self.diagnostics.extend(pending);
    }
    pub fn take_diagnostics(&mut self) -> Vec<HookDiagnostic> {
        std::mem::take(&mut self.diagnostics)
    }
    fn state(&self, name: &str) -> Option<&Storage<Self>> {
        self.bindings
            .get(name)
            .and_then(|&slot| self.slots[slot].as_ref())
    }
    pub fn has_callbacks(&self, name: &str) -> bool {
        self.state(name).is_some_and(Storage::has_callbacks)
    }
    pub fn has_at(&self, name: &str, priority: i64, id: &CallbackId) -> bool {
        self.state(name)
            .is_some_and(|state| state.has_at(priority, id))
    }
    pub fn priority_of(&self, name: &str, id: &CallbackId) -> Option<i64> {
        self.state(name).and_then(|state| state.priority_of(id))
    }
    pub fn current_hook(&self) -> Option<&str> {
        self.current.last().map(String::as_str)
    }
    pub fn hook_stack_depth(&self) -> usize {
        self.current.len()
    }
    pub fn doing_hook(&self, name: Option<&str>) -> bool {
        name.map_or(!self.current.is_empty(), |name| {
            self.current.iter().any(|current| current == name)
        })
    }
    pub fn did_action(&self, name: &str) -> u64 {
        self.actions.get(name).copied().unwrap_or(0)
    }
    pub fn did_filter(&self, name: &str) -> u64 {
        self.filters.get(name).copied().unwrap_or(0)
    }
    pub fn apply_filters(
        &mut self,
        name: &str,
        value: PhpValue,
        extra: &[PhpValue],
    ) -> Result<PhpValue, HookError> {
        self.dispatch(name, value, extra, false)
    }
    pub fn do_action(&mut self, name: &str, args: &[PhpValue]) -> Result<(), HookError> {
        self.dispatch(name, PhpValue::Bytes(Vec::new()), args, true)
            .map(|_| ())
    }

    fn dispatch(
        &mut self,
        name: &str,
        value: PhpValue,
        extra: &[PhpValue],
        action: bool,
    ) -> Result<PhpValue, HookError> {
        if self.active_calls >= MAX_NESTING {
            return Err(HookError::NestingLimit);
        }
        self.active_calls += 1;
        let base_depth = self.current.len();
        let counts = if action {
            &mut self.actions
        } else {
            &mut self.filters
        };
        let count = counts.entry(name.into()).or_default();
        *count = count.saturating_add(1);
        let result = (|| {
            if let Some(&all) = self.bindings.get("all") {
                self.current.push(name.into());
                let mut args = vec![PhpValue::Bytes(name.as_bytes().to_vec())];
                if !action {
                    args.push(value.clone());
                }
                args.extend_from_slice(extra);
                hooks::invoke(self, all, PhpValue::Bytes(Vec::new()), args, Mode::All)?;
            }
            let Some(&slot) = self.bindings.get(name) else {
                if self.bindings.contains_key("all") {
                    self.current.pop();
                }
                return Ok(value);
            };
            if !self.bindings.contains_key("all") {
                self.current.push(name.into());
            }
            let mut args = if action {
                Vec::new()
            } else {
                vec![value.clone()]
            };
            args.extend_from_slice(extra);
            if action && args.is_empty() {
                args.push(PhpValue::Bytes(Vec::new()));
            }
            let output = hooks::invoke(
                self,
                slot,
                value,
                args,
                if action { Mode::Action } else { Mode::Filter },
            )?;
            self.current.pop();
            Ok(output)
        })();
        self.active_calls -= 1;
        if result.is_err() {
            self.current.truncate(base_depth);
        }
        self.collect_unused();
        result
    }
}
