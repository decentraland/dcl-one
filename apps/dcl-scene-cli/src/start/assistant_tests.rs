use super::*;
use crate::start::testkit::Tmp;

#[test]
fn every_engine_keeps_permission_policy_and_uses_the_project() {
    let temp = Tmp::new("assistant-args");
    for id in provider::ENGINES {
        let invocation =
            provider::invocation(id, "Build a chair", None, None, &temp.0, None).unwrap();
        let args = invocation.args.join(" ");
        for bypass in [
            "bypassPermissions",
            "danger-full-access",
            "--yolo",
            "--force",
            "-f ",
        ] {
            assert!(!args.contains(bypass), "{id}: {args}");
        }
        assert!(args.contains("json"));
        assert!(args.contains("Build a chair") || invocation.stdin.contains("Build a chair"));
    }
    let codex =
        provider::invocation("codex", "Continue", Some("thread-1"), None, &temp.0, None).unwrap();
    assert_eq!(&codex.args[..3], ["exec", "resume", "thread-1"]);
    assert!(
        provider::invocation("gemini", "Continue", Some("unstable"), None, &temp.0, None).is_err()
    );
    assert!(!identifier("--danger"));
    let opencode =
        provider::invocation("opencode", "Continue", Some("ses_1"), None, &temp.0, None).unwrap();
    assert_eq!(
        &opencode.args[..5],
        ["run", "--format", "json", "--session", "ses_1"]
    );
    let hermes = provider::invocation(
        "hermes",
        "Continue",
        Some("20260923_1"),
        None,
        &temp.0,
        None,
    )
    .unwrap();
    assert!(
        hermes
            .args
            .windows(2)
            .any(|w| w == ["--resume", "20260923_1"])
    );
    assert!(hermes.stdin.is_empty() && hermes.args.last().unwrap().ends_with("Continue"));
}

#[test]
fn configured_setups_pick_a_wrapper_per_engine_and_replace_built_ins() {
    let _env = crate::deploy::ENV_LOCK.blocking_lock();
    let configured = provider::parse_config(
        r#"{"assistants":[
            {"id":"opencode-flash","label":"OpenCode (Cerebras)","engine":"opencode","command":"opencode-flash"},
            {"id":"hermes","engine":"hermes","command":"/opt/bin/hermes-local","model":"llm-default","args":["--toolsets","file,terminal"]}
        ]}"#,
    )
    .unwrap();
    assert_eq!(configured[0].engine, "opencode");
    assert_eq!(configured[0].label, "OpenCode (Cerebras)");
    assert_eq!(configured[1].label, "hermes");
    let temp = Tmp::new("assistant-config");
    let call = provider::invocation_for(&configured[1], "Hi", None, None, &temp.0, None).unwrap();
    assert_eq!(&call.args[..3], ["chat", "--toolsets", "file,terminal"]);
    assert!(
        call.args
            .windows(2)
            .any(|w| w == ["--model", "llm-default"])
    );
    let chosen =
        provider::invocation_for(&configured[1], "Hi", None, Some("other"), &temp.0, None).unwrap();
    assert!(chosen.args.windows(2).any(|w| w == ["--model", "other"]));
    for bad in [
        r#"{"assistants":[{"id":"x","engine":"aider","command":"aider"}]}"#,
        r#"{"assistants":[{"id":"--x","engine":"codex","command":"codex"}]}"#,
        r#"{"assistants":[{"id":"x","engine":"codex"}]}"#,
        r#"{"assistants":[{"id":"x","engine":"codex","command":"codex","args":"--yolo"}]}"#,
        r#"[]"#,
    ] {
        assert!(provider::parse_config(bad).is_err(), "{bad}");
    }
    let dir = Tmp::new("assistant-config-file");
    let path = dir.0.join("assistants.json");
    std::fs::write(&path, r#"{"assistants":[{"id":"codex","label":"Codex (local)","engine":"codex","command":"codex-local"},{"id":"opencode-flash","engine":"opencode","command":"opencode-flash"}]}"#).unwrap();
    crate::set_test_env(provider::CONFIG_ENV, &path);
    let (list, error) = provider::providers();
    crate::remove_test_env(provider::CONFIG_ENV);
    assert!(error.is_none());
    let codex = list.iter().find(|p| p.id == "codex").unwrap();
    assert_eq!(
        (codex.label.as_str(), codex.command.as_str()),
        ("Codex (local)", "codex-local")
    );
    assert_eq!(list.iter().filter(|p| p.id == "codex").count(), 1);
    assert_eq!(list.last().unwrap().id, "opencode-flash");
}

