use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// The agent CLIs whose argv and output the SDK speaks. A provider is one of these plus the
/// command that runs it, so every wrapper of a CLI (a sandboxed build, one pinned to another
/// model or backend) is a provider of the same engine.
pub(super) const ENGINES: [&str; 6] = ["codex", "claude", "cursor", "gemini", "opencode", "hermes"];

const BUILT_IN: [(&str, &str, &str); 6] = [
    ("codex", "Codex", "codex"),
    ("claude", "Claude", "claude"),
    ("cursor", "Cursor", "cursor-agent"),
    ("gemini", "Gemini", "gemini"),
    ("opencode", "OpenCode", "opencode"),
    ("hermes", "Hermes", "hermes"),
];

/// Where extra assistant setups come from: `DCL_SCENE_CLI_ASSISTANTS`, else
/// `$XDG_CONFIG_HOME/dcl-scene-cli/assistants.json` (`~/.config` by default). Per user, never per
/// project: a scene checked out from elsewhere must not choose what the SDK executes.
pub(super) const CONFIG_ENV: &str = "DCL_SCENE_CLI_ASSISTANTS";

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Provider {
    pub id: String,
    pub label: String,
    pub engine: &'static str,
    pub command: String,
    pub model: Option<String>,
    pub args: Vec<String>,
}

fn built_in() -> Vec<Provider> {
    BUILT_IN
        .iter()
        .map(|(id, label, bin)| Provider {
            id: (*id).into(),
            label: (*label).into(),
            engine: id,
            command: (*bin).into(),
            model: None,
            args: vec![],
        })
        .collect()
}

pub(super) fn config_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(CONFIG_ENV).filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(path));
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("dcl-scene-cli/assistants.json"))
}

/// Reads `{"assistants":[{"id","engine","command","label"?,"model"?,"args"?}]}`. An entry whose id
/// is a built-in one replaces it.
pub(super) fn parse_config(text: &str) -> Result<Vec<Provider>, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let entries = value["assistants"]
        .as_array()
        .ok_or("expected {\"assistants\": [...]}")?;
    entries
        .iter()
        .map(|entry| {
            let field = |name: &str| {
                entry[name]
                    .as_str()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
            };
            let id = field("id").ok_or("every assistant needs an \"id\"")?;
            if !super::assistant::identifier(id) {
                return Err(format!("{id}: an id is letters, digits and -_.:/"));
            }
            let engine = field("engine")
                .and_then(|e| ENGINES.iter().find(|known| **known == e))
                .ok_or(format!(
                    "{id}: \"engine\" must be one of {}",
                    ENGINES.join(", ")
                ))?;
            let command = field("command").ok_or(format!("{id}: \"command\" is required"))?;
            let model = field("model").map(String::from);
            if model
                .as_deref()
                .is_some_and(|m| !super::assistant::identifier(m))
            {
                return Err(format!("{id}: invalid \"model\""));
            }
            let args = match &entry["args"] {
                Value::Null => vec![],
                Value::Array(items) => items
                    .iter()
                    .map(|a| {
                        a.as_str()
                            .map(String::from)
                            .ok_or(format!("{id}: \"args\" holds strings"))
                    })
                    .collect::<Result<_, _>>()?,
                _ => return Err(format!("{id}: \"args\" is a list of strings")),
            };
            Ok(Provider {
                id: id.into(),
                label: field("label").unwrap_or(id).into(),
                engine,
                command: command.into(),
                model,
                args,
            })
        })
        .collect()
}

/// The built-in providers followed by the configured ones, and why the configuration was
/// ignored, if it was.
pub(super) fn providers() -> (Vec<Provider>, Option<String>) {
    let mut list = built_in();
    let Some(path) = config_path() else {
        return (list, None);
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (list, None),
        Err(e) => return (list, Some(format!("{}: {e}", path.display()))),
    };
    match parse_config(&text) {
        Ok(configured) => {
            for provider in configured {
                match list.iter_mut().find(|p| p.id == provider.id) {
                    Some(slot) => *slot = provider,
                    None => list.push(provider),
                }
            }
            (list, None)
        }
        Err(e) => (list, Some(format!("{}: {e}", path.display()))),
    }
}

