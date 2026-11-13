//! A watcher's type checker: one node process per scene that keeps tsc
//! loaded and the last program in memory (`check_server.js`), so the check
//! after a save costs what the edit touched rather than a node start, a
//! compiler load and a parse of every declaration file.
//!
//! It runs tsc's own command line with the arguments a spawned tsc gets, so
//! the report is the same text. Whenever it cannot serve — TypeScript 7.x,
//! whose `tsc.js` launches a native binary, a compiler without the functions
//! it drives, a process that died — [`Checker::check`] answers `None` and the
//! caller spawns tsc as a build does.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};

const SERVER: &str = include_str!("check_server.js");

/// What one tsc run said: its exit status and its stdout.
pub struct Run {
    pub code: i32,
    pub out: String,
}

struct Request {
    args: Vec<String>,
    /// A dropped receiver is a check nobody waits for any more (a newer save
    /// replaced it): it is skipped unless it is `unattended`.
    reply: oneshot::Sender<Run>,
    unattended: bool,
}

/// The compiler and scene a process serves; another of either is another
/// process.
#[derive(Clone, PartialEq)]
struct Key {
    tsc: PathBuf,
    /// Size and mtime of `tsc.js` and its `_tsc.js`: an install under a
    /// running preview replaces the loaded compiler.
    tsc_files: Vec<Option<(u64, std::time::SystemTime)>>,
    root: PathBuf,
}

impl Key {
    fn of(tsc: &Path, root: &Path) -> Self {
        let stat = |path: &Path| {
            let meta = std::fs::metadata(path).ok()?;
            Some((meta.len(), meta.modified().ok()?))
        };
        Key {
            tsc: tsc.to_path_buf(),
            tsc_files: vec![stat(tsc), stat(&tsc.with_file_name("_tsc.js"))],
            root: root.to_path_buf(),
        }
    }
}

#[derive(Default)]
enum State {
    #[default]
    Idle,
    Serving {
        key: Key,
        requests: mpsc::UnboundedSender<Request>,
    },
    /// This compiler has no checker; tsc is spawned until it changes.
    Off(Key),
}

/// A handle on the process; clones share it. The process ends when the last
/// handle drops, when its stdin closes, or with this one (`kill_on_drop`).
#[derive(Clone, Default)]
pub struct Checker {
    state: Arc<Mutex<State>>,
}

impl Checker {
    fn requests(
        &self,
        node: &Path,
        tsc: &Path,
        root: &Path,
    ) -> Option<(Key, mpsc::UnboundedSender<Request>, bool)> {
        let key = Key::of(tsc, root);
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match &*state {
            State::Serving { key: k, requests } if *k == key && !requests.is_closed() => {
                return Some((key, requests.clone(), false));
            }
            State::Off(k) if *k == key => return None,
            _ => {}
        }
        let (requests, queue) = mpsc::unbounded_channel();
        tokio::spawn(serve(node.to_path_buf(), key.clone(), queue));
        *state = State::Serving {
            key: key.clone(),
            requests: requests.clone(),
        };
        Some((key, requests, true))
    }

    fn turn_off(&self, key: Key) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(&*state, State::Serving { key: k, .. } if *k == key) {
            *state = State::Off(key);
        }
    }

    /// Run tsc with `args` in `root`. `None` when there is no checker for
    /// this compiler: spawn tsc instead.
    pub async fn check(
        &self,
        node: &Path,
        tsc: &Path,
        root: &Path,
        args: Vec<String>,
    ) -> Option<Run> {
        let (key, requests, _) = self.requests(node, tsc, root)?;
        let (reply, answer) = oneshot::channel();
        let sent = requests.send(Request {
            args,
            reply,
            unattended: false,
        });
        match (sent, answer.await) {
            (Ok(()), Ok(run)) => Some(run),
            _ => {
                tracing::info!("the type checker process is not available; spawning tsc");
                self.turn_off(key);
                None
            }
        }
    }

    /// Start the process if this scene has none and have it build its
    /// program with `args`, unreported, so the first save after an
    /// unchanged start is already an incremental check.
    pub fn prime(&self, node: &Path, tsc: &Path, root: &Path, args: Vec<String>) {
        if let Some((_, requests, true)) = self.requests(node, tsc, root) {
            let (reply, _) = oneshot::channel();
            let _ = requests.send(Request {
                args,
                reply,
                unattended: true,
            });
        }
    }
}

#[derive(serde::Deserialize)]
struct Reply {
    #[serde(default)]
    ready: bool,
    id: Option<u64>,
    code: Option<i32>,
    out: Option<String>,
}