#[test]
fn provider_streams_normalize_text_tools_sessions_and_failures_without_final_duplicates() {
    assert_eq!(
        provider::events("codex", r#"{"type":"thread.started","thread_id":"one"}"#)[0]["sessionId"],
        "one"
    );
    assert_eq!(
        provider::events(
            "codex",
            r#"{"type":"item.completed","item":{"type":"agent_message","text":"Done"}}"#
        )[0]["text"],
        "Done"
    );
    assert_eq!(
        provider::events(
            "codex",
            r#"{"type":"turn.failed","error":{"message":"Sign in"}}"#
        )[0]["message"],
        "Sign in"
    );
    for id in ["claude", "cursor"] {
        assert_eq!(
            provider::events(
                id,
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello"}]}}"#
            )[0]["text"],
            "Hello"
        );
        let result = provider::events(
            id,
            r#"{"type":"result","session_id":"one","result":"Hello"}"#,
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["type"], "session");
    }
    assert_eq!(
        provider::events(
            "claude",
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"path":"src/index.ts"}}]}}"#
        )[0]["type"],
        "tool"
    );
    assert_eq!(
        provider::events(
            "cursor",
            r#"{"type":"tool_call","subtype":"started","tool_call":{"readToolCall":{"args":{"path":"src/index.ts"}}}}"#
        )[0]["type"],
        "tool"
    );
    assert_eq!(
        provider::events(
            "gemini",
            r#"{"type":"message","role":"assistant","content":"Hello"}"#
        )[0]["text"],
        "Hello"
    );
    assert!(
        provider::events(
            "gemini",
            r#"{"type":"message","role":"user","content":"Hello"}"#
        )
        .is_empty()
    );
    assert_eq!(
        provider::events(
            "gemini",
            r#"{"type":"result","status":"error","error":{"message":"No credentials"}}"#
        )[0]["message"],
        "No credentials"
    );
    assert_eq!(
        provider::events(
            "opencode",
            r#"{"type":"step_start","sessionID":"ses_1","part":{}}"#
        )[0]["sessionId"],
        "ses_1"
    );
    assert_eq!(
        provider::events(
            "opencode",
            r#"{"type":"text","sessionID":"ses_1","part":{"type":"text","text":"Blue."}}"#
        )[0]["text"],
        "Blue."
    );
    let tool = provider::events(
        "opencode",
        r#"{"type":"tool_use","part":{"tool":"read","state":{"input":{"filePath":"index.ts"}}}}"#,
    );
    assert_eq!(
        (tool[0]["name"].as_str(), tool[0]["type"].as_str()),
        (Some("read"), Some("tool"))
    );
    assert_eq!(
        provider::events(
            "opencode",
            r#"{"type":"error","error":{"name":"APIError","data":{"message":"Rate limited"}}}"#
        )[0]["message"],
        "Rate limited"
    );
    assert_eq!(
        provider::events(
            "hermes",
            r#"{"type":"system","subtype":"init","session_id":"20260923_1"}"#
        )[0]["sessionId"],
        "20260923_1"
    );
    assert_eq!(
        provider::events("hermes", r#"{"type":"text","text":"blue"}"#)[0]["text"],
        "blue"
    );
    assert_eq!(
        provider::events(
            "hermes",
            r#"{"type":"tool_use","name":"read_file","input":{"path":"index.ts"}}"#
        )[0]["name"],
        "read_file"
    );
    let done = provider::events(
        "hermes",
        r#"{"type":"result","session_id":"s","exit_code":0,"text":"blue"}"#,
    );
    assert_eq!(done.len(), 1);
    assert_eq!(
        provider::events(
            "hermes",
            r#"{"type":"result","session_id":"s","exit_code":1,"text":"No key"}"#
        )[1]["message"],
        "No key"
    );
}

