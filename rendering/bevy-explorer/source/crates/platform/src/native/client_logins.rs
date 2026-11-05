//! Cross-process mutations of the login queue. The lock is never waited on by an IO worker.
use std::{collections::BTreeMap, fs::OpenOptions, path::Path};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client_login_store::{valid_event, MAX_AGE_MS, MAX_PENDING, TOMBSTONE_MS};

#[derive(Default, Serialize, Deserialize)]
struct Store {
    events: BTreeMap<String, Value>,
    removed: BTreeMap<String, u64>,
}

#[derive(Deserialize)]
struct Changes {
    additions: Vec<Value>,
    removals: Vec<String>,
}

fn read_store(path: &Path) -> Result<Store, String> {
    let text = match std::fs::read(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Store::default()),
        Err(error) => return Err(error.to_string()),
    };
    let value = serde_json::from_slice::<Value>(&text).ok();
    let mut store = Store::default();
    let mut damaged = false;
    let entries: Vec<(String, Value)> = match value.as_ref() {
        Some(Value::Array(events)) => events
            .iter()
            .map(|event| {
                (
                    event["login_id"].as_str().unwrap_or_default().to_owned(),
                    event.clone(),
                )
            })
            .collect(),
        Some(Value::Object(object)) => {
            match object.get("removed").and_then(Value::as_object) {
                Some(removed) => {
                    for (id, at) in removed {
                        if let Some(at) = at.as_u64() {
                            store.removed.insert(id.clone(), at);
                        } else {
                            damaged = true;
                        }
                    }
                }
                None => damaged = true,
            }
            match object.get("events").and_then(Value::as_object) {
                Some(events) => events
                    .iter()
                    .map(|(id, event)| (id.clone(), event.clone()))
                    .collect(),
                None => {
                    damaged = true;
                    Vec::new()
                }
            }
        }
        _ => {
            damaged = true;
            Vec::new()
        }
    };
    for (id, event) in entries {
        if valid_event(&event) && event["login_id"].as_str() == Some(id.as_str()) {
            store.events.insert(id, event);
        } else {
            damaged = true;
        }
    }
    if damaged {
        // Keep one bounded recovery snapshot, never log its contents.
        std::fs::copy(path, path.with_extension("json.corrupt")).map_err(|e| e.to_string())?;
        bevy::log::warn!("recovered damaged login event storage; invalid records quarantined");
    }
    Ok(store)
}

fn prune(store: &mut Store, now: u64) {
    store
        .removed
        .retain(|_, at| now.saturating_sub(*at) < TOMBSTONE_MS);
    let expired = store
        .events
        .iter()
        .filter(|(_, event)| {
            event["occurred_at"]
                .as_u64()
                .is_none_or(|at| now.saturating_sub(at) >= MAX_AGE_MS)
        })
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    for id in expired {
        store.events.remove(&id);
        store.removed.insert(id, now);
    }
    let mut oldest = store
        .events
        .iter()
        .map(|(id, event)| (event["occurred_at"].as_u64().unwrap(), id.clone()))
        .collect::<Vec<_>>();
    oldest.sort();
    for (_, id) in oldest
        .into_iter()
        .take(store.events.len().saturating_sub(MAX_PENDING))
    {
        store.events.remove(&id);
        store.removed.insert(id, now);
    }
}

fn locked<T>(
    directory: &Path,
    action: impl FnOnce(&Path) -> Result<T, String>,
) -> Result<T, String> {
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join("client-logins.lock"))
        .map_err(|e| e.to_string())?;
    lock.try_lock()
        .map_err(|e| format!("login queue busy: {e}"))?;
    action(&directory.join("client-logins.json"))
}

