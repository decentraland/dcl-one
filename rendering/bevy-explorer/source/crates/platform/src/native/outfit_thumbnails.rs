//! Saved-outfit pictures on native: one file per slot under the app data dir, in a directory per
//! account named by a hash of the address.

use std::path::PathBuf;

use sha2::{Digest, Sha256};

use crate::OutfitThumbnail;

fn account_dir(account: &str) -> Result<PathBuf, String> {
    let dirs = super::project_directories().ok_or("no project directories")?;
    let digest = Sha256::digest(account.to_lowercase().as_bytes());
    let name: String = digest[..12].iter().map(|b| format!("{b:02x}")).collect();
    Ok(dirs.data_dir().join("outfit-thumbnails").join(name))
}

pub async fn outfit_thumbnails_read(account: &str) -> Result<Vec<OutfitThumbnail>, String> {
    let dir = account_dir(account)?;
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let mut thumbnails = Vec::new();
    for entry in entries.flatten() {
        let Some(slot) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        if let Ok(thumbnail) = std::fs::read_to_string(entry.path()) {
            thumbnails.push(OutfitThumbnail { slot, thumbnail });
        }
    }
    thumbnails.sort_by_key(|t| t.slot);
    Ok(thumbnails)
}

pub async fn outfit_thumbnail_write(
    account: &str,
    slot: u32,
    thumbnail: Option<&str>,
) -> Result<(), String> {
    let path = account_dir(account)?.join(slot.to_string());
    match thumbnail {
        Some(thumbnail) => {
            std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
            let partial = path.with_extension("partial");
            std::fs::write(&partial, thumbnail).map_err(|e| e.to_string())?;
            std::fs::rename(&partial, &path).map_err(|e| e.to_string())
        }
        None => match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        },
    }
}
