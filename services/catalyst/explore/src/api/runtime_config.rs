//! The explorer-api's runtime key/value map: process state an operator sets
//! through the console (`explorer-api/config-{list,get,set,delete}`). No
//! route or module reads it; the old service kept it the same way, behind
//! its bearer-guarded `/admin/config/*`, which are the console calls now.

use std::collections::BTreeMap;

use parking_lot::RwLock;
use serde_json::Value;

#[derive(Default)]
pub struct RuntimeConfigState {
    inner: RwLock<BTreeMap<String, Value>>,
}

impl RuntimeConfigState {
    pub fn snapshot(&self) -> BTreeMap<String, Value> {
        self.inner.read().clone()
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        self.inner.read().get(key).cloned()
    }

    pub fn set(&self, key: String, value: Value) {
        self.inner.write().insert(key, value);
    }

    pub fn remove(&self, key: &str) -> Option<Value> {
        self.inner.write().remove(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn set_get_list_remove() {
        let c = RuntimeConfigState::default();
        assert!(c.get("a").is_none());
        c.set("b".into(), json!(2));
        c.set("a".into(), json!({ "x": 1 }));
        assert_eq!(c.get("a"), Some(json!({ "x": 1 })));
        assert_eq!(c.snapshot().keys().collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(c.remove("a"), Some(json!({ "x": 1 })));
        assert!(c.remove("a").is_none());
    }
}