/// Owns the process: one request at a time, in order. Returning drops the
/// queue, which every waiting [`Checker::check`] reads as "spawn tsc".
async fn serve(node: PathBuf, key: Key, mut queue: mpsc::UnboundedReceiver<Request>) {
    let spawned = tokio::process::Command::new(&node)
        .arg("-e")
        .arg(SERVER)
        .arg(&key.tsc)
        .current_dir(&key.root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(e) => return tracing::info!("type checker process did not start: {e}"),
    };
    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        return;
    };
    let mut lines = BufReader::new(stdout).lines();
    let mut next = async || -> Option<Reply> {
        let line = lines.next_line().await.ok()??;
        serde_json::from_str(&line).ok()
    };
    if !next().await.is_some_and(|hello| hello.ready) {
        return tracing::info!("no type checker process for {}", key.tsc.display());
    }
    let mut id = 0u64;
    while let Some(request) = queue.recv().await {
        if request.reply.is_closed() && !request.unattended {
            continue;
        }
        id += 1;
        let line = serde_json::json!({ "id": id, "args": request.args }).to_string() + "\n";
        if stdin.write_all(line.as_bytes()).await.is_err() || stdin.flush().await.is_err() {
            return;
        }
        match next().await {
            Some(Reply {
                id: Some(got),
                code: Some(code),
                out: Some(out),
                ..
            }) if got == id => {
                let _ = request.reply.send(Run { code, out });
            }
            _ => return tracing::info!("the type checker process failed a check"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in `tsc.js` with the entry line the server looks for and the
    /// functions it drives: it reports `src/index.ts`'s text as the "error"
    /// when that holds `bad`, and counts the programs built from scratch.
    const FAKE_TSC: &str = r#"
var sys = {
  write(s) { process.stdout.write(s); },
  exit(code) { process.exit(code); },
  args: process.argv.slice(2),
};
function noop() {}
var fresh = 0;
function readBuilderProgram(options, host) { fresh++; return undefined; }
function createIncrementalCompilerHost(options, system) {
  return { getSourceFile() { return undefined; }, readFile: (f) => require("fs").readFileSync(f, "utf8") };
}
function executeCommandLine(system, cb, args) {
  const options = { args: args.join(" ") };
  const host = createIncrementalCompilerHost(options, system);
  readBuilderProgram(options, host);
  const text = host.readFile("src/index.ts");
  if (args.includes("--throw")) throw new Error("boom");
  if (text.includes("bad")) system.write("src/index.ts(1,1): error TS1: " + text.trim() + " fresh=" + fresh + "\n");
  cb({ getProgram() { return {}; } });
  return system.exit(text.includes("bad") ? 2 : 0);
}
executeCommandLine(sys, noop, sys.args);
"#;

    fn scene(tag: &str, tsc: &str) -> (crate::scene::Tmp, PathBuf) {
        let t = crate::scene::Tmp::new(&format!("check-server-{tag}"));
        t.write("node_modules/typescript/lib/tsc.js", tsc);
        t.write("src/index.ts", "ok\n");
        let tsc = t.0.join("node_modules/typescript/lib/tsc.js");
        (t, tsc)
    }

    fn args(extra: &[&str]) -> Vec<String> {
        extra.iter().map(|s| s.to_string()).collect()
    }

    #[tokio::test]
    async fn one_process_answers_check_after_check_with_tsc_text_and_status() {
        let node = crate::build::find_node().expect("node on PATH");
        let (t, tsc) = scene("serves", FAKE_TSC);
        let checker = Checker::default();
        let run = checker
            .check(&node, &tsc, &t.0, args(&["-p", "x"]))
            .await
            .unwrap();
        assert_eq!((run.code, run.out.as_str()), (0, ""));

        t.write("src/index.ts", "bad one\n");
        let run = checker
            .check(&node, &tsc, &t.0, args(&["-p", "x"]))
            .await
            .unwrap();
        assert_eq!(run.code, 2);
        assert_eq!(run.out, "src/index.ts(1,1): error TS1: bad one fresh=1\n");

        // Other arguments are another program: the kept one is dropped.
        let run = checker
            .check(
                &node,
                &tsc,
                &t.0,
                args(&["-p", "x", "--ignoreDeprecations", "6.0"]),
            )
            .await
            .unwrap();
        assert!(run.out.ends_with("fresh=2\n"), "{}", run.out);

        t.write("src/index.ts", "ok again\n");
        let run = checker
            .check(
                &node,
                &tsc,
                &t.0,
                args(&["-p", "x", "--ignoreDeprecations", "6.0"]),
            )
            .await
            .unwrap();
        assert_eq!((run.code, run.out.as_str()), (0, ""));
    }

    #[tokio::test]
    async fn a_compiler_it_cannot_drive_or_a_failed_check_falls_back_to_spawning() {
        let node = crate::build::find_node().expect("node on PATH");
        // TypeScript 7's tsc.js: a launcher, no bundled compiler.
        let (t, tsc) = scene("native", "require('child_process')\n");
        let checker = Checker::default();
        assert!(checker.check(&node, &tsc, &t.0, args(&[])).await.is_none());
        assert!(checker.check(&node, &tsc, &t.0, args(&[])).await.is_none());

        // Installing a compiler it can drive brings the checker back.
        std::fs::write(&tsc, FAKE_TSC).unwrap();
        assert!(checker.check(&node, &tsc, &t.0, args(&[])).await.is_some());

        // A check that throws ends the process; nothing is reported from it.
        assert!(
            checker
                .check(&node, &tsc, &t.0, args(&["--throw"]))
                .await
                .is_none()
        );
        assert!(checker.check(&node, &tsc, &t.0, args(&[])).await.is_none());
    }

    #[tokio::test]
    async fn the_process_ends_with_its_last_handle() {
        let node = crate::build::find_node().expect("node on PATH");
        let (t, tsc) = scene("ends", FAKE_TSC);
        let pid_file = t.0.join("pid");
        let reports_pid = FAKE_TSC.replace(
            "var fresh = 0;",
            &format!(
                "var fresh = 0; require('fs').writeFileSync({:?}, String(process.pid));",
                pid_file.to_string_lossy()
            ),
        );
        std::fs::write(&tsc, reports_pid).unwrap();
        let checker = Checker::default();
        assert!(checker.check(&node, &tsc, &t.0, args(&[])).await.is_some());
        let pid = std::fs::read_to_string(&pid_file).unwrap();
        let alive = || Path::new("/proc").join(&pid).exists();
        if !Path::new("/proc/self").exists() {
            return;
        }
        assert!(alive());
        drop(checker);
        let gone = crate::test_wait::until(crate::test_wait::HANG_GUARD, async || {
            (!alive()).then_some(())
        })
        .await;
        assert!(gone.is_some(), "the checker process outlived its handle");
    }
}
