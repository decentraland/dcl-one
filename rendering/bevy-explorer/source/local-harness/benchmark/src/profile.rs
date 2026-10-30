//! The Chrome profile is one per laptop (it keeps the local guest) and outlives every run, and
//! every build is served from the same origin, so what one build leaves in it must not reach the
//! next. Before each run, with Chrome closed, the folders where a page can leave code or data are
//! deleted from disk: a service worker registered by an earlier build would answer the reset
//! page's navigation itself and never be unregistered, so the reset page alone is not enough. The
//! reset page then clears local storage (but for the guest's root key), session storage and
//! cookies.

use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use crate::failure::Failure;
use crate::out;
use crate::platform;

/// Under `<profile>/Default`: service workers, the HTTP and code caches, IndexedDB, the file
/// system (OPFS), the storage buckets (`WebStorage`: what a page keeps in a bucket other than the
/// default one, and the quota database that lists every origin's buckets, which Chrome rebuilds),
/// shared dictionaries, blobs, session storage, cookies.
const LEFTOVERS: [&[&str]; 11] = [
    &["Service Worker"],
    &["Cache"],
    &["Code Cache"],
    &["IndexedDB"],
    &["File System"],
    &["WebStorage"],
    &["Shared Dictionary"],
    &["blob_storage"],
    &["Session Storage"],
    &["Network", "Cookies"],
    &["Network", "Cookies-journal"],
];

/// The folder of the profile Chrome is started with (`--profile-directory`).
pub const PROFILE_DIRECTORY: &str = "Default";

pub fn scrub(profile: &Path) -> Result<(), Failure> {
    let home = profile.join(PROFILE_DIRECTORY);
    let mut removed = 0;

    for parts in LEFTOVERS {
        let path = parts
            .iter()
            .fold(home.clone(), |path, part| path.join(part));
        if !path.exists() {
            continue;
        }
        delete(&path)?;
        removed += 1;
    }

    out::log(format!(
        "profile scrubbed: {removed} storage folder(s) or file(s) deleted from {}",
        profile
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default()
    ));
    Ok(())
}

fn delete(path: &Path) -> Result<(), Failure> {
    for attempt in 1.. {
        let result = if path.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        };
        match result {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) if attempt >= 5 => {
                return Err(Failure::cannot_run(format!(
                "cannot delete '{}' from the Chrome profile ({}); is a Chrome still running on it?",
                path.display(),
                out::safe(&error.to_string())
            )))
            }
            // Something (a scanner, a Chrome process still going down) holds it for a moment.
            Err(_) => std::thread::sleep(Duration::from_millis(500)),
        }
    }
    unreachable!("the loop returns")
}