#[test]
fn scene_tool_config_uses_sdk_proxy_without_credentials_and_restores_user_settings() {
    let temp = Tmp::new("assistant-mcp");
    for (id, path) in [
        ("cursor", ".cursor/mcp.json"),
        ("gemini", ".gemini/settings.json"),
    ] {
        let path = temp.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original = r#"{"mcpServers":{"mine":{"command":"my-server"}},"theme":"dark"}"#;
        std::fs::write(&path, original).unwrap();
        {
            let call = provider::invocation(
                id,
                "Hello",
                None,
                None,
                &temp.0,
                Some("http://localhost:8000/api/project/assistant/mcp"),
            )
            .unwrap();
            assert_eq!(call.overlays.len(), 1);
            let data: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            assert_eq!(data["theme"], "dark");
            assert_eq!(data["mcpServers"]["mine"]["command"], "my-server");
            assert!(data["mcpServers"]["creator-hub"]["headers"].is_null());
        }
        assert_eq!(std::fs::read_to_string(path).unwrap(), original);
    }
    let opencode_json = temp.0.join("opencode.json");
    std::fs::write(
        &opencode_json,
        r#"{"model":"mine/model","mcp":{"mine":{"type":"local"}}}"#,
    )
    .unwrap();
    {
        let _call = provider::invocation(
            "opencode",
            "Hello",
            None,
            None,
            &temp.0,
            Some("http://localhost:8000/api/project/assistant/mcp"),
        )
        .unwrap();
        let data: Value = serde_json::from_slice(&std::fs::read(&opencode_json).unwrap()).unwrap();
        assert_eq!(data["model"], "mine/model");
        assert_eq!(data["mcp"]["mine"]["type"], "local");
        assert_eq!(data["mcp"]["creator-hub"]["type"], "remote");
    }
    assert!(
        std::fs::read_to_string(&opencode_json)
            .unwrap()
            .contains("mine/model")
    );
    assert!(
        !std::fs::read_to_string(&opencode_json)
            .unwrap()
            .contains("creator-hub")
    );
    let codex = provider::invocation(
        "codex",
        "Hello",
        None,
        None,
        &temp.0,
        Some("http://localhost:8000/api/project/assistant/mcp"),
    )
    .unwrap();
    assert!(
        codex
            .args
            .iter()
            .any(|a| a.contains("mcp_servers.creator-hub.url"))
    );
    assert!(!codex.args.iter().any(|a| a.contains("bearer")));
}

#[cfg(unix)]
fn fake_child(script: &str, root: &std::path::Path) -> tokio::process::Child {
    spawn_fake(script, root, Stdio::null())
}

/// A fake CLI that stays up until it is killed: its stdin is a pipe the test
/// holds, so a `cat` in `script` blocks for as long as the handle lives.
#[cfg(unix)]
fn fake_child_held(
    script: &str,
    root: &std::path::Path,
) -> (tokio::process::Child, tokio::process::ChildStdin) {
    let mut child = spawn_fake(script, root, Stdio::piped());
    let stdin = child.stdin.take().expect("piped stdin");
    (child, stdin)
}

#[cfg(unix)]
fn spawn_fake(script: &str, root: &std::path::Path, stdin: Stdio) -> tokio::process::Child {
    use std::os::unix::process::CommandExt;
    let mut command = tokio::process::Command::new("sh");
    command
        .args(["-c", script])
        .current_dir(root)
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    command.as_std_mut().process_group(0);
    command.spawn().unwrap()
}

