use super::{gotham_state::GothamState, WasmError, WorkerContext};
use bevy::{
    log::{debug, warn},
    platform::collections::HashMap,
};
use dcl_js::storage_address;
use dcl_types::interface::crdt_context::CrdtContext;
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{
    FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemGetDirectoryOptions,
    FileSystemGetFileOptions, FileSystemWritableFileStream, WorkerGlobalScope,
};

// wrap localStorage to include player address in all operations
// TODO: init / store

#[derive(Default, Serialize, Deserialize, Clone)]
struct LocalStorage(HashMap<String, String>);

const STORAGE_DIR: &str = "scene_storage";
const STORAGE_FILE: &str = "data";

// The scene's storage file for the signed-in player, held open for the life of the worker.
//
// Deliberately NOT `web_fs`: that resolves navigator.storage.getDirectory() — the OPFS *root*, which
// also holds config.json and the ipfs cache — on every single open, so it would need OPFS reachable
// from the scene's realm the whole time the scene runs. sandbox_worker.js scrubs the API off the
// worker global instead, and this handle keeps working afterwards because it is a live object rather
// than a fresh lookup. Only the scene's own file: scene code can reach this runtime's memory and so
// steer what it does with anything it holds, so it holds nothing wider.
//
// The file sits alone in a directory of its own (STORAGE_DIR/<scene>/<player>/STORAGE_FILE): what a
// file handle can reach is bounded by its directory, not by the one file.
//
// SCENE_DIR is the one exception to "nothing wider", and only for a scene that started before the
// player signed in: the scene's own directory is kept until the sign-in, so the player's file can
// still be opened. Until then the scene can reach its other players' files, and no other scene's.
thread_local! {
    static FILE: RefCell<Option<FileSystemFileHandle>> = const { RefCell::new(None) };
    static SCENE_DIR: RefCell<Option<FileSystemDirectoryHandle>> = const { RefCell::new(None) };
}

async fn open_dir(
    parent: &FileSystemDirectoryHandle,
    name: &str,
) -> Result<FileSystemDirectoryHandle, JsValue> {
    let options = FileSystemGetDirectoryOptions::new();
    options.set_create(true);
    Ok(
        JsFuture::from(parent.get_directory_handle_with_options(name, &options))
            .await?
            .unchecked_into(),
    )
}

async fn open_scene_dir(scene_urn: &str) -> Result<FileSystemDirectoryHandle, JsValue> {
    let global = js_sys::global().unchecked_into::<WorkerGlobalScope>();
    let root: FileSystemDirectoryHandle =
        JsFuture::from(global.navigator().storage().get_directory())
            .await?
            .unchecked_into();

    open_dir(&open_dir(&root, STORAGE_DIR).await?, scene_urn).await
}

async fn open_file(
    scene_dir: &FileSystemDirectoryHandle,
    address: &str,
) -> Result<FileSystemFileHandle, JsValue> {
    let dir = open_dir(scene_dir, address).await?;

    // created now, as the handle can't be had later
    let options = FileSystemGetFileOptions::new();
    options.set_create(true);
    Ok(
        JsFuture::from(dir.get_file_handle_with_options(STORAGE_FILE, &options))
            .await?
            .unchecked_into(),
    )
}

async fn read(handle: &FileSystemFileHandle) -> Result<Option<String>, JsValue> {
    let file = JsFuture::from(handle.get_file()).await?;
    let text = JsFuture::from(file.unchecked_into::<web_sys::File>().text()).await?;
    // A scene that has never stored anything has an empty file; that isn't an error.
    Ok(text.as_string().filter(|text| !text.is_empty()))
}

async fn store(data: &str) -> Result<(), JsValue> {
    let Some(handle) = FILE.with(|file| file.borrow().clone()) else {
        return Err(JsValue::from_str("scene storage unavailable"));
    };

    let writable: FileSystemWritableFileStream = JsFuture::from(handle.create_writable())
        .await?
        .unchecked_into();
    JsFuture::from(writable.write_with_str(data)?).await?;
    JsFuture::from(writable.close()).await?;
    Ok(())
}

// Opens the player's file and loads what it holds. FILE is set last: the ops raise until it is.
async fn load(
    state: Weak<RefCell<GothamState>>,
    scene_dir: FileSystemDirectoryHandle,
    address: String,
) {
    let file = match open_file(&scene_dir, &address).await {
        Ok(file) => file,
        Err(e) => {
            warn!("failed to open scene storage: {e:?}");
            return;
        }
    };

    match read(&file).await {
        Ok(Some(buf)) => match serde_json::from_str::<LocalStorage>(&buf) {
            Ok(storage) => {
                if let Some(state) = state.upgrade() {
                    state.borrow_mut().put(storage);
                }
            }
            Err(_) => warn!("failed to deserialize storage"),
        },
        Ok(None) => (),
        Err(e) => warn!("failed to read storage: {e:?}"),
    }

    FILE.with(|slot| *slot.borrow_mut() = Some(file));
}

