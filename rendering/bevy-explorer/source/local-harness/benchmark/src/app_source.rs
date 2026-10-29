//! Where a web app comes from: a folder, a zip or the URL of a zip. The address is typed by hand
//! and the archive is somebody's build, so both are bounded before anything is unpacked.

use std::fs::{self, File};
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use reqwest::Url;
use tokio_util::sync::CancellationToken;
use zip06::ZipArchive;

use crate::failure::Failure;
use crate::out;

const MAX_DOWNLOAD_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 20_000;
const MAX_UNPACKED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const DOWNLOAD_ATTEMPTS: u32 = 5;
const MAX_REDIRECTS: usize = 3;

pub fn under_actions() -> bool {
    std::env::var("GITHUB_ACTIONS").is_ok_and(|value| value == "true")
}

/// Under Actions a build is a folder or a `.zip` on this machine, which the workflow downloaded:
/// an address is refused, and so is a network path (`\\host\share`, `//host/share`), which would
/// be an SMB login to the host it names.
pub fn require_local(source: &str) -> Result<(), Failure> {
    if web_address(source).is_some() || source.starts_with("\\\\") || source.starts_with("//") {
        return Err(Failure::usage(format!(
            "'{}' is not a folder or a .zip on this machine; under Actions the workflow downloads \
             the build",
            out::safe(source)
        )));
    }
    Ok(())
}

/// Makes the app available at a folder holding `index.html` and returns that folder: a folder is
/// used where it is, a zip or a download is unpacked into `destination`.
pub fn provision(
    source: &str,
    destination: &Path,
    cancel: &CancellationToken,
) -> Result<PathBuf, Failure> {
    if let Some(url) = web_address(source) {
        let mut zip = destination.as_os_str().to_owned();
        zip.push(".zip");
        let zip = PathBuf::from(zip);

        let unpacked = download(&url, &zip, cancel).and_then(|()| extract(&zip, destination));
        let _ = fs::remove_file(&zip);
        unpacked?;
    } else if Path::new(source).is_file() && source.to_ascii_lowercase().ends_with(".zip") {
        extract(Path::new(source), destination)?;
    } else if Path::new(source).is_dir() {
        return find_app_root(&std::path::absolute(source)?);
    } else {
        return Err(Failure::usage(format!(
            "'{}' is not a web app folder, a .zip or a URL",
            out::safe(source)
        )));
    }

    find_app_root(&std::path::absolute(destination)?)
}

fn web_address(source: &str) -> Option<Url> {
    Url::parse(source)
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https"))
}

/// The folder that holds `index.html`: the folder itself, or the one folder inside it (zips are
/// often wrapped in a top folder).
pub fn find_app_root(dir: &Path) -> Result<PathBuf, Failure> {
    if dir.join("index.html").is_file() {
        return Ok(dir.to_path_buf());
    }

    if let Ok(entries) = fs::read_dir(dir) {
        let inner: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_default();
                path.is_dir()
                    && !name.starts_with('.')
                    && !name.starts_with("__")
                    && path.join("index.html").is_file()
            })
            .collect();
        if let [only] = inner.as_slice() {
            return Ok(only.clone());
        }
    }

    Err(Failure::usage(format!(
        "no web app (index.html) in '{}'",
        out::safe(&dir.to_string_lossy())
    )))
}

/// The destination is emptied and unpacked into. The archive is checked before anything is
/// written (entry count, declared sizes); every entry is checked as it is unpacked (its name, its
/// real size); on any failure what was unpacked is removed and the error is a message.
fn extract(zip: &Path, destination: &Path) -> Result<(), Failure> {
    let destination = std::path::absolute(destination)?;
    refuse_to_empty(&destination)?;

    if destination.exists() {
        fs::remove_dir_all(&destination)?;
    }

    let unpacked = unpack_archive(zip, &destination);
    if unpacked.is_err() {
        let _ = fs::remove_dir_all(&destination);
    }
    unpacked
}

fn unpack_archive(zip: &Path, destination: &Path) -> Result<(), Failure> {
    let mut archive = ZipArchive::new(BufReader::new(File::open(zip)?))?;
    check_archive(&mut archive)?;
    fs::create_dir_all(destination)?;
    unpack(&mut archive, destination)
}

