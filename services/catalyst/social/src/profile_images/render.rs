//! The external renderer, one process per entity writing the body and face
//! PNGs into a work directory: the engine's headless `avatar_render` (bevy,
//! windowless, the default) or the godot-explorer client's `--avatar-renderer`
//! mode on a throwaway X display. Both take the same `avatars.json` payload.
//! Nothing here runs unless `PROFILE_IMAGES_RENDER_ENABLED=true` names a binary.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::process::Command;

use super::cache::ImageKind;

pub const BODY_W: u32 = 256;
pub const BODY_H: u32 = 512;

pub const FACE_W: u32 = 256;
pub const FACE_H: u32 = 256;

/// Which renderer `PROFILE_IMAGES_RENDERER` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderBackend {
    /// The engine's `avatar_render` (`PROFILE_IMAGES_AVATAR_RENDER_BIN`).
    Bevy,
    /// The godot-explorer client (`PROFILE_IMAGES_GODOT_*`).
    Godot,
}

impl RenderBackend {
    fn label(self) -> &'static str {
        match self {
            Self::Bevy => "avatar_render",
            Self::Godot => "godot",
        }
    }
}

/// `PROFILE_IMAGES_RENDERER`, its binary, `PROFILE_IMAGES_GODOT_*` and
/// `PROFILE_IMAGES_RENDER_*`.
#[derive(Debug, Clone)]
pub struct RenderSettings {
    pub backend: RenderBackend,
    pub bin: String,
    /// The directory the binary runs in: the godot project root
    /// (`PROFILE_IMAGES_GODOT_PROJECT`, default: two levels above the binary);
    /// `None` runs it in the job's work directory.
    pub work_root: Option<String>,
    pub rendering_method: String,
    pub rendering_driver: String,
    pub dclenv: Option<String>,
    pub display: Option<String>,
    pub extra_args: Vec<String>,
    pub timeout_seconds: u64,
    pub max_concurrent: usize,
    pub workdir_root: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("avatar renderer binary not found at {0}")]
    BinaryMissing(PathBuf),
    #[error("failed to spawn the avatar renderer: {0}")]
    Spawn(String),
    #[error("avatar render timed out after {0:?}")]
    Timeout(Duration),
    #[error("avatar renderer exited with status {status}: {tail}")]
    NonZero { status: String, tail: String },
    #[error("render produced no {kind} png (the renderer ran but output missing)")]
    OutputMissing { kind: &'static str },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("payload serialize error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub struct RenderOutputs {
    pub body_path: PathBuf,
    pub face_path: PathBuf,
}

pub struct AvatarRenderer {
    cfg: RenderSettings,
}

impl AvatarRenderer {
    pub fn new(cfg: RenderSettings) -> Self {
        Self { cfg }
    }

    pub async fn render(
        &self,
        entity: &str,
        avatar: &Value,
        content_base: &str,
        workdir: &Path,
    ) -> Result<RenderOutputs, RenderError> {
        let bin = PathBuf::from(&self.cfg.bin);
        if !bin.is_file() {
            return Err(RenderError::BinaryMissing(bin));
        }

        tokio::fs::create_dir_all(workdir).await?;
        let body_path = workdir.join(format!("{entity}.png"));
        let face_path = workdir.join(format!("{entity}_face.png"));
        let payload_path = workdir.join("avatars.json");

        let payload = json!({
            "baseUrl": content_base,
            "payload": [{
                "entity": entity,
                "destPath": body_path.to_string_lossy(),
                "width": BODY_W,
                "height": BODY_H,
                "faceDestPath": face_path.to_string_lossy(),
                "faceWidth": FACE_W,
                "faceHeight": FACE_H,
                "avatar": avatar,
            }]
        });
        tokio::fs::write(&payload_path, serde_json::to_vec(&payload)?).await?;

        let mut args: Vec<String> = Vec::new();
        if self.cfg.backend == RenderBackend::Godot {
            args.push("--rendering-method".into());
            args.push(self.cfg.rendering_method.clone());
            args.push("--rendering-driver".into());
            args.push(self.cfg.rendering_driver.clone());
            args.push("--avatar-renderer".into());
        }
        args.push("--avatars".into());
        args.push(payload_path.to_string_lossy().into_owned());
        if self.cfg.backend == RenderBackend::Godot
            && let Some(env) = &self.cfg.dclenv
        {
            args.push("--dclenv".into());
            args.push(env.clone());
        }
        args.extend(self.cfg.extra_args.iter().cloned());

        let mut cmd = Command::new(&bin);
        cmd.args(&args)
            .current_dir(self.cfg.work_root.as_deref().map_or(workdir, Path::new))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        cmd.process_group(0);
        if let Some(display) = &self.cfg.display {
            cmd.env("DISPLAY", display);
        }
        if self.cfg.backend == RenderBackend::Bevy {
            // the engine keeps its log, settings and asset cache under the XDG
            // dirs; the job's own keep concurrent renders apart and go with it
            let xdg = workdir.join("xdg");
            cmd.env("XDG_DATA_HOME", xdg.join("data"))
                .env("XDG_CACHE_HOME", xdg.join("cache"))
                .env("XDG_CONFIG_HOME", xdg.join("config"));
        }
        cmd.env("DCL_PEER_BASE", peer_base_of(content_base));

        tracing::debug!(entity = %entity, backend = self.cfg.backend.label(), bin = %bin.display(), args = ?args, "spawning the avatar renderer");

        let child = cmd.spawn().map_err(|e| RenderError::Spawn(e.to_string()))?;
        let pgid = child.id();

        let timeout = Duration::from_secs(self.cfg.timeout_seconds);
        let output = match tokio::time::timeout(timeout, child.wait_with_output()).await {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => return Err(RenderError::Spawn(e.to_string())),
            Err(_) => {
                if let Some(pgid) = pgid {
                    kill_process_group(pgid);
                }
                return Err(RenderError::Timeout(timeout));
            }
        };

        let body_ok = verify_output(&body_path, ImageKind::Body).await;
        let face_ok = verify_output(&face_path, ImageKind::Face).await;
        if let (Ok(()), Ok(())) = (&body_ok, &face_ok) {
            if !output.status.success() {
                tracing::warn!(status = %output.status, backend = self.cfg.backend.label(), "the renderer exited non-zero after writing both images; serving them");
            }
        } else if !output.status.success() {
            let tail = tail_of(&output.stderr, &output.stdout);
            return Err(RenderError::NonZero {
                status: output.status.to_string(),
                tail,
            });
        }
        body_ok?;
        face_ok?;

        Ok(RenderOutputs {
            body_path,
            face_path,
        })
    }
}

