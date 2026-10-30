//! `catalyst secrets init`: the secrets a self-contained host mints for
//! itself. With the SFU on the host (`[livekit].enabled`) and Pulse beside
//! serve, every slot of the two environment files is a value the host can
//! generate once and keep: the LiveKit API key pair (serve, the SFU and Pulse
//! share it), the Pulse room-authority key, the world-storage encryption key,
//! the authoritative scene-host wallet with its address, the admin bearers.
//! `init` fills the slots the serve file lacks and leaves every present line
//! as it is (rotation: delete the line, run it again, restart the units); the
//! pulse file is derived from serve's values on every run, so the pair never
//! drifts. Nothing is rewritten when nothing changed, so the release
//! controller's digests hold across activations. Files land 0400, atomically,
//! in a 0700 directory, owned by the caller (root: the
//! `catalyst-secrets-init` oneshot, deploy/nix/generated-secrets.nix).
//! Formats are the parsers': `worlds::storage::encryption` (64 hex),
//! `comms::gatekeeper::admission::relay_authority::parse_service_key` (32 base64url bytes,
//! no padding, distinct from the LiveKit secret), `scene_state::delegation::
//! Wallet::from_hex` (a 32-byte secp256k1 key; the address is derived).

use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

use anyhow::{Context, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

use crate::worlds::scene_state::delegation::Wallet;

/// The serve file's slots, in the order a fresh file lists them.
pub const SERVE_SLOTS: [&str; 11] = [
    "LIVEKIT_API_KEY",
    "LIVEKIT_API_SECRET",
    "PULSE_ROOM_AUTHORITY_KEY",
    "ENCRYPTION_KEY",
    "SCENE_HOST_AUTHORITATIVE_KEY",
    "AUTHORITATIVE_SERVER_ADDRESS",
    "SCENE_HOST_ADMIN_TOKEN",
    "CATALYRST_WORLDS_ADMIN_TOKEN",
    "CATALYRST_CONTENT_ADMIN_TOKEN",
    "COMMS_GATEKEEPER_AUTH_TOKEN",
    "MODERATOR_TOKEN",
];
/// The bearer abgen's queue and admin routes check (`catalyst-abgen-secrets`
/// copies this one line into abgen's credential), minted while abgen runs.
pub const ABGEN_SLOT: &str = "AB_REGISTRY_ADMIN_TOKEN";
/// The pulse file: the LiveKit pair and the room-authority key serve holds.
pub const PULSE_SLOTS: [(&str, &str); 4] = [
    ("PULSE_APPLICATION_RELAY_LIVEKIT_API_KEY", "LIVEKIT_API_KEY"),
    (
        "PULSE_APPLICATION_RELAY_LIVEKIT_SECRET",
        "LIVEKIT_API_SECRET",
    ),
    ("PULSE_ROOM_AUTHORITY_KEY", "PULSE_ROOM_AUTHORITY_KEY"),
    ("COMMS_MODERATOR_TOKEN", "MODERATOR_TOKEN"),
];

/// What one run changed: the serve slots it minted (names only) and whether
/// either file was rewritten.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub generated: Vec<&'static str>,
    pub serve_written: bool,
    pub pulse_written: bool,
}

/// One `KEY=VALUE` file: every line kept verbatim, slots looked up by key.
struct EnvFile {
    lines: Vec<String>,
}

impl EnvFile {
    fn read(path: &Path) -> anyhow::Result<Self> {
        let lines = match fs::read_to_string(path) {
            Ok(text) => text.lines().map(str::to_owned).collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        Ok(Self { lines })
    }

    fn get(&self, key: &str) -> Option<String> {
        self.lines.iter().find_map(|line| {
            let (k, v) = line.split_once('=')?;
            (k.trim() == key).then(|| unquote(v.trim()).to_owned())
        })
    }

    /// Replace the slot's line in place, or append one; true when the text changed.
    fn set(&mut self, key: &str, value: &str) -> bool {
        let rendered = format!("{key}={value}");
        match self
            .lines
            .iter()
            .position(|line| line.split_once('=').is_some_and(|(k, _)| k.trim() == key))
        {
            Some(i) if self.lines[i] == rendered => false,
            Some(i) => {
                self.lines[i] = rendered;
                true
            }
            None => {
                self.lines.push(rendered);
                true
            }
        }
    }

    fn render(&self) -> String {
        let mut text = self.lines.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        text
    }
}

fn unquote(value: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|v| v.strip_suffix(quote))
        {
            return inner;
        }
    }
    value
}