/// A mistyped destination (`.`, `..`, a root) must not empty the folder the work is in.
fn refuse_to_empty(destination: &Path) -> Result<(), Failure> {
    let comparable = |path: &Path| {
        let text = path
            .to_string_lossy()
            .trim_end_matches(['/', '\\'])
            .to_string();
        if cfg!(windows) {
            text.to_lowercase()
        } else {
            text
        }
    };
    let here = std::env::current_dir()?;
    let (dir, here) = (comparable(destination), comparable(&here));

    let is_root = destination.parent().is_none();
    let holds_here =
        here == dir || here.starts_with(&format!("{dir}{}", std::path::MAIN_SEPARATOR));
    if is_root || holds_here {
        return Err(Failure::usage(format!(
            "'{}' is a root, the working directory or one of its parents; it is emptied before a build is unpacked into it",
            out::safe(&destination.to_string_lossy())
        )));
    }
    Ok(())
}

fn check_archive<R: Read + io::Seek>(archive: &mut ZipArchive<R>) -> Result<(), Failure> {
    if archive.len() > MAX_ENTRIES {
        return Err(Failure::usage(format!(
            "the archive has {} entries, over the limit of {MAX_ENTRIES}",
            archive.len()
        )));
    }

    let mut declared = 0u64;
    for index in 0..archive.len() {
        let size = archive.by_index_raw(index)?.size();
        if size > MAX_UNPACKED_BYTES - declared {
            return Err(Failure::usage(format!(
                "the archive unpacks to more than the limit of {MAX_UNPACKED_BYTES} B"
            )));
        }
        declared += size;
    }
    Ok(())
}

fn unpack<R: Read + io::Seek>(
    archive: &mut ZipArchive<R>,
    destination: &Path,
) -> Result<(), Failure> {
    let mut written = 0u64;
    let mut buffer = vec![0u8; 80 * 1024];

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().to_string();
        let link = entry
            .unix_mode()
            .is_some_and(|mode| mode & 0xF000 == 0xA000);
        let path = entry_path(destination, &name, link)?;

        if name.ends_with('/') {
            fs::create_dir_all(&path)?;
            continue;
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Created new: a name repeated in the archive, or a file and a folder of one name, fails.
        let mut output = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                return Err(Failure::usage(format!(
                    "the archive holds '{}' twice, or as a file and a folder",
                    out::safe(&name)
                )))
            }
            Err(error) => return Err(error.into()),
        };

        // Counted as it is written: the sizes in a zip are the archive's say-so.
        loop {
            let count = entry.read(&mut buffer).map_err(unreadable)?;
            if count == 0 {
                break;
            }
            written += count as u64;
            if written > MAX_UNPACKED_BYTES {
                return Err(Failure::usage(format!(
                    "the archive unpacks to more than the limit of {MAX_UNPACKED_BYTES} B"
                )));
            }
            output.write_all(&buffer[..count])?;
        }
    }
    Ok(())
}

/// A zip entry that does not read back as its archive says (a bad checksum, a bad stream) is a
/// bad archive; any other error is the disk's.
fn unreadable(error: io::Error) -> Failure {
    if error.kind() == io::ErrorKind::InvalidData {
        Failure::usage(format!(
            "the archive cannot be read as a zip ({})",
            out::safe(&error.to_string())
        ))
    } else {
        error.into()
    }
}

/// Where an entry goes, or an error: relative names of plain segments only (no `..`, no root, no
/// drive or stream colon, no backslash, no device name), not a link, and inside the destination.
fn entry_path(destination: &Path, name: &str, link: bool) -> Result<PathBuf, Failure> {
    let segments: Vec<&str> = name.trim_end_matches('/').split('/').collect();

    if name.is_empty()
        || name.starts_with('/')
        || name.contains('\\')
        || name.contains(':')
        || name.chars().any(char::is_control)
        || segments
            .iter()
            .any(|s| s.is_empty() || *s == "." || *s == ".." || is_device_name(s))
    {
        return Err(Failure::usage(format!(
            "the archive has an entry with an unsafe name ('{}')",
            out::safe(name)
        )));
    }

    if link {
        return Err(Failure::usage(format!(
            "the archive has a symbolic link ('{}')",
            out::safe(name)
        )));
    }

    let mut path = destination.to_path_buf();
    path.extend(&segments);
    if !path.starts_with(destination) {
        return Err(Failure::usage(format!(
            "the archive has an entry outside its folder ('{}')",
            out::safe(name)
        )));
    }
    Ok(path)
}

