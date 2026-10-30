use super::AppState;
use axum::{extract::State, http::StatusCode};
use serde_json::Value;
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

type Failure = (StatusCode, String);
const MAX_LINE: usize = 64 * 1024;
const MAX_RECORDS: usize = 1000;
pub(super) const FILE: &str = "assistant-utterances.jsonl";

/// The hands-free assistant's utterance records, training data for its
/// end-of-turn model, one JSON object per line under `.dcl-one/`. The file
/// is append-only: a record's later states are re-posted whole under the
/// same `id`, so its last line is current and [`Log::latest`] folds them.
/// The SDK stores what the page sends and reads none of it.
pub(super) struct Log {
    path: PathBuf,
}

fn failure(error: impl std::fmt::Display) -> Failure {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Assistant utterances: {error}"),
    )
}

impl Log {
    pub fn open(root: &Path) -> Result<Self, Failure> {
        let dir = root.join(".dcl-one");
        if std::fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(failure("project state directory is a symlink"));
        }
        let dir = crate::scene::work_dir(root).map_err(failure)?;
        let path = dir.join(FILE);
        if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(failure("utterance log is a symlink"));
        }
        Ok(Self { path })
    }

    pub fn upsert(&self, records: &[Value]) -> Result<(), Failure> {
        let mut lines = String::new();
        for record in records {
            lines.push_str(&serde_json::to_string(record).map_err(failure)?);
            lines.push('\n');
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(failure)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))
                .map_err(failure)?;
        }
        file.write_all(lines.as_bytes()).map_err(failure)
    }

    /// Every record's current state, in first-seen order.
    #[cfg(test)]
    pub fn latest(&self) -> Result<Vec<Value>, Failure> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(failure(e)),
        };
        let mut order = Vec::new();
        let mut by_id = std::collections::HashMap::<String, Value>::new();
        for line in text.lines() {
            let Ok(record) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let Some(id) = record["id"].as_str().map(str::to_owned) else {
                continue;
            };
            if !by_id.contains_key(&id) {
                order.push(id.clone());
            }
            by_id.insert(id, record);
        }
        Ok(order
            .into_iter()
            .filter_map(|id| by_id.remove(&id))
            .collect())
    }
}

/// NDJSON records, each an object with a valid `id`; blank lines are skipped.
pub(super) fn parse(body: &str) -> Result<Vec<Value>, Failure> {
    let bad = |what: &str| {
        (
            StatusCode::BAD_REQUEST,
            format!("Utterance records: {what}"),
        )
    };
    let mut records = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.len() > MAX_LINE {
            return Err(bad("a record exceeds 64 KiB"));
        }
        let record: Value = serde_json::from_str(line).map_err(|e| bad(&e.to_string()))?;
        if !record.is_object() {
            return Err(bad("each line must be a JSON object"));
        }
        if !record["id"]
            .as_str()
            .is_some_and(super::assistant::identifier)
        {
            return Err(bad("each record needs a plain `id`"));
        }
        records.push(record);
        if records.len() > MAX_RECORDS {
            return Err(bad("at most 1000 records per request"));
        }
    }
    Ok(records)
}

pub(super) async fn upsert(
    State(st): State<Arc<AppState>>,
    body: String,
) -> Result<StatusCode, Failure> {
    let project = super::project_bridge::single_project(&st)?;
    let records = parse(&body)?;
    if !records.is_empty() {
        Log::open(&project.root)?.upsert(&records)?;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
#[path = "assistant_utterances_tests.rs"]
mod tests;
