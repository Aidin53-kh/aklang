use ::std::collections::HashMap;
use ::std::sync::{Arc, Mutex};

use self::value::Value;

pub mod eval;
pub mod std;
pub mod value;

#[derive(Debug, Clone)]
pub struct ScopeStack(Vec<Arc<Mutex<Scope>>>);

pub type Scope = HashMap<String, (Value, DeclType)>;

#[derive(Debug, Clone)]
pub enum DeclType {
    Mutable,
    Immutable,
}

impl ScopeStack {
    pub fn new(scopes: Vec<Arc<Mutex<Scope>>>) -> ScopeStack {
        ScopeStack(scopes)
    }

    fn new_from_push(&self, scope: Scope) -> ScopeStack {
        let mut scopes = self.0.clone();
        scopes.push(Arc::new(Mutex::new(scope)));

        ScopeStack::new(scopes)
    }

    fn declare(&mut self, name: String, value: Value, decl_type: DeclType) -> Result<(), String> {
        let mut current_scope = self
            .0
            .last()
            .expect("`ScopeStack` stack shouldn't be empty")
            .lock()
            .unwrap();

        if current_scope.contains_key(&name) {
            return Err(format!("'{}' already define in this scope", name));
        }
        current_scope.insert(name, (value, decl_type));

        Ok(())
    }

    fn assgin(&mut self, name: String, value: Value) -> Result<(), String> {
        for scope in self.0.iter().rev() {
            let mut unlocked_scope = scope.lock().unwrap();
            if let Some(v) = unlocked_scope.get(&name) {
                if let DeclType::Immutable = v.1 {
                    return Err(format!("cannot mutate a immutable item '{}'", name));
                }
                unlocked_scope.insert(name.clone(), (value.clone(), DeclType::Mutable));
                return Ok(());
            }
        }

        Err(format!("'{}' is not defined", name))
    }

    fn get(&self, name: &String) -> Option<Value> {
        for scope in self.0.iter().rev() {
            let unlocked_scope = scope.lock().unwrap();
            if let Some(v) = unlocked_scope.get(name) {
                return Some(v.0.clone());
            }
        }

        None
    }
}
