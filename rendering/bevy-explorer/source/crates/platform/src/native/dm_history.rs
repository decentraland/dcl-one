//! Local DM history on native: one directory per account under the app data dir, one file per
//! conversation. Contents are AES-256-CBC with a key
//! derived from the account address and a fresh IV per record, and names are hashes, so nothing
//! on disk is readable or attributable without knowing the account. That is obfuscation, not
//! secrecy: the account address is public.

use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    sync::Mutex,
};

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use sha2::{Digest, Sha256};

use crate::DmHistoryEntry;

type Encryptor = cbc::Encryptor<aes::Aes256>;
type Decryptor = cbc::Decryptor<aes::Aes256>;

/// Salt for the account directory name.
const ACCOUNT_DIR: &str = "account";

/// Serializes all history file access; appends from several tasks must not interleave.
static LOCK: Mutex<()> = Mutex::new(());

fn key(account: &str) -> [u8; 32] {
    Sha256::digest(account.to_lowercase().as_bytes()).into()
}

fn hashed_name(key: &[u8; 32], name: &str) -> String {
    let digest = Sha256::digest([key.as_slice(), name.to_lowercase().as_bytes()].concat());
    digest[..12].iter().map(|b| format!("{b:02x}")).collect()
}

fn account_dir(key: &[u8; 32]) -> Result<PathBuf, String> {
    let dirs = super::project_directories().ok_or("no project directories")?;
    Ok(dirs
        .data_dir()
        .join("dm")
        .join(hashed_name(key, ACCOUNT_DIR)))
}

fn encrypt_record(key: &[u8; 32], plain: &[u8]) -> String {
    let iv: [u8; 16] = rand::random();
    let cipher = Encryptor::new(key.into(), &iv.into()).encrypt_padded_vec_mut::<Pkcs7>(plain);
    BASE64.encode([iv.as_slice(), &cipher].concat())
}

fn decrypt_record(key: &[u8; 32], line: &str) -> Option<Vec<u8>> {
    let bytes = BASE64.decode(line.trim()).ok()?;
    let (iv, cipher) = bytes.split_at_checked(16)?;
    Decryptor::new(key.into(), iv.into())
        .decrypt_padded_vec_mut::<Pkcs7>(cipher)
        .ok()
}

fn read_records<T: serde::de::DeserializeOwned>(
    key: &[u8; 32],
    path: &PathBuf,
) -> Result<Vec<T>, String> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    Ok(BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter(|line| !line.is_empty())
        .filter_map(|line| decrypt_record(key, &line))
        .filter_map(|plain| serde_json::from_slice(&plain).ok())
        .collect())
}

fn append_record<T: serde::Serialize>(
    key: &[u8; 32],
    path: &PathBuf,
    record: &T,
) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let plain = serde_json::to_vec(record).map_err(|e| e.to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    writeln!(file, "{}", encrypt_record(key, &plain)).map_err(|e| e.to_string())
}

pub async fn dm_history_append(
    account: &str,
    partner: &str,
    entry: &DmHistoryEntry,
) -> Result<(), String> {
    let key = key(account);
    let path = account_dir(&key)?.join(hashed_name(&key, partner));
    let _guard = LOCK.lock().map_err(|e| e.to_string())?;
    append_record(&key, &path, entry)
}

pub async fn dm_history_read(account: &str, partner: &str) -> Result<Vec<DmHistoryEntry>, String> {
    let key = key(account);
    let path = account_dir(&key)?.join(hashed_name(&key, partner));
    let _guard = LOCK.lock().map_err(|e| e.to_string())?;
    read_records(&key, &path)
}

/// The stored lines to keep so that the last `keep` messages stay, with every record after the
/// oldest of them (the reactions to them); `None` when nothing has to go. A line that does not
/// decrypt to a record counts as a message, so a damaged line never drops its neighbours.
fn trimmed_lines(key: &[u8; 32], lines: &[String], keep: usize) -> Option<Vec<String>> {
    let is_message = |line: &String| {
        decrypt_record(key, line)
            .and_then(|plain| serde_json::from_slice::<DmHistoryEntry>(&plain).ok())
            .is_none_or(|entry| entry.reaction.is_none())
    };
    if keep == 0 {
        return (!lines.is_empty()).then(Vec::new);
    }
    // the oldest message to keep: the `keep`-th from the end
    let mut seen = 0;
    let first_kept = lines.iter().rposition(|line| {
        if !is_message(line) {
            return false;
        }
        seen += 1;
        seen == keep
    })?;
    (first_kept > 0).then(|| lines[first_kept..].to_vec())
}

/// Keeps only the last `keep` messages of a conversation, and the reactions to them.
pub async fn dm_history_trim(account: &str, partner: &str, keep: usize) -> Result<(), String> {
    let key = key(account);
    let path = account_dir(&key)?.join(hashed_name(&key, partner));
    let _guard = LOCK.lock().map_err(|e| e.to_string())?;
    let lines: Vec<String> = match std::fs::read_to_string(&path) {
        Ok(contents) => contents
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    let Some(kept) = trimmed_lines(&key, &lines, keep) else {
        return Ok(());
    };
    // written whole, then renamed over the old file, so a crash leaves one or the other
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, kept.join("\n") + "\n").map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

pub async fn dm_history_delete(account: &str, partner: &str) -> Result<(), String> {
    let key = key(account);
    let path = account_dir(&key)?.join(hashed_name(&key, partner));
    let _guard = LOCK.lock().map_err(|e| e.to_string())?;
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DmHistoryReaction;

    fn message(key: &[u8; 32], n: u32) -> String {
        let entry = DmHistoryEntry {
            from: "0xa".to_owned(),
            message: format!("m{n}"),
            received_at: n as f64,
            sent_at: Some(n as f64),
            reaction: None,
        };
        encrypt_record(key, &serde_json::to_vec(&entry).unwrap())
    }

    fn reaction(key: &[u8; 32], to: u32) -> String {
        let entry = DmHistoryEntry {
            from: "0xb".to_owned(),
            message: String::new(),
            received_at: to as f64 + 0.5,
            sent_at: None,
            reaction: Some(DmHistoryReaction {
                message_id: format!("0xa:{to}"),
                emoji: "\u{1f44d}".to_owned(),
                remove: false,
            }),
        };
        encrypt_record(key, &serde_json::to_vec(&entry).unwrap())
    }

    #[test]
    fn the_trim_keeps_the_last_messages_and_their_reactions() {
        let key = key("0xaccount");
        // m1 r1 m2 r2 r2 m3 r3 m4: keeping 2 messages keeps m3 r3 m4, and orphans r1 r2 go
        let lines = vec![
            message(&key, 1),
            reaction(&key, 1),
            message(&key, 2),
            reaction(&key, 2),
            reaction(&key, 2),
            message(&key, 3),
            reaction(&key, 3),
            message(&key, 4),
        ];
        assert_eq!(trimmed_lines(&key, &lines, 2), Some(lines[5..].to_vec()));
        assert_eq!(trimmed_lines(&key, &lines, 4), None);
        assert_eq!(trimmed_lines(&key, &lines, 0), Some(Vec::new()));
        // a line that is not a record counts as a message
        let mut with_junk = lines.clone();
        with_junk.push("not base64!".to_owned());
        assert_eq!(
            trimmed_lines(&key, &with_junk, 1),
            Some(vec!["not base64!".to_owned()])
        );
    }
}