/// Windows reads CON, NUL, COM1 ... as devices wherever they stand in a path, with or without an
/// extension.
pub fn is_device_name(segment: &str) -> bool {
    let stem = segment
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ');
    let upper = stem.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        upper
            .strip_prefix(prefix)
            .is_some_and(|digit| digit.len() == 1 && digit.as_bytes()[0].is_ascii_digit())
    })
}

enum Attempt {
    /// Worth another try: the network or the disk failed.
    Retry(String),
    Stop(Failure),
}

// An error message holds the address it was for, query included, and a signed address's query is
// a secret: only the error's own words are kept.
impl From<reqwest::Error> for Attempt {
    fn from(error: reqwest::Error) -> Self {
        Self::Retry(error.without_url().to_string())
    }
}

impl From<io::Error> for Attempt {
    fn from(error: io::Error) -> Self {
        Self::Retry(error.to_string())
    }
}

/// `curl -fsSL --retry 5 --retry-all-errors`, with a limit on the size.
fn download(url: &Url, file: &Path, cancel: &CancellationToken) -> Result<(), Failure> {
    // Only the path is logged: a query string may hold a signature, and a step's own `env:`
    // header prints the whole address, so a signed URL does not belong here (under Actions
    // `fetch` takes no address at all).
    let mut shown = url.clone();
    shown.set_query(None);
    shown.set_fragment(None);
    let _ = shown.set_username("");
    let _ = shown.set_password(None);
    out::log(format!("downloading {shown}"));

    let https_only = url.scheme() == "https";
    let client = Client::builder()
        .redirect(Policy::custom(move |attempt| {
            if attempt.previous().len() > MAX_REDIRECTS {
                attempt.error("too many redirects")
            } else if https_only && attempt.url().scheme() != "https" {
                // A redirect may not take an https download off https.
                attempt.error("the download was redirected away from https")
            } else {
                attempt.follow()
            }
        }))
        .timeout(Duration::from_secs(30 * 60))
        .build()?;

    for attempt in 1.. {
        if cancel.is_cancelled() {
            return Err(Failure::Interrupted);
        }
        match fetch_once(&client, url, file, cancel) {
            Ok(()) => return Ok(()),
            Err(Attempt::Stop(failure)) => return Err(failure),
            Err(Attempt::Retry(message)) if attempt < DOWNLOAD_ATTEMPTS => {
                out::log(format!(
                    "download failed ({}), retrying",
                    out::safe(&message)
                ));
                std::thread::sleep(Duration::from_secs(u64::from(attempt) * 2));
            }
            Err(Attempt::Retry(message)) => {
                return Err(Failure::cannot_run(format!(
                    "cannot download {shown}: {}",
                    out::safe(&message)
                )))
            }
        }
    }
    unreachable!("the loop returns")
}