/// An earlier job killed mid-run leaves its Chrome (and the page under test) running on the
/// profile, which keeps the profile locked and the page alive. Any process started with this
/// profile as its `--user-data-dir` is closed; other Chrome windows on the machine are not
/// touched.
pub fn close_strays(profile: &Path) -> Result<(), Failure> {
    let stray = running(profile)?;
    if stray.is_empty() {
        return Ok(());
    }

    out::log(format!(
        "{} Chrome process(es) still run on the profile (an earlier run cut short); closing them",
        stray.len()
    ));
    for pid in stray {
        platform::kill_tree(pid);
    }

    for attempt in 0.. {
        if running(profile)?.is_empty() {
            return Ok(());
        }
        if attempt >= 20 {
            return Err(Failure::cannot_run(
                "a Chrome is still running on the profile and cannot be stopped; close it",
            ));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    unreachable!("the loop returns")
}

pub fn running(profile: &Path) -> Result<Vec<u32>, Failure> {
    let me = std::process::id();
    Ok(platform::chrome_processes()?
        .into_iter()
        .filter(|process| process.pid != me && uses_profile(&process.command, profile))
        .map(|process| process.pid)
        .collect())
}

/// `--user-data-dir=<profile>` as one argument (quotes dropped: an argument with a space is
/// quoted), preceded by the start or white space and followed by the end or the next argument.
pub fn uses_profile(command: &str, profile: &Path) -> bool {
    const SWITCH: &str = "--user-data-dir=";
    let command = command.replace('"', "");
    let profile = profile.to_string_lossy();
    let wanted = profile.trim_end_matches(['/', '\\']);

    command.match_indices(SWITCH).any(|(at, _)| {
        let before_ok = command[..at]
            .chars()
            .next_back()
            .is_none_or(char::is_whitespace);
        let rest = &command[at + SWITCH.len()..];
        let Some(candidate) = rest.get(..wanted.len()) else {
            return false;
        };
        let same = if cfg!(windows) {
            candidate.eq_ignore_ascii_case(wanted)
        } else {
            candidate == wanted
        };
        let after = rest[wanted.len()..].trim_start_matches(['/', '\\']);
        before_ok && same && after.chars().next().is_none_or(char::is_whitespace)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn a_command_line_names_the_profile_as_one_whole_argument() {
        let profile = Path::new("/home/a/chrome-profile");
        assert!(uses_profile(
            "chrome --user-data-dir=/home/a/chrome-profile --no-first-run",
            profile
        ));
        assert!(uses_profile(
            "chrome --no-first-run --user-data-dir=/home/a/chrome-profile",
            profile
        ));
        assert!(uses_profile(
            "chrome --user-data-dir=/home/a/chrome-profile/ x",
            profile
        ));
        assert!(uses_profile(
            "\"chrome\" \"--user-data-dir=/home/a/chrome-profile\" x",
            profile
        ));
        assert!(uses_profile(
            "chrome --user-data-dir=/home/a/chrome-profile//",
            profile
        ));

        for other in [
            "chrome --user-data-dir=/home/a/chrome-profile2",
            "chrome --user-data-dir=/home/a/chrome-profile-copy x",
            "chrome --user-data-dir=/home/a/other",
            "chrome --user-data-dir=/home/a",
            "chrome --user-data-dir=",
            "chrome x--user-data-dir=/home/a/chrome-profile",
            "chrome",
            "",
        ] {
            assert!(!uses_profile(other, profile), "{other}");
        }
    }

    #[test]
    fn a_profile_path_with_a_space_is_found_inside_its_quotes() {
        let profile = Path::new("/Users/a b/Library/Application Support/x/chrome-profile");
        assert!(uses_profile(
            "\"--user-data-dir=/Users/a b/Library/Application Support/x/chrome-profile\" --y",
            profile
        ));
    }

    #[cfg(windows)]
    #[test]
    fn windows_paths_compare_without_case_and_with_either_slash() {
        let profile = Path::new(r"C:\Users\A\chrome-profile");
        assert!(uses_profile(
            r"chrome.exe --user-data-dir=c:\users\a\CHROME-PROFILE --x",
            profile
        ));
        assert!(uses_profile(
            r"chrome.exe --user-data-dir=C:\Users\A\chrome-profile\ --x",
            profile
        ));
    }

    fn scratch(name: &str) -> PathBuf {
        crate::scratch("profile", name)
    }

    #[test]
    fn scrub_deletes_what_a_page_can_leave_and_keeps_the_rest() {
        let profile = scratch("scrub");
        let home = profile.join(PROFILE_DIRECTORY);
        for folder in [
            "Service Worker/ScriptCache",
            "Cache/Cache_Data",
            "Code Cache/js",
            "IndexedDB/http_localhost_18742.indexeddb.leveldb",
            "File System/000",
            "WebStorage/1",
            "Shared Dictionary",
            "blob_storage/x",
            "Session Storage",
            "Local Storage/leveldb",
            "GPUCache",
        ] {
            fs::create_dir_all(home.join(folder)).unwrap();
            fs::write(home.join(folder).join("data"), "x").unwrap();
        }
        fs::create_dir_all(home.join("Network")).unwrap();
        for file in ["Cookies", "Cookies-journal", "Network Persistent State"] {
            fs::write(home.join("Network").join(file), "x").unwrap();
        }
        fs::write(profile.join("Local State"), "x").unwrap();
        fs::write(home.join("Preferences"), "x").unwrap();

        scrub(&profile).unwrap();

        for gone in [
            "Service Worker",
            "Cache",
            "Code Cache",
            "IndexedDB",
            "File System",
            "WebStorage",
            "Shared Dictionary",
            "blob_storage",
            "Session Storage",
            "Network/Cookies",
            "Network/Cookies-journal",
        ] {
            assert!(!home.join(gone).exists(), "{gone} should be deleted");
        }
        // The guest's root key lives in local storage (the reset page keeps it); the rest is
        // Chrome's own.
        for kept in [
            "Local Storage/leveldb/data",
            "GPUCache/data",
            "Preferences",
            "Network/Network Persistent State",
        ] {
            assert!(home.join(kept).exists(), "{kept} should stay");
        }
        assert!(profile.join("Local State").exists());
    }

    #[test]
    fn scrubbing_a_profile_that_was_never_used_is_fine() {
        scrub(&scratch("fresh")).unwrap();
    }

    #[test]
    fn the_process_list_can_be_read() {
        // On the machine of whoever runs the tests; it holds this test process at least.
        let me = std::process::id();
        let profile = Path::new("/no/chrome/runs/on/this/profile");
        assert!(running(profile).unwrap().iter().all(|pid| *pid != me));
    }
}