fn random_bytes<const N: usize>() -> anyhow::Result<[u8; N]> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("no entropy: {e}"))?;
    Ok(bytes)
}

fn random_hex<const N: usize>() -> anyhow::Result<String> {
    Ok(hex::encode(random_bytes::<N>()?))
}

/// Fill the serve file's missing slots, derive the pulse file, write what changed.
pub fn init(
    serve_file: &Path,
    pulse_file: Option<&Path>,
    with_abgen: bool,
) -> anyhow::Result<Outcome> {
    let mut serve = EnvFile::read(serve_file)?;
    let mut outcome = Outcome::default();
    let mut mint = |file: &mut EnvFile, key: &'static str, value: String| {
        file.set(key, &value);
        outcome.generated.push(key);
    };

    if serve.get("LIVEKIT_API_KEY").is_none() {
        mint(
            &mut serve,
            "LIVEKIT_API_KEY",
            format!("API{}", random_hex::<8>()?),
        );
    }
    if serve.get("LIVEKIT_API_SECRET").is_none() {
        mint(&mut serve, "LIVEKIT_API_SECRET", random_hex::<32>()?);
    }
    if serve.get("PULSE_ROOM_AUTHORITY_KEY").is_none() {
        mint(
            &mut serve,
            "PULSE_ROOM_AUTHORITY_KEY",
            URL_SAFE_NO_PAD.encode(random_bytes::<32>()?),
        );
    }
    if serve.get("ENCRYPTION_KEY").is_none() {
        mint(&mut serve, "ENCRYPTION_KEY", random_hex::<32>()?);
    }
    let wallet = match serve.get("SCENE_HOST_AUTHORITATIVE_KEY") {
        Some(key) => Wallet::from_hex(&key).map_err(|e| {
            anyhow::anyhow!(
                "SCENE_HOST_AUTHORITATIVE_KEY in {}: {e}",
                serve_file.display()
            )
        })?,
        None => {
            let wallet = Wallet::random()
                .map_err(|e| anyhow::anyhow!("SCENE_HOST_AUTHORITATIVE_KEY: {e}"))?;
            mint(
                &mut serve,
                "SCENE_HOST_AUTHORITATIVE_KEY",
                wallet.private_key_hex(),
            );
            wallet
        }
    };
    if serve
        .get("AUTHORITATIVE_SERVER_ADDRESS")
        .is_none_or(|a| !a.eq_ignore_ascii_case(&wallet.address()))
    {
        mint(&mut serve, "AUTHORITATIVE_SERVER_ADDRESS", wallet.address());
    }
    for token in [
        "SCENE_HOST_ADMIN_TOKEN",
        "CATALYRST_WORLDS_ADMIN_TOKEN",
        "CATALYRST_CONTENT_ADMIN_TOKEN",
        "COMMS_GATEKEEPER_AUTH_TOKEN",
        "MODERATOR_TOKEN",
    ] {
        if serve.get(token).is_none() {
            mint(&mut serve, token, random_hex::<32>()?);
        }
    }
    if with_abgen && serve.get(ABGEN_SLOT).is_none() {
        mint(&mut serve, ABGEN_SLOT, random_hex::<32>()?);
    }
    if !outcome.generated.is_empty() {
        write_private(serve_file, &serve.render())?;
        outcome.serve_written = true;
    }

    if let Some(pulse_file) = pulse_file {
        let mut pulse = EnvFile::read(pulse_file)?;
        let mut changed = false;
        for (slot, source) in PULSE_SLOTS {
            let value = serve.get(source).with_context(|| {
                format!("{source} missing from {} after init", serve_file.display())
            })?;
            changed |= pulse.set(slot, &value);
        }
        if changed || !pulse_file.exists() {
            write_private(pulse_file, &pulse.render())?;
            outcome.pulse_written = true;
        }
    }
    Ok(outcome)
}