fn fetch_once(
    client: &Client,
    url: &Url,
    file: &Path,
    cancel: &CancellationToken,
) -> Result<(), Attempt> {
    let mut response = client.get(url.clone()).send()?.error_for_status()?;

    if let Some(declared) = response
        .content_length()
        .filter(|n| *n > MAX_DOWNLOAD_BYTES)
    {
        return Err(Attempt::Stop(Failure::usage(format!(
            "the download is {declared} B, over the limit of {MAX_DOWNLOAD_BYTES} B"
        ))));
    }

    let mut output = File::create(file)?;
    let mut buffer = vec![0u8; 80 * 1024];
    let mut total = 0u64;

    // Counted as it arrives: a response need not say how long it is.
    loop {
        if cancel.is_cancelled() {
            return Err(Attempt::Stop(Failure::Interrupted));
        }
        let count = response.read(&mut buffer)?;
        if count == 0 {
            return Ok(());
        }
        total += count as u64;
        if total > MAX_DOWNLOAD_BYTES {
            return Err(Attempt::Stop(Failure::usage(format!(
                "the download passed the limit of {MAX_DOWNLOAD_BYTES} B"
            ))));
        }
        output.write_all(&buffer[..count])?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        crate::scratch("app-source", name)
    }

    #[test]
    fn only_a_folder_or_a_zip_on_this_machine_is_fetched_under_actions() {
        for local in [
            "change_build",
            "builds/app.zip",
            "/builds/app.zip",
            "C:\\builds\\app.zip",
        ] {
            assert!(require_local(local).is_ok(), "{local}");
        }
        for refused in [
            "https://bucket.example/builds/a.zip",
            "http://127.0.0.1:1/a.zip",
            "HTTPS://bucket.example/a.zip",
            "\\\\host\\share\\a.zip",
            "//host/share/a.zip",
        ] {
            assert!(
                matches!(require_local(refused), Err(Failure::Usage(_))),
                "{refused}"
            );
        }
    }

    #[test]
    fn a_failed_request_keeps_the_addresss_query_out_of_every_message() {
        // A port nothing listens on, so the request fails at once.
        let port = std::net::TcpListener::bind(("127.0.0.1", 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let address = format!("http://127.0.0.1:{port}/app.zip?signature=not-for-a-log");
        let failed = || {
            Client::builder()
                .no_proxy()
                .build()
                .unwrap()
                .get(&address)
                .send()
                .expect_err("nothing listens there")
        };

        let error = failed();
        assert!(
            error.to_string().contains("not-for-a-log"),
            "the error prints the address, which is why it is stripped"
        );
        match Attempt::from(error) {
            Attempt::Retry(message) => assert!(!message.contains("not-for-a-log"), "{message}"),
            Attempt::Stop(failure) => panic!("{failure}"),
        }
        let message = Failure::from(failed()).to_string();
        assert!(!message.contains("not-for-a-log"), "{message}");
    }

    #[test]
    fn windows_devices_are_names_wherever_they_stand() {
        for device in ["con", "NUL", "nul.txt", "Com1", "lpt9.x", "aux ", "CONIN$"] {
            assert!(is_device_name(device), "{device}");
        }
        for plain in ["index.html", "console", "com", "com10", "lpt", "a.con"] {
            assert!(!is_device_name(plain), "{plain}");
        }
    }

    #[test]
    fn entries_are_plain_relative_names_inside_the_folder() {
        let base = Path::new("base");
        assert_eq!(
            entry_path(base, "a/b.js", false).unwrap(),
            base.join("a").join("b.js")
        );
        assert!(entry_path(base, "dir/", false).is_ok());
        for unsafe_name in [
            "../x",
            "a/../../x",
            "a/./x",
            "/abs",
            "a\\b",
            "c:/x",
            "index.html:stream",
            "a//b",
            "nul",
            "a/CON.txt",
            "",
            "a\u{0}b",
        ] {
            assert!(
                entry_path(base, unsafe_name, false).is_err(),
                "{unsafe_name:?}"
            );
        }
        assert!(entry_path(base, "link", true).is_err());
    }

    #[test]
    fn a_folder_is_used_where_it_is_and_a_wrapped_one_is_found() {
        let root = scratch("find");
        fs::create_dir_all(root.join("wrapped")).unwrap();
        fs::write(root.join("wrapped").join("index.html"), "x").unwrap();
        fs::create_dir_all(root.join("__MACOSX")).unwrap();
        fs::write(root.join("__MACOSX").join("index.html"), "x").unwrap();
        assert_eq!(find_app_root(&root).unwrap(), root.join("wrapped"));

        fs::create_dir_all(root.join("second")).unwrap();
        fs::write(root.join("second").join("index.html"), "x").unwrap();
        assert!(find_app_root(&root).is_err(), "two apps are ambiguous");

        fs::write(root.join("index.html"), "x").unwrap();
        assert_eq!(find_app_root(&root).unwrap(), root);
    }

    #[test]
    fn a_destination_that_holds_the_working_directory_is_not_emptied() {
        let here = std::env::current_dir().unwrap();
        assert!(refuse_to_empty(&here).is_err());
        assert!(refuse_to_empty(here.parent().unwrap()).is_err());
        assert!(refuse_to_empty(Path::new(std::path::MAIN_SEPARATOR_STR)).is_err());
        assert!(refuse_to_empty(&here.join("a-build-folder")).is_ok());
    }

    #[test]
    fn a_zip_is_unpacked_and_its_limits_hold() {
        let dir = scratch("zip");
        let zip = dir.join("app.zip");
        fs::write(
            &zip,
            stored_zip(&[("app/index.html", "<html>"), ("app/a/b.js", "1")]),
        )
        .unwrap();

        let root = provision(
            zip.to_str().unwrap(),
            &dir.join("out"),
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(
            root,
            std::path::absolute(dir.join("out").join("app")).unwrap()
        );
        assert_eq!(fs::read(root.join("a").join("b.js")).unwrap(), b"1");

        fs::write(&zip, stored_zip(&[("ok.txt", "1"), ("../escape.txt", "2")])).unwrap();
        let refused = provision(
            zip.to_str().unwrap(),
            &dir.join("out2"),
            &CancellationToken::new(),
        );
        assert!(matches!(refused, Err(Failure::Usage(_))));
        assert!(!dir.join("out2").exists(), "what was unpacked is removed");
        assert!(!dir.join("escape.txt").exists());

        fs::write(&zip, stored_zip(&[("a.txt", "1"), ("a.txt", "2")])).unwrap();
        let twice = provision(
            zip.to_str().unwrap(),
            &dir.join("out3"),
            &CancellationToken::new(),
        );
        assert!(matches!(twice, Err(Failure::Usage(_))));

        fs::write(&zip, b"not a zip").unwrap();
        let broken = provision(
            zip.to_str().unwrap(),
            &dir.join("out4"),
            &CancellationToken::new(),
        );
        assert!(matches!(broken, Err(Failure::Usage(_))));
    }

    /// What `zip -r` and `Compress-Archive` write: deflated entries, a folder entry, a wrapping
    /// folder. 262 bytes, made once by Python's zipfile.
    const DEFLATED_ZIP: &str = concat!(
        "504b030414000000080000605b5ded88bb4d1c0000002902000012000000777261707065642f696e6465782e68746d6c",
        "b3c928c9cdb14b494dcb492c494d5118658c3272501936fae0240200504b030414000000000000605b5d000000000000",
        "0000000000000c000000777261707065642f7375622f504b0102140014000000080000605b5ded88bb4d1c0000002902",
        "0000120000000000000000000000800100000000777261707065642f696e6465782e68746d6c504b0102140014000000",
        "000000605b5d0000000000000000000000000c000000000000000000000080014c000000777261707065642f7375622f",
        "504b050600000000020002007a000000760000000000",
    );

    #[test]
    fn an_archive_with_more_entries_than_the_limit_is_refused_before_anything_is_written() {
        let dir = scratch("entries");
        let names: Vec<String> = (0..=MAX_ENTRIES).map(|n| format!("f{n}")).collect();
        let entries: Vec<(&str, &str)> = names.iter().map(|name| (name.as_str(), "")).collect();
        let zip = dir.join("many.zip");
        fs::write(&zip, stored_zip(&entries)).unwrap();

        let refused = provision(
            zip.to_str().unwrap(),
            &dir.join("out"),
            &CancellationToken::new(),
        );
        assert!(
            matches!(&refused, Err(Failure::Usage(message)) if message.contains("entries")),
            "{refused:?}"
        );
        assert!(!dir.join("out").exists());

        // At the limit the archive passes the check.
        let (kept, _) = entries.split_at(MAX_ENTRIES);
        let mut archive = ZipArchive::new(io::Cursor::new(stored_zip(kept))).unwrap();
        assert!(check_archive(&mut archive).is_ok());
    }

    #[test]
    fn an_archive_that_declares_more_than_it_may_unpack_to_is_refused_on_its_say_so() {
        let dir = scratch("declared");
        let mut bytes = stored_zip(&[("big.bin", "x")]);
        // The central directory's uncompressed size of the one entry: just under 4 GiB.
        let central = bytes
            .windows(4)
            .position(|w| w == [0x50, 0x4b, 1, 2])
            .unwrap();
        bytes[central + 24..central + 28].copy_from_slice(&0xFFFF_FF00u32.to_le_bytes());
        let zip = dir.join("bomb.zip");
        fs::write(&zip, bytes).unwrap();

        let refused = provision(
            zip.to_str().unwrap(),
            &dir.join("out"),
            &CancellationToken::new(),
        );
        assert!(
            matches!(&refused, Err(Failure::Usage(message)) if message.contains("limit")),
            "{refused:?}"
        );
        assert!(!dir.join("out").exists());
    }

    #[test]
    fn a_deflated_zip_is_unpacked() {
        let dir = scratch("deflated");
        let bytes: Vec<u8> = (0..DEFLATED_ZIP.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&DEFLATED_ZIP[at..at + 2], 16).unwrap())
            .collect();
        let zip = dir.join("app.zip");
        fs::write(&zip, bytes).unwrap();

        let root = provision(
            zip.to_str().unwrap(),
            &dir.join("out"),
            &CancellationToken::new(),
        )
        .unwrap();

        assert_eq!(
            root,
            std::path::absolute(dir.join("out").join("wrapped")).unwrap()
        );
        let page = fs::read_to_string(root.join("index.html")).unwrap();
        assert_eq!(page, format!("<html>{}</html>", "deflated ".repeat(60)));
        assert!(root.join("sub").is_dir());
    }

    #[test]
    fn something_that_is_no_app_is_a_usage_error() {
        let missing = provision("does/not/exist", Path::new("x"), &CancellationToken::new());
        assert!(matches!(missing, Err(Failure::Usage(_))));
    }

    /// A zip of stored (uncompressed) entries, written by hand: the tests need no zip writer.
    fn stored_zip(entries: &[(&str, &str)]) -> Vec<u8> {
        let crc = |data: &[u8]| {
            let mut crc = 0xFFFF_FFFFu32;
            for byte in data {
                crc ^= u32::from(*byte);
                for _ in 0..8 {
                    crc = if crc & 1 == 1 {
                        (crc >> 1) ^ 0xEDB8_8320
                    } else {
                        crc >> 1
                    };
                }
            }
            !crc
        };
        let (mut archive, mut directory) = (Vec::new(), Vec::new());
        for (name, data) in entries {
            let data = data.as_bytes();
            let offset = archive.len() as u32;
            let checksum = crc(data);
            let head = |out: &mut Vec<u8>, central: bool| {
                out.extend(if central {
                    [0x50, 0x4b, 1, 2]
                } else {
                    [0x50, 0x4b, 3, 4]
                });
                if central {
                    out.extend(20u16.to_le_bytes()); // made by
                }
                out.extend(20u16.to_le_bytes()); // needed
                out.extend(0u16.to_le_bytes()); // flags
                out.extend(0u16.to_le_bytes()); // stored
                out.extend(0u16.to_le_bytes()); // time
                out.extend(0x21u16.to_le_bytes()); // date
                out.extend(checksum.to_le_bytes());
                out.extend((data.len() as u32).to_le_bytes());
                out.extend((data.len() as u32).to_le_bytes());
                out.extend((name.len() as u16).to_le_bytes());
                out.extend(0u16.to_le_bytes()); // extra
            };
            head(&mut archive, false);
            archive.extend(name.as_bytes());
            archive.extend(data);

            head(&mut directory, true);
            directory.extend([0u8; 2 + 2 + 2 + 4]); // comment, disk, internal, external attributes
            directory.extend(offset.to_le_bytes());
            directory.extend(name.as_bytes());
        }
        let start = archive.len() as u32;
        let size = directory.len() as u32;
        archive.extend(directory);
        archive.extend([0x50, 0x4b, 5, 6, 0, 0, 0, 0]);
        archive.extend((entries.len() as u16).to_le_bytes());
        archive.extend((entries.len() as u16).to_le_bytes());
        archive.extend(size.to_le_bytes());
        archive.extend(start.to_le_bytes());
        archive.extend(0u16.to_le_bytes());
        archive
    }
}