pub async fn init(state: &WorkerContext) {
    let (scene_urn, is_server) = {
        let state = state.state.borrow();
        let context = state.borrow::<CrdtContext>();
        (context.hash.clone(), context.is_server)
    };
    // a scene server never has a player to open a file for
    if is_server {
        return;
    }

    // Ahead of the read: this is what captures the handle, and it has to happen
    // while navigator.storage is still there. wasm_init_scene (our caller) returns before
    // sandbox_worker.js builds the js context and scrubs, so this is the last chance to take it —
    // including for a scene with nothing stored yet, which still needs the handle to write later.
    let scene_dir = match open_scene_dir(&scene_urn).await {
        Ok(scene_dir) => scene_dir,
        Err(e) => {
            warn!("failed to open scene storage: {e:?}");
            return;
        }
    };

    let address = storage_address(&*state.state.borrow());
    match address {
        Ok(address) => load(Rc::downgrade(&state.state), scene_dir, address).await,
        Err(_) => SCENE_DIR.with(|slot| *slot.borrow_mut() = Some(scene_dir)),
    }
}

fn write(state: &WorkerContext) {
    let storage = state.state.borrow().borrow::<LocalStorage>().clone();

    spawn_local(async move {
        let Ok(data) = serde_json::to_string(&storage) else {
            warn!("failed to serialize storage");
            return;
        };

        if let Err(e) = store(&data).await {
            warn!("failed to write storage: {e:?}");
        }
    })
}

fn address(state: &WorkerContext) -> Result<String, WasmError> {
    let address = storage_address(&*state.state.borrow())?;
    if FILE.with(|file| file.borrow().is_none()) {
        // the player signed in after the scene started: open their file, for a later call
        if let Some(scene_dir) = SCENE_DIR.with(|slot| slot.borrow_mut().take()) {
            spawn_local(load(Rc::downgrade(&state.state), scene_dir, address));
        }
        return Err(anyhow::anyhow!(
            "localStorage is not available: the player's storage is not open"
        )
        .into());
    }
    debug!("local storage address: {address:?}");
    Ok(address)
}

fn strip_prefix(key: &mut String) {
    *key = key.split_once(':').unwrap().1.to_owned()
}

fn with_storage<R>(state: &WorkerContext, f: impl Fn(&mut HashMap<String, String>) -> R) -> R {
    f(&mut state
        .state
        .borrow_mut()
        .borrow_mut_or_default::<LocalStorage>()
        .0)
}

// returns filtered keys matching current user, including the prefix
fn iterate_keys(state: &WorkerContext) -> Result<Vec<String>, WasmError> {
    let address = address(state)?;
    let address = &address;
    Ok(with_storage(state, |storage| {
        storage
            .keys()
            .filter(|k| k.starts_with(address))
            .map(ToOwned::to_owned)
            .collect()
    }))
}

#[wasm_bindgen]
pub fn op_webstorage_length(state: &WorkerContext) -> Result<u32, WasmError> {
    Ok(iterate_keys(state)?.len() as u32)
}

#[wasm_bindgen]
pub fn op_webstorage_key(state: &WorkerContext, index: u32) -> Result<Option<String>, WasmError> {
    let mut key = iterate_keys(state)?
        .get(index as usize)
        .map(ToOwned::to_owned);
    key.iter_mut().for_each(strip_prefix);
    Ok(key)
}

#[wasm_bindgen]
pub fn op_webstorage_set(
    state: &WorkerContext,
    key_name: &str,
    value: &str,
) -> Result<(), WasmError> {
    let address = address(state)?;
    with_storage(state, |storage| {
        storage.insert(format!("{address}:{key_name}"), value.to_owned())
    });
    write(state);
    Ok(())
}

#[wasm_bindgen]
pub fn op_webstorage_get(
    state: &WorkerContext,
    key_name: &str,
) -> Result<Option<String>, WasmError> {
    let address = address(state)?;
    Ok(with_storage(state, |storage| {
        storage
            .get(&format!("{address}:{key_name}"))
            .map(ToOwned::to_owned)
    }))
}

#[wasm_bindgen]
pub fn op_webstorage_remove(state: &WorkerContext, key_name: &str) -> Result<(), WasmError> {
    let address = address(state)?;
    with_storage(state, |storage| {
        storage.remove(&format!("{address}:{key_name}"))
    });
    write(state);
    Ok(())
}

#[wasm_bindgen]
pub fn op_webstorage_clear(state: &WorkerContext) -> Result<(), WasmError> {
    let keys = iterate_keys(state)?;
    with_storage(state, move |storage| {
        for key in &keys {
            storage.remove(key);
        }
    });
    write(state);
    Ok(())
}

#[wasm_bindgen]
pub fn op_webstorage_iterate_keys(state: &WorkerContext) -> Result<Vec<String>, WasmError> {
    let mut keys = iterate_keys(state)?;
    keys.iter_mut().for_each(strip_prefix);
    Ok(keys)
}

#[wasm_bindgen]
pub fn op_webstorage_has(state: &WorkerContext, key_name: &str) -> Result<bool, WasmError> {
    let address = address(state)?;
    Ok(with_storage(state, |storage| {
        storage.contains_key(&format!("{address}:{key_name}"))
    }))
}