#[cfg(unix)]
#[tokio::test]
async fn fake_cli_writes_project_streams_output_and_reports_exit() {
    let temp = Tmp::new("assistant-stream");
    let child = fake_child(
        "printf authored > result.txt; printf '%s\\n' '{\"type\":\"thread.started\",\"thread_id\":\"test-session\"}' '{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"Saved\"}}'",
        &temp.0,
    );
    let (_cancel, rx) = watch::channel(false);
    let (tx, mut output) = mpsc::channel(64);
    run_child(child, "codex", rx, tx).await;
    let mut events = Vec::new();
    while let Some(event) = output.recv().await {
        events.push(event);
    }
    assert_eq!(events[0]["sessionId"], "test-session");
    assert_eq!(events[1]["text"], "Saved");
    assert_eq!(
        events.last().unwrap(),
        &json!({"type":"done","exitCode":0,"cancelled":false})
    );
    assert_eq!(
        std::fs::read_to_string(temp.0.join("result.txt")).unwrap(),
        "authored"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn wrapper_chatter_on_stderr_shows_only_when_the_run_fails() {
    let temp = Tmp::new("assistant-quiet");
    for (script, failed) in [
        (
            "echo 'compgen: command not found' >&2; printf '%s\\n' '{\"type\":\"text\",\"text\":\"Blue\"}'",
            false,
        ),
        ("echo 'no API key' >&2; exit 3", true),
    ] {
        let child = fake_child(script, &temp.0);
        let (_cancel, rx) = watch::channel(false);
        let (tx, mut output) = mpsc::channel(64);
        run_child(child, "hermes", rx, tx).await;
        let mut events = Vec::new();
        while let Some(event) = output.recv().await {
            events.push(event);
        }
        let errors: Vec<&Value> = events.iter().filter(|e| e["type"] == "error").collect();
        if failed {
            assert_eq!(errors[0]["message"], "no API key");
        } else {
            assert!(errors.is_empty(), "{events:?}");
            assert_eq!(events[0]["text"], "Blue");
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn cancelling_a_fake_cli_kills_the_turn_and_its_child_processes() {
    let temp = Tmp::new("assistant-cancel");
    let (child, _held) = fake_child_held(
        "printf '%s\\n' '{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"Ready\"}}'; cat >/dev/null; printf leaked > leaked.txt",
        &temp.0,
    );
    let (cancel, rx) = watch::channel(false);
    let (tx, mut output) = mpsc::channel(64);
    let task = tokio::spawn(async move {
        run_child(child, "codex", rx, tx).await;
    });
    assert_eq!(output.recv().await.unwrap()["text"], "Ready");
    cancel.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(30), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(output.recv().await.unwrap()["cancelled"], true);
    assert!(!temp.0.join("leaked.txt").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn closing_the_stream_cancels_the_cli() {
    let temp = Tmp::new("assistant-disconnect");
    let (child, _held) = fake_child_held("exec cat >/dev/null", &temp.0);
    let (_cancel, rx) = watch::channel(false);
    let (tx, output) = mpsc::channel(64);
    drop(output);
    tokio::time::timeout(Duration::from_secs(30), run_child(child, "codex", rx, tx))
        .await
        .unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn cancellation_works_when_a_browser_stops_reading_output() {
    let temp = Tmp::new("assistant-backpressure");
    let (child, _held) = fake_child_held(
        "i=0; while [ $i -lt 100 ]; do printf '%s\\n' '{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"More\"}}'; i=$((i+1)); done; exec cat >/dev/null",
        &temp.0,
    );
    let (cancel, rx) = watch::channel(false);
    let (tx, mut output) = mpsc::channel(1);
    let task = tokio::spawn(async move {
        run_child(child, "codex", rx, tx).await;
    });
    assert_eq!(output.recv().await.unwrap()["text"], "More");
    // The browser stops reading here: once the one slot is full, the turn
    // cannot get its next line out until something gives.
    crate::test_wait::until(Duration::from_secs(10), async || {
        (output.len() == output.max_capacity()).then_some(())
    })
    .await
    .expect("the turn never filled the output channel");
    cancel.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(30), task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn cancel_endpoint_returns_the_agreed_json_and_releases_the_active_turn() {
    let state = Arc::new(crate::start::testkit::state(vec![]));
    let (tx, rx) = watch::channel(false);
    *state.assistant.active.lock().unwrap() = Some(("turn-1".into(), tx));
    let (status, Json(body)) = cancel(State(state.clone()), Path("turn-1".into()))
        .await
        .unwrap();
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(body, json!({"turnId":"turn-1","cancelled":true}));
    assert!(*rx.borrow());
    drop(TurnGuard {
        state: state.clone(),
        id: "turn-1".into(),
    });
    assert!(state.assistant.active.lock().unwrap().is_none());
}

#[test]
fn scene_tools_cannot_implicitly_connect_to_remote_or_credential_bearing_urls() {
    for allowed in [
        "http://localhost:5196/mcp",
        "http://127.0.0.1:5196/mcp",
        "http://[::1]:5196/mcp",
    ] {
        assert!(local_mcp_url(allowed).is_some());
    }
    for denied in [
        "https://remote.example/mcp",
        "http://localhost.evil/mcp",
        "http://user:secret@localhost:5196/mcp",
        "http://localhost:5196/mcp?token=secret",
        "file:///tmp/mcp",
    ] {
        assert!(local_mcp_url(denied).is_none(), "{denied}");
    }
}
