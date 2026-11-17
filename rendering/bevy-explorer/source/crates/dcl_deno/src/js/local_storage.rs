// `localStorage` for scenes: deno_webstorage 0.163's sqlite store (MIT, the Deno authors),
// with the player's address prefixed to every key. Its errors are plain `Error`s, as before.

use std::path::PathBuf;

use bevy::log::debug;
use dcl_js::storage_address;
use deno_core::{op2, OpDecl, OpState};
use deno_error::JsErrorBox;
use rusqlite::{params, Connection, OptionalExtension};

use super::errors::OpError;

/// The folder that holds a scene's `local_storage` database.
pub struct LocalStorageDir(pub PathBuf);

const MAX_STORAGE_BYTES: usize = 10 * 1024 * 1024;

pub fn ops() -> Vec<OpDecl> {
    vec![
        op_webstorage_length(),
        op_webstorage_key(),
        op_webstorage_set(),
        op_webstorage_get(),
        op_webstorage_remove(),
        op_webstorage_clear(),
        op_webstorage_iterate_keys(),
    ]
}

struct LocalStorage(Connection);
struct SessionStorage(Connection);

fn sqlite_error(error: rusqlite::Error) -> OpError {
    JsErrorBox::generic(error.to_string()).into()
}

fn get_webstorage(state: &mut OpState, persistent: bool) -> Result<&Connection, OpError> {
    if persistent {
        if state.try_borrow::<LocalStorage>().is_none() {
            let path = state.try_borrow::<LocalStorageDir>().ok_or_else(|| {
                JsErrorBox::generic("LocalStorage is not supported in this context.")
            })?;
            std::fs::create_dir_all(&path.0).map_err(|e| JsErrorBox::generic(e.to_string()))?;
            let conn = Connection::open(path.0.join("local_storage")).map_err(sqlite_error)?;
            // Enable write-ahead-logging and tweak some other stuff.
            let initial_pragmas = "
              -- enable write-ahead-logging mode
              PRAGMA journal_mode=WAL;
              PRAGMA synchronous=NORMAL;
              PRAGMA temp_store=memory;
              PRAGMA page_size=4096;
              PRAGMA mmap_size=6000000;
              PRAGMA optimize;
            ";
            conn.execute_batch(initial_pragmas).map_err(sqlite_error)?;
            conn.set_prepared_statement_cache_capacity(128);
            conn.prepare_cached(
                "CREATE TABLE IF NOT EXISTS data (key VARCHAR UNIQUE, value VARCHAR)",
            )
            .and_then(|mut stmt| stmt.execute(params![]))
            .map_err(sqlite_error)?;
            state.put(LocalStorage(conn));
        }
        Ok(&state.borrow::<LocalStorage>().0)
    } else {
        if state.try_borrow::<SessionStorage>().is_none() {
            let conn = Connection::open_in_memory().map_err(sqlite_error)?;
            conn.prepare_cached("CREATE TABLE data (key VARCHAR UNIQUE, value VARCHAR)")
                .and_then(|mut stmt| stmt.execute(params![]))
                .map_err(sqlite_error)?;
            state.put(SessionStorage(conn));
        }
        Ok(&state.borrow::<SessionStorage>().0)
    }
}

fn size_check(input: usize) -> Result<(), OpError> {
    if input >= MAX_STORAGE_BYTES {
        return Err(JsErrorBox::generic("Exceeded maximum storage size").into());
    }
    Ok(())
}

fn address(state: &OpState) -> Result<String, OpError> {
    let address = storage_address(state)?;
    debug!("local storage address: {address:?}");
    Ok(address)
}

fn strip_prefix(key: impl AsRef<str>) -> String {
    key.as_ref()
        .split_once(':')
        .map(|(_, key)| key.to_owned())
        .unwrap_or_default()
}