/// The engine appends its own `/content/` and `/lambdas/`, so it wants the
/// catalyst root rather than the content endpoint the payload carries.
fn peer_base_of(content_base: &str) -> String {
    let trimmed = content_base.trim_end_matches('/');
    trimmed
        .strip_suffix("/content")
        .unwrap_or(trimmed)
        .to_string()
}

/// SIGKILLs the whole group `spawn()` put the child in. Runs after kill_on_drop
/// has already SIGKILLed the group leader, so this is what actually reaches
/// what it spawned (godot's X server, the engine's scene sidecar). Best-effort: the group is simply gone when a
/// render exits between the timeout firing and this call, and ESRCH is the
/// ordinary result rather than a fault to report.
#[cfg(unix)]
fn kill_process_group(pgid: u32) {
    let pgid = pgid as i32;
    if pgid <= 0 {
        return;
    }
    unsafe {
        libc::kill(-pgid, libc::SIGKILL);
    }
}

/// No process groups here: kill_on_drop has already killed the child itself.
#[cfg(not(unix))]
fn kill_process_group(_pgid: u32) {}

const BLANK_BYTES_THRESHOLD: u64 = 3000;

async fn verify_output(path: &Path, kind: ImageKind) -> Result<(), RenderError> {
    match tokio::fs::metadata(path).await {
        Ok(m) if m.is_file() && m.len() >= BLANK_BYTES_THRESHOLD => Ok(()),
        _ => Err(RenderError::OutputMissing { kind: kind.label() }),
    }
}

fn tail_of(stderr: &[u8], stdout: &[u8]) -> String {
    let src = if stderr.is_empty() { stdout } else { stderr };
    let start = src.len().saturating_sub(2048);
    String::from_utf8_lossy(&src[start..]).trim().to_string()
}

#[cfg(test)]
mod peer_base_tests {
    use super::peer_base_of;

    #[test]
    fn the_content_endpoint_is_reduced_to_its_catalyst_root() {
        assert_eq!(
            peer_base_of("http://127.0.0.1:5141/content"),
            "http://127.0.0.1:5141"
        );
        assert_eq!(
            peer_base_of("http://127.0.0.1:5141/content/"),
            "http://127.0.0.1:5141"
        );
    }

    #[test]
    fn a_base_that_is_already_a_root_is_left_alone() {
        assert_eq!(
            peer_base_of("https://peer.example.org"),
            "https://peer.example.org"
        );
        assert_eq!(
            peer_base_of("https://peer.example.org/"),
            "https://peer.example.org"
        );
    }

    /// A path merely ending in the word "content" is not the content endpoint;
    /// stripping it would point the engine at a parent that serves nothing.
    #[test]
    fn only_a_trailing_content_segment_is_stripped() {
        assert_eq!(
            peer_base_of("https://host.example/my-content"),
            "https://host.example/my-content"
        );
        assert_eq!(
            peer_base_of("https://host.example/catalyst/content"),
            "https://host.example/catalyst"
        );
    }
}
