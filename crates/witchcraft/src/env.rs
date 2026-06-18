//! Lexically-scoped environment with a parent chain. `let` bindings are
//! immutable; `var` bindings may be reassigned. Inner scopes can read and mutate
//! bindings in enclosing scopes.

use crate::value::Value;

#[derive(Clone, Debug)]
struct Binding {
    value: Value,
    mutable: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Scope {
    bindings: Vec<(String, Binding)>,
}

#[derive(Clone, Debug, Default)]
pub struct Env {
    scopes: Vec<Scope>,
}

pub enum DefineError {
    Duplicate,
}

pub enum AssignError {
    Undefined,
    Immutable,
}

impl Env {
    pub fn new() -> Self {
        Env {
            scopes: vec![Scope::default()],
        }
    }

    /// A fresh environment retaining only the global (outermost) scope. Used to
    /// give a called function access to globals without leaking caller locals.
    pub fn global_only(&self) -> Env {
        Env {
            scopes: vec![self.scopes[0].clone()],
        }
    }

    pub fn push(&mut self) {
        self.scopes.push(Scope::default());
    }

    pub fn pop(&mut self) {
        self.scopes.pop();
    }

    pub fn define(&mut self, name: &str, value: Value, mutable: bool) -> Result<(), DefineError> {
        let scope = self.scopes.last_mut().expect("at least one scope");
        if scope.bindings.iter().any(|(n, _)| n == name) {
            return Err(DefineError::Duplicate);
        }
        scope
            .bindings
            .push((name.to_string(), Binding { value, mutable }));
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        for scope in self.scopes.iter().rev() {
            if let Some((_, b)) = scope.bindings.iter().find(|(n, _)| n == name) {
                return Some(&b.value);
            }
        }
        None
    }

    pub fn assign(&mut self, name: &str, value: Value) -> Result<(), AssignError> {
        for scope in self.scopes.iter_mut().rev() {
            if let Some((_, b)) = scope.bindings.iter_mut().find(|(n, _)| n == name) {
                if !b.mutable {
                    return Err(AssignError::Immutable);
                }
                b.value = value;
                return Ok(());
            }
        }
        Err(AssignError::Undefined)
    }
}