/// Every stored key, with its address prefix.
fn all_keys(state: &mut OpState, persistent: bool) -> Result<Vec<String>, OpError> {
    let conn = get_webstorage(state, persistent)?;
    let mut stmt = conn
        .prepare_cached("SELECT key FROM data")
        .map_err(sqlite_error)?;
    let keys = stmt
        .query_map(params![], |row| row.get::<_, String>(0))
        .map_err(sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sqlite_error)?;
    Ok(keys)
}

/// The current player's keys, with the prefix.
fn iterate_keys(
    state: &mut OpState,
    persistent: bool,
) -> Result<impl Iterator<Item = String>, OpError> {
    let address = address(state)?;
    let keys = all_keys(state, persistent)?;
    Ok(keys.into_iter().filter(move |k| k.starts_with(&address)))
}

fn set(state: &mut OpState, key: &str, value: &str, persistent: bool) -> Result<(), OpError> {
    let conn = get_webstorage(state, persistent)?;
    size_check(key.len() + value.len())?;
    let size: u32 = conn
        .prepare_cached("SELECT SUM(pgsize) FROM dbstat WHERE name = 'data'")
        .and_then(|mut stmt| stmt.query_row(params![], |row| row.get(0)))
        .map_err(sqlite_error)?;
    size_check(size as usize)?;
    conn.prepare_cached("INSERT OR REPLACE INTO data (key, value) VALUES (?, ?)")
        .and_then(|mut stmt| stmt.execute(params![key, value]))
        .map_err(sqlite_error)?;
    Ok(())
}

fn get(state: &mut OpState, key: &str, persistent: bool) -> Result<Option<String>, OpError> {
    let conn = get_webstorage(state, persistent)?;
    conn.prepare_cached("SELECT value FROM data WHERE key = ?")
        .and_then(|mut stmt| stmt.query_row(params![key], |row| row.get(0)).optional())
        .map_err(sqlite_error)
}

fn remove(state: &mut OpState, key: &str, persistent: bool) -> Result<(), OpError> {
    let conn = get_webstorage(state, persistent)?;
    conn.prepare_cached("DELETE FROM data WHERE key = ?")
        .and_then(|mut stmt| stmt.execute(params![key]))
        .map_err(sqlite_error)?;
    Ok(())
}

#[op2(fast)]
pub fn op_webstorage_length(state: &mut OpState, persistent: bool) -> Result<u32, OpError> {
    Ok(iterate_keys(state, persistent)?.count() as u32)
}

#[op2]
#[string]
pub fn op_webstorage_key(
    state: &mut OpState,
    #[smi] index: u32,
    persistent: bool,
) -> Result<Option<String>, OpError> {
    Ok(iterate_keys(state, persistent)?
        .nth(index as usize)
        .map(strip_prefix))
}

#[op2(fast)]
pub fn op_webstorage_set(
    state: &mut OpState,
    #[string] key: &str,
    #[string] value: &str,
    persistent: bool,
) -> Result<(), OpError> {
    let address = address(state)?;
    set(state, &format!("{address}:{key}"), value, persistent)
}

#[op2]
#[string]
pub fn op_webstorage_get(
    state: &mut OpState,
    #[string] key_name: String,
    persistent: bool,
) -> Result<Option<String>, OpError> {
    let address = address(state)?;
    get(state, &format!("{address}:{key_name}"), persistent)
}

#[op2(fast)]
pub fn op_webstorage_remove(
    state: &mut OpState,
    #[string] key_name: &str,
    persistent: bool,
) -> Result<(), OpError> {
    let address = address(state)?;
    remove(state, &format!("{address}:{key_name}"), persistent)
}

#[op2(fast)]
pub fn op_webstorage_clear(state: &mut OpState, persistent: bool) -> Result<(), OpError> {
    for key in iterate_keys(state, persistent)?.collect::<Vec<_>>() {
        remove(state, &key, persistent)?;
    }
    Ok(())
}

#[op2]
#[serde]
pub fn op_webstorage_iterate_keys(
    state: &mut OpState,
    persistent: bool,
) -> Result<Vec<String>, OpError> {
    Ok(iterate_keys(state, persistent)?.map(strip_prefix).collect())
}