pub(super) fn read(directory: &Path, now: u64) -> Result<String, String> {
    locked(directory, |path| {
        let mut store = read_store(path)?;
        prune(&mut store, now);
        // Pruning is a durable removal, so another process cannot reinsert a capped record.
        super::write_atomically(
            path,
            &serde_json::to_vec(&store).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        serde_json::to_string(&store.events.values().collect::<Vec<_>>()).map_err(|e| e.to_string())
    })
}

pub(super) fn write(directory: &Path, json: &str, now: u64) -> Result<String, String> {
    let changes: Changes = serde_json::from_str(json).map_err(|e| e.to_string())?;
    locked(directory, |path| {
        let mut store = read_store(path)?;
        prune(&mut store, now);
        for event in changes.additions {
            if !valid_event(&event) {
                return Err("invalid login event".into());
            }
            let id = event["login_id"]
                .as_str()
                .ok_or("invalid login ID")?
                .to_owned();
            if !store.removed.contains_key(&id) {
                store.events.entry(id).or_insert(event);
            }
        }
        for id in changes.removals {
            store.events.remove(&id);
            store.removed.insert(id, now);
        }
        prune(&mut store, now);
        super::write_atomically(
            path,
            &serde_json::to_vec(&store).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        serde_json::to_string(&store.events.keys().collect::<Vec<_>>()).map_err(|e| e.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn change(id: &str, at: u64) -> String {
        serde_json::json!({"additions":[{"login_id":id,"occurred_at":at,"wallet":"alice","endpoint":"https://example.org/v1/client-login"}],"removals":[]})
            .to_string()
    }
    fn directory() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "dcl-login-store-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    #[test]
    fn empty_and_mixed_stores_quarantine_only_invalid_records() {
        let dir = directory();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("client-logins.json");
        for text in ["", "{", "null", r#"{"events":null,"removed":{}}"#] {
            std::fs::write(&path, text).unwrap();
            assert_eq!(read(&dir, 1000).unwrap(), "[]");
            assert_eq!(
                std::fs::read_to_string(dir.join("client-logins.json.corrupt")).unwrap(),
                text
            );
        }
        std::fs::write(&path, [0xff, 0xfe]).unwrap();
        assert_eq!(read(&dir, 1000).unwrap(), "[]");
        assert_eq!(
            std::fs::read(dir.join("client-logins.json.corrupt")).unwrap(),
            [0xff, 0xfe]
        );
        let valid: Value = serde_json::from_str(&change("good", 1000)).unwrap();
        let mixed =
            serde_json::json!([valid["additions"][0], null, {"login_id":"bad","occurred_at":1000}])
                .to_string();
        std::fs::write(&path, &mixed).unwrap();
        let recovered: Vec<Value> = serde_json::from_str(&read(&dir, 1000).unwrap()).unwrap();
        assert_eq!(recovered, vec![valid["additions"][0].clone()]);
        assert_eq!(
            std::fs::read_to_string(dir.join("client-logins.json.corrupt")).unwrap(),
            mixed
        );
        write(&dir, &change("new", 1000), 1000).unwrap();
        let recovered: Vec<Value> = serde_json::from_str(&read(&dir, 1000).unwrap()).unwrap();
        assert_eq!(recovered.len(), 2);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn independent_instances_merge_restart_and_ack_without_resurrection() {
        let dir = directory();
        assert_eq!(read(&dir, 1000).unwrap(), "[]");
        assert_eq!(read(&dir, 1000).unwrap(), "[]");
        write(&dir, &change("a", 1000), 1000).unwrap();
        write(&dir, &change("b", 1000), 1000).unwrap();
        let restarted: Vec<Value> = serde_json::from_str(&read(&dir, 1001).unwrap()).unwrap();
        assert_eq!(restarted.len(), 2);
        write(&dir, r#"{"additions":[],"removals":["a"]}"#, 1001).unwrap();
        // An uncertain earlier save retries its addition after another process acknowledged it.
        write(&dir, &change("a", 1000), 500).unwrap(); // clock rollback retains tombstone
        write(&dir, &change("a", 1000), 1002).unwrap();
        let restarted: Vec<Value> = serde_json::from_str(&read(&dir, 1002).unwrap()).unwrap();
        assert_eq!(restarted.len(), 1);
        assert_eq!(restarted[0]["login_id"], "b");
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn locked_or_failed_save_is_retryable_and_store_bound_is_global() {
        let dir = directory();
        read(&dir, 1000).unwrap();
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(dir.join("client-logins.lock"))
            .unwrap();
        lock.lock().unwrap();
        assert!(write(&dir, &change("blocked", 1000), 1000)
            .unwrap_err()
            .contains("busy"));
        drop(lock);
        write(&dir, &change("kept", 1000), 1000).unwrap();
        // An unusable staging path fails atomic replacement and leaves the prior file intact.
        std::fs::create_dir(dir.join("client-logins.json.tmp")).unwrap();
        assert!(write(&dir, &change("failed", 1000), 1000).is_err());
        std::fs::remove_dir(dir.join("client-logins.json.tmp")).unwrap();
        let retained: Vec<Value> = serde_json::from_str(&read(&dir, 1000).unwrap()).unwrap();
        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0]["login_id"], "kept");
        for i in 0..40 {
            write(&dir, &change(&i.to_string(), 1000 + i), 1040).unwrap();
        }
        let rows: Vec<Value> = serde_json::from_str(&read(&dir, 1040).unwrap()).unwrap();
        assert_eq!(rows.len(), 32);
        assert!(!rows.iter().any(|event| event["login_id"] == "0"));
        assert_eq!(read(&dir, MAX_AGE_MS + 1040).unwrap(), "[]");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