pub(super) fn find(id: &str) -> Option<Provider> {
    providers().0.into_iter().find(|p| p.id == id)
}

/// What `start --talk` says about the assistant: a setups file it could not use, and no CLI
/// to take a turn.
pub(super) fn note_at_start() {
    if let (_, Some(error)) = providers() {
        crate::ux::note(format!("assistant setups ignored: {error}"));
    }
    if !any_available() {
        crate::ux::note(format!(
            "no assistant CLI on PATH: the Scene assistant panel will open but cannot take a turn until one of {} is installed",
            supported_clis()
        ));
    }
}

/// Whether any supported assistant CLI is on PATH.
pub(super) fn any_available() -> bool {
    providers()
        .0
        .iter()
        .any(|p| executable(&p.command).is_some())
}

/// The supported CLIs by executable name, for a "none found" line.
pub(super) fn supported_clis() -> String {
    BUILT_IN
        .iter()
        .map(|(_, _, bin)| *bin)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Engines whose stderr is chatter (wrapper warnings, session lines) rather than errors: it is
/// kept back and shown only when the run fails.
pub(super) fn quiet_stderr(engine: &str) -> bool {
    matches!(engine, "opencode" | "hermes")
}

/// A provider's invocation: the engine's argv with the setup's own arguments after the
/// subcommand and its model unless the turn names one.
pub(super) fn invocation_for(
    provider: &Provider,
    prompt: &str,
    session: Option<&str>,
    model: Option<&str>,
    root: &Path,
    mcp: Option<&str>,
) -> Result<Invocation, String> {
    let model = model
        .filter(|m| *m != "default")
        .or(provider.model.as_deref());
    let mut call = invocation(provider.engine, prompt, session, model, root, mcp)?;
    let at = usize::from(matches!(provider.engine, "codex" | "opencode" | "hermes"));
    call.args.splice(at..at, provider.args.iter().cloned());
    Ok(call)
}

pub(super) fn executable(name: &str) -> Option<PathBuf> {
    let explicit = name.contains('/').then(|| PathBuf::from(name));
    let candidates: Vec<PathBuf> = match explicit {
        Some(path) => vec![path],
        None => std::env::split_paths(&std::env::var_os("PATH")?)
            .map(|dir| dir.join(name))
            .collect(),
    };
    candidates.into_iter().find(|p| {
        let Ok(meta) = std::fs::metadata(p) else {
            return false;
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            meta.is_file() && meta.permissions().mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            meta.is_file()
        }
    })
}

pub(super) struct Invocation {
    pub args: Vec<String>,
    pub stdin: String,
    pub overlays: Vec<ConfigOverlay>,
}

pub(super) fn invocation(
    provider: &str,
    prompt: &str,
    session: Option<&str>,
    model: Option<&str>,
    root: &Path,
    mcp: Option<&str>,
) -> Result<Invocation, String> {
    let mut args: Vec<String> = match provider {
        "codex" => {
            let mut v = vec!["exec".into()];
            if let Some(session) = session {
                v.extend(["resume".into(), session.into()]);
            }
            v.extend(["--json".into(), "--skip-git-repo-check".into()]);
            v
        }
        "claude" => vec!["-p", "--output-format", "stream-json", "--verbose"]
            .into_iter()
            .map(String::from)
            .collect(),
        "cursor" => vec!["-p", "--output-format", "stream-json"]
            .into_iter()
            .map(String::from)
            .collect(),
        "gemini" => vec!["-o", "stream-json"]
            .into_iter()
            .map(String::from)
            .collect(),
        "opencode" => vec!["run", "--format", "json"]
            .into_iter()
            .map(String::from)
            .collect(),
        "hermes" => vec!["chat", "--format", "stream-json", "--source", "dcl-one"]
            .into_iter()
            .map(String::from)
            .collect(),
        _ => return Err("Unknown assistant provider".into()),
    };
    if let Some(session) = session {
        if provider == "gemini" {
            return Err(
                "Gemini turns cannot resume a stable session; include prior context in the prompt"
                    .into(),
            );
        }
        match provider {
            "codex" => {}
            "opencode" => args.extend(["--session".into(), session.into()]),
            _ => args.extend(["--resume".into(), session.into()]),
        }
    }
    if let Some(model) = model.filter(|m| *m != "default") {
        args.extend(["--model".into(), model.into()]);
    }
    let mut overlays = Vec::new();
    if let Some(url) = mcp {
        match provider {
            "codex" => {
                args.extend([
                    "-c".into(),
                    format!("mcp_servers.creator-hub.url={}", json!(url)),
                ]);
            }
            "claude" => {
                let server = json!({"type":"http","url":url});
                let path = std::env::temp_dir()
                    .canonicalize()
                    .unwrap_or_else(|_| std::env::temp_dir())
                    .join(format!(
                        "dcl-one-assistant-{:x}.json",
                        rand::random::<u64>()
                    ));
                overlays.push(ConfigOverlay::write(
                    path.clone(),
                    "mcpServers",
                    server,
                    false,
                )?);
                args.extend(["--mcp-config".into(), path.to_string_lossy().into_owned()]);
            }
            "cursor" | "gemini" | "opencode" => {
                let (path, key, server) = match provider {
                    "cursor" => (
                        root.join(".cursor/mcp.json"),
                        "mcpServers",
                        json!({"url":url}),
                    ),
                    "gemini" => (
                        root.join(".gemini/settings.json"),
                        "mcpServers",
                        json!({"httpUrl":url}),
                    ),
                    _ => (
                        root.join("opencode.json"),
                        "mcp",
                        json!({"type":"remote","url":url}),
                    ),
                };
                overlays.push(ConfigOverlay::write(path, key, server, true)?);
            }
            _ => {}
        }
    }
    let instructions = "You are editing this Decentraland SDK scene. Work in the current project. Use its existing dcl-scene-cli build/watch workflow. Edit authored source and composites, not generated bin/ or .dcl-one/ output. The SDK watcher rebuilds saved files. Use available creator-hub MCP scene tools to inspect the running editor; call session_status before scene tools. Explain permission or authentication failures clearly.";
    let text = format!("{instructions}\n\n{prompt}");
    let stdin = match provider {
        "codex" => {
            args.push("-".into());
            text
        }
        "claude" => text,
        "cursor" => {
            args.push(text);
            String::new()
        }
        "gemini" => {
            args.extend(["-p".into(), prompt.into()]);
            instructions.into()
        }
        "opencode" => {
            args.push(text);
            String::new()
        }
        "hermes" => {
            args.extend(["-q".into(), text]);
            String::new()
        }
        _ => unreachable!(),
    };
    Ok(Invocation {
        args,
        stdin,
        overlays,
    })
}

pub(super) struct ConfigOverlay {
    path: PathBuf,
    before: Option<Vec<u8>>,
    written: Vec<u8>,
    permissions: Option<std::fs::Permissions>,
}
impl ConfigOverlay {
    /// Sets `<key>.creator-hub` to `server` in the JSON file at `path`, merging into what is there
    /// when `merge`; dropping the overlay puts the file back as it was.
    fn write(path: PathBuf, key: &str, server: Value, merge: bool) -> Result<Self, String> {
        for ancestor in path.ancestors().take_while(|p| p.parent().is_some()) {
            if std::fs::symlink_metadata(ancestor).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err("MCP configuration cannot follow symbolic links".into());
            }
        }
        let before = match std::fs::read(&path) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        let permissions = std::fs::metadata(&path).ok().map(|m| m.permissions());
        let mut value = if merge {
            before
                .as_ref()
                .map(|b| serde_json::from_slice::<Value>(b))
                .transpose()
                .map_err(|e| e.to_string())?
                .unwrap_or(json!({}))
        } else {
            json!({})
        };
        if !value.is_object() || value.get(key).is_some_and(|v| !v.is_object()) {
            return Err("Existing MCP configuration must contain JSON objects".into());
        }
        if value.get(key).is_none() {
            value[key] = json!({});
        }
        value[key]["creator-hub"] = server;
        let written = serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let temporary =
            path.with_file_name(format!(".assistant-config-{:x}.tmp", rand::random::<u64>()));
        let outcome = (|| -> std::io::Result<()> {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            use std::io::Write;
            let mut file = options.open(&temporary)?;
            file.write_all(&written)?;
            file.sync_all()?;
            std::fs::rename(&temporary, &path)
        })();
        if let Err(error) = outcome {
            let _ = std::fs::remove_file(temporary);
            return Err(error.to_string());
        }
        Ok(Self {
            path,
            before,
            written,
            permissions,
        })
    }
}
impl Drop for ConfigOverlay {
    fn drop(&mut self) {
        if std::fs::read(&self.path).ok().as_ref() != Some(&self.written) {
            return;
        }
        if let Some(before) = &self.before {
            let _ = std::fs::write(&self.path, before);
            if let Some(permissions) = &self.permissions {
                let _ = std::fs::set_permissions(&self.path, permissions.clone());
            }
        } else {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

pub(super) fn events(provider: &str, line: &str) -> Vec<Value> {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return vec![];
    };
    let mut out = Vec::new();
    let kind = v["type"].as_str().unwrap_or("");
    let session = if provider == "codex" && kind == "thread.started" {
        v["thread_id"].as_str()
    } else if matches!(kind, "system" | "init" | "result") {
        v["session_id"].as_str()
    } else {
        None
    };
    if let Some(id) = session {
        out.push(json!({"type":"session","sessionId":id}));
    }
    if provider == "codex" && kind == "item.completed" {
        let item = &v["item"];
        match item["type"].as_str().unwrap_or("") {
            "agent_message" => out.push(json!({"type":"text","text":item["text"]})),
            "error" => out.push(json!({"type":"error","message":item["message"]})),
            "command_execution" => {
                out.push(json!({"type":"tool","name":"Run","detail":item["command"]}))
            }
            "file_change" => {
                out.push(json!({"type":"tool","name":"Edit","detail":item["changes"].to_string()}))
            }
            "mcp_tool_call" => {
                out.push(json!({"type":"tool","name":item["tool"],"detail":item["server"]}))
            }
            _ => {}
        }
    }
    if matches!(provider, "claude" | "cursor")
        && kind == "assistant"
        && let Some(blocks) = v["message"]["content"].as_array()
    {
        for b in blocks {
            match b["type"].as_str() {
                Some("text") => out.push(json!({"type":"text","text":b["text"]})),
                Some("tool_use") => out
                    .push(json!({"type":"tool","name":b["name"],"detail":b["input"].to_string()})),
                _ => {}
            }
        }
    }
    if provider == "cursor"
        && kind == "tool_call"
        && v["subtype"] == "started"
        && let Some(tools) = v["tool_call"].as_object()
    {
        for (name, detail) in tools {
            out.push(json!({"type":"tool","name":name,"detail":detail.to_string()}));
        }
    }
    if provider == "opencode" {
        match kind {
            "step_start" => {
                if let Some(id) = v["sessionID"].as_str() {
                    out.push(json!({"type":"session","sessionId":id}));
                }
            }
            "text" => out.push(json!({"type":"text","text":v["part"]["text"]})),
            "tool_use" => out.push(json!({"type":"tool","name":v["part"]["tool"],"detail":v["part"]["state"]["input"].to_string()})),
            _ => {}
        }
    }
    if provider == "hermes" {
        match kind {
            "text" => out.push(json!({"type":"text","text":v["text"]})),
            "tool_use" => out.push(json!({"type":"tool","name":v["name"],"detail":v["input"].to_string()})),
            "result" if v["exit_code"].as_i64().is_some_and(|c| c != 0) => out.push(json!({"type":"error","message":v["text"].as_str().filter(|t| !t.is_empty()).unwrap_or("Hermes turn failed")})),
            _ => {}
        }
    }
    if provider == "gemini" {
        if kind == "message" && v["role"] == "assistant" {
            out.push(json!({"type":"text","text":v["content"]}));
        }
        if kind == "tool_use" {
            out.push(
                json!({"type":"tool","name":v["tool_name"],"detail":v["parameters"].to_string()}),
            );
        }
    }
    if kind == "error"
        || kind == "turn.failed"
        || (kind == "result" && (v["is_error"] == true || v["status"] == "error"))
    {
        let message = v["message"]
            .as_str()
            .or(v["error"]["message"].as_str())
            .or(v["error"]["data"]["message"].as_str())
            .or(v["result"].as_str())
            .unwrap_or("Assistant turn failed");
        out.push(json!({"type":"error","message":message}));
    }
    out
}