/// 0400 file in a 0700 directory, written beside its final name and renamed
/// over it, both fsynced; a stale temp file from an interrupted run is replaced.
fn write_private(path: &Path, text: &str) -> anyhow::Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut dir_builder = fs::DirBuilder::new();
    dir_builder.recursive(true);
    #[cfg(unix)]
    dir_builder.mode(0o700);
    dir_builder
        .create(dir)
        .with_context(|| format!("creating {}", dir.display()))?;
    #[cfg(unix)]
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("securing {}", dir.display()))?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty());
    let Some(name) = name else {
        bail!("{} has no file name", path.display())
    };
    let tmp = dir.join(format!(".{name}.tmp"));
    let _ = fs::remove_file(&tmp);
    let mut open_options = fs::OpenOptions::new();
    open_options.write(true).create_new(true);
    #[cfg(unix)]
    open_options.mode(0o400);
    let mut file = open_options
        .open(&tmp)
        .with_context(|| format!("creating {}", tmp.display()))?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .with_context(|| format!("writing {}", tmp.display()))?;
    drop(file);
    fs::rename(&tmp, path)
        .with_context(|| format!("renaming {} over {}", tmp.display(), path.display()))?;
    fs::File::open(dir)
        .and_then(|d| d.sync_all())
        .with_context(|| format!("syncing {}", dir.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn scratch() -> std::path::PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "catalyst-generated-secrets-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[cfg(unix)]
    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    // unix only: Windows cannot fsync a directory, which `write_private` does last.
    #[cfg(unix)]
    #[test]
    fn a_fresh_host_gets_every_slot_in_the_parsers_formats() {
        let dir = scratch();
        let serve = dir.join("secrets/catalyst.env");
        let pulse = dir.join("secrets/catalyst-pulse.env");
        let out = init(&serve, Some(&pulse), true).unwrap();
        assert_eq!(out.generated.len(), SERVE_SLOTS.len() + 1, "{out:?}");
        assert!(out.serve_written && out.pulse_written);
        #[cfg(unix)]
        assert_eq!(mode(&serve), 0o400);
        #[cfg(unix)]
        assert_eq!(mode(&pulse), 0o400);
        #[cfg(unix)]
        assert_eq!(mode(serve.parent().unwrap()), 0o700);

        let file = EnvFile::read(&serve).unwrap();
        assert_eq!(file.lines.len(), SERVE_SLOTS.len() + 1, "{:?}", file.lines);
        let key = file.get("LIVEKIT_API_KEY").unwrap();
        assert!(key.starts_with("API") && key.len() == 19, "{key}");
        let secret = file.get("LIVEKIT_API_SECRET").unwrap();
        assert_eq!(hex::decode(&secret).unwrap().len(), 32);
        let room = file.get("PULSE_ROOM_AUTHORITY_KEY").unwrap();
        crate::comms::gatekeeper::admission::relay_authority::parse_service_key(
            &room,
            secret.as_bytes(),
        )
        .unwrap();
        assert_eq!(
            hex::decode(file.get("ENCRYPTION_KEY").unwrap())
                .unwrap()
                .len(),
            32
        );
        let wallet = Wallet::from_hex(&file.get("SCENE_HOST_AUTHORITATIVE_KEY").unwrap()).unwrap();
        assert_eq!(
            file.get("AUTHORITATIVE_SERVER_ADDRESS").unwrap(),
            wallet.address()
        );
        for token in [
            "SCENE_HOST_ADMIN_TOKEN",
            "CATALYRST_WORLDS_ADMIN_TOKEN",
            "CATALYRST_CONTENT_ADMIN_TOKEN",
            "COMMS_GATEKEEPER_AUTH_TOKEN",
            "MODERATOR_TOKEN",
            ABGEN_SLOT,
        ] {
            assert_eq!(
                hex::decode(file.get(token).unwrap()).unwrap().len(),
                32,
                "{token}"
            );
        }

        let pulse_file = EnvFile::read(&pulse).unwrap();
        assert_eq!(
            pulse_file.lines.len(),
            PULSE_SLOTS.len(),
            "{:?}",
            pulse_file.lines
        );
        for (slot, source) in PULSE_SLOTS {
            assert_eq!(
                pulse_file.get(slot).unwrap(),
                file.get(source).unwrap(),
                "{slot}"
            );
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    // unix only: Windows cannot fsync a directory, which `write_private` does last.
    #[cfg(unix)]
    #[test]
    fn a_second_run_changes_nothing() {
        let dir = scratch();
        let serve = dir.join("catalyst.env");
        let pulse = dir.join("catalyst-pulse.env");
        init(&serve, Some(&pulse), false).unwrap();
        let before = (
            fs::read_to_string(&serve).unwrap(),
            fs::read_to_string(&pulse).unwrap(),
        );
        let again = init(&serve, Some(&pulse), false).unwrap();
        assert_eq!(again, Outcome::default());
        assert_eq!(
            before,
            (
                fs::read_to_string(&serve).unwrap(),
                fs::read_to_string(&pulse).unwrap()
            )
        );
        assert!(
            EnvFile::read(&serve).unwrap().get(ABGEN_SLOT).is_none(),
            "abgen's bearer is minted only with --with-abgen"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    // unix only: Windows cannot fsync a directory, which `write_private` does last.
    #[cfg(unix)]
    #[test]
    fn a_deleted_line_is_rotated_alone_and_the_pulse_file_follows() {
        let dir = scratch();
        let serve = dir.join("catalyst.env");
        let pulse = dir.join("catalyst-pulse.env");
        init(&serve, Some(&pulse), false).unwrap();
        let before = EnvFile::read(&serve).unwrap();
        let kept: Vec<String> = before
            .lines
            .iter()
            .filter(|l| !l.starts_with("LIVEKIT_API_SECRET="))
            .cloned()
            .collect();
        #[cfg(unix)]
        fs::set_permissions(&serve, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&serve, format!("{}\n", kept.join("\n"))).unwrap();

        let out = init(&serve, Some(&pulse), false).unwrap();
        assert_eq!(out.generated, vec!["LIVEKIT_API_SECRET"]);
        assert!(out.pulse_written);
        let after = EnvFile::read(&serve).unwrap();
        for slot in SERVE_SLOTS {
            if slot == "LIVEKIT_API_SECRET" {
                assert_ne!(after.get(slot), before.get(slot));
            } else {
                assert_eq!(after.get(slot), before.get(slot), "{slot}");
            }
        }
        assert_eq!(
            EnvFile::read(&pulse)
                .unwrap()
                .get("PULSE_APPLICATION_RELAY_LIVEKIT_SECRET")
                .unwrap(),
            after.get("LIVEKIT_API_SECRET").unwrap()
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    // unix only: Windows cannot fsync a directory, which `write_private` does last.
    #[cfg(unix)]
    #[test]
    fn an_operators_key_keeps_its_value_and_gets_its_derived_address() {
        let dir = scratch();
        let serve = dir.join("catalyst.env");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            &serve,
            "# hand-written\nSCENE_HOST_AUTHORITATIVE_KEY=0000000000000000000000000000000000000000000000000000000000000001\nAUTHORITATIVE_SERVER_ADDRESS=0x0000000000000000000000000000000000000000\nLIVEKIT_API_KEY=\"APIhand\"\n",
        )
        .unwrap();
        let out = init(&serve, None, false).unwrap();
        assert!(
            out.generated.contains(&"AUTHORITATIVE_SERVER_ADDRESS"),
            "{out:?}"
        );
        assert!(
            !out.generated.contains(&"SCENE_HOST_AUTHORITATIVE_KEY")
                && !out.generated.contains(&"LIVEKIT_API_KEY"),
            "{out:?}"
        );
        let file = EnvFile::read(&serve).unwrap();
        assert_eq!(file.lines[0], "# hand-written");
        assert_eq!(
            file.get("AUTHORITATIVE_SERVER_ADDRESS").unwrap(),
            "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf"
        );
        assert_eq!(
            file.lines[3], "LIVEKIT_API_KEY=\"APIhand\"",
            "a present line is left verbatim"
        );
        assert!(!dir.join("catalyst-pulse.env").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_malformed_key_is_refused_rather_than_replaced() {
        let dir = scratch();
        let serve = dir.join("catalyst.env");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&serve, "SCENE_HOST_AUTHORITATIVE_KEY=not-a-key\n").unwrap();
        let err = init(&serve, None, false).unwrap_err().to_string();
        assert!(err.contains("SCENE_HOST_AUTHORITATIVE_KEY"), "{err}");
        assert_eq!(
            fs::read_to_string(&serve).unwrap(),
            "SCENE_HOST_AUTHORITATIVE_KEY=not-a-key\n"
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
