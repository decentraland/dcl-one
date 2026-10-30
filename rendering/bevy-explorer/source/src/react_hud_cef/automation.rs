//! The HUD's test commands on the standalone (docs/guidelines/running.md, "Test commands"):
//! `/hud_list`, `/hud_read`, `/hud_wait`, `/hud_click` and `/hud_fill`. The page (react-web
//! `lib/hudAutomation.ts`) finds a control by role and accessible name and says whether a user
//! could click it; the engine then clicks and types with real input through CEF, the path the
//! mouse and keyboard take (route_mouse, cef_offscreen's keyboard forwarding). `/hud_wait` is
//! the page's own polling: the engine relays the request and waits for the reply. On the web the
//! page answers these commands itself, since there the engine cannot reach the DOM.

use std::collections::VecDeque;

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use cef_offscreen::prelude::{Browsers, EditCommand, HostEmitEvent};
use common::rpc::{RpcResultReceiver, RpcResultSender};
use console::{DoAddConsoleCommand, PendingConsoleResponses};
use serde_json::{json, Value};

use super::{PageEnvelope, ReactHudCef};
use crate::automation::input::{queue_typing, InputScript};

/// How long a request waits for the page's answer before it fails.
const PAGE_TIMEOUT_SECONDS: f64 = 10.0;
/// How long `/hud_wait` waits for its state unless told otherwise, in milliseconds. The page's
/// default (`DEFAULT_WAIT_TIMEOUT_MS` in `lib/hudAutomation.ts`) is the same.
const DEFAULT_WAIT_TIMEOUT_MS: f64 = 10_000.0;
/// What a wait's deadline allows the page beyond its own timeout to say that it timed out.
const WAIT_MARGIN_SECONDS: f64 = 5.0;
/// Frames between input steps, so a click's focus has landed before text is typed into it.
const STEP_FRAMES: u32 = 3;

pub(super) struct HudAutomationPlugin;

impl Plugin for HudAutomationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HudCalls>()
            .add_console_command::<HudListCommand, _>(hud_list_cmd)
            .add_console_command::<HudReadCommand, _>(hud_read_cmd)
            .add_console_command::<HudWaitCommand, _>(hud_wait_cmd)
            .add_console_command::<HudClickCommand, _>(hud_click_cmd)
            .add_console_command::<HudFillCommand, _>(hud_fill_cmd)
            .add_systems(Update, (expire_calls, play_hud_input, reread_filled))
            .add_observer(on_page_reply);
    }
}

enum Action {
    List,
    Read,
    /// wait for a control's state, for this many seconds
    Wait(f64),
    Click,
    /// type this text into the control the selector names
    Fill(String, Value),
    /// describe a control after a fill: the field as the typing left it
    Filled,
}

struct Call {
    id: u64,
    action: Action,
    reply: RpcResultSender<Result<String, String>>,
    sent_at: f64,
    /// how long the page may take to answer, in seconds
    timeout: f64,
}

enum Step {
    Click(Vec2),
    /// Select the focused field's text, so the typing that follows replaces it.
    SelectAll,
    /// Type the text as key presses (crate::automation::input), then describe the field again.
    Type {
        text: String,
        reply: RpcResultSender<Result<String, String>>,
        selector: Value,
    },
    Reply(RpcResultSender<Result<String, String>>, String),
}

/// A fill whose keys are being played: once they are, the page describes the field again.
struct Typing {
    done: RpcResultReceiver<Result<String, String>>,
    reply: RpcResultSender<Result<String, String>>,
    selector: Value,
}

/// Requests waiting for the page's answer, the input answered clicks and fills still owe, and the
/// fills being typed.
#[derive(Resource, Default)]
struct HudCalls {
    last_id: u64,
    waiting: Vec<Call>,
    input: VecDeque<Step>,
    typing: Vec<Typing>,
}

impl HudCalls {
    #[allow(clippy::too_many_arguments)]
    fn ask(
        &mut self,
        page: Entity,
        action: Action,
        selector: Value,
        now: f64,
        pending: &mut PendingConsoleResponses,
        responder: Option<bevy_console::ConsoleResponder>,
        commands: &mut Commands,
    ) {
        let (reply, rx) = RpcResultSender::channel();
        pending.push_receiver(rx, |result| result, responder);
        self.send(page, action, selector, reply, now, commands);
    }

    /// Asks the page, and answers `reply` with what it says.
    fn send(
        &mut self,
        page: Entity,
        action: Action,
        selector: Value,
        reply: RpcResultSender<Result<String, String>>,
        now: f64,
        commands: &mut Commands,
    ) {
        self.last_id += 1;
        let id = self.last_id;
        // `locate` also scrolls the control into view, for the click to come; `fill` does too, and
        // says when the control takes no text. A wait is the page's, answered when it is over.
        let op = match action {
            Action::List => "list",
            Action::Read | Action::Filled => "describe",
            Action::Wait(_) => "wait",
            Action::Click => "locate",
            Action::Fill(..) => "fill",
        };
        let timeout = match action {
            Action::Wait(seconds) => seconds + WAIT_MARGIN_SECONDS,
            _ => PAGE_TIMEOUT_SECONDS,
        };
        commands.trigger_targets(
            HostEmitEvent {
                id: "hudAutomation".to_owned(),
                payload: json!({ "id": id, "op": op, "selector": selector }).to_string(),
            },
            page,
        );
        self.waiting.push(Call {
            id,
            action,
            reply,
            sent_at: now,
            timeout,
        });
    }
}

/// The HUD webview, once its page is listening.
fn page(hud: &Option<Res<ReactHudCef>>) -> Result<Entity, &'static str> {
    match hud {
        Some(hud) if hud.page_seen => Ok(hud.hud),
        Some(_) => Err("the HUD page has not started yet"),
        None => Err("there is no HUD page"),
    }
}

/// The text after a `/hud_*` command: a JSON selector object, or just an accessible name, bare or
/// as a JSON string (what `window.engine.hudClick('Backpack')` sends).
fn selector_from(words: &[String]) -> Value {
    let text = words.join(" ");
    match serde_json::from_str::<Value>(&text) {
        Ok(selector @ Value::Object(_)) => selector,
        Ok(Value::String(name)) => json!({ "name": name }),
        _ => json!({ "name": text }),
    }
}

/// How long a `/hud_wait` selector's `timeout` says to wait, in milliseconds: the default when it
/// has none.
fn wait_timeout_ms(selector: &Value) -> Result<f64, String> {
    match selector.get("timeout") {
        None | Some(Value::Null) => Ok(DEFAULT_WAIT_TIMEOUT_MS),
        Some(timeout) => match timeout.as_f64() {
            Some(ms) if ms.is_finite() && ms >= 0.0 => Ok(ms),
            _ => Err("expected \"timeout\" to be a number of milliseconds".to_owned()),
        },
    }
}

/// Where to click a control the page described: its centre, once it was found and a user could
/// click it.
fn click_point(control: &Value) -> Result<Vec2, String> {
    if control.get("found").and_then(Value::as_bool) != Some(true) {
        return Err(control
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("no HUD control matches")
            .to_owned());
    }
    if control.get("actionable").and_then(Value::as_bool) != Some(true) {
        let reason = control
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("not actionable");
        return Err(format!("the control can't be clicked: {reason}"));
    }
    let number = |key: &str| control.get(key).and_then(Value::as_f64);
    match (number("x"), number("y"), number("width"), number("height")) {
        (Some(x), Some(y), Some(width), Some(height)) => Ok(Vec2::new(
            (x + width / 2.0) as f32,
            (y + height / 2.0) as f32,
        )),
        _ => Err("the page sent no bounds for the control".to_owned()),
    }
}

// --- /hud_list ---

/// List the HUD's visible named controls as JSON (`[{"role", "name"}]`), optionally of one role
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/hud_list")]
struct HudListCommand {
    /// only controls with this role (`button`, `link`, `textbox`, `checkbox`, `tab`, ...)
    role: Option<String>,
}

fn hud_list_cmd(
    mut input: ConsoleCommand<HudListCommand>,
    hud: Option<Res<ReactHudCef>>,
    mut calls: ResMut<HudCalls>,
    mut pending: ResMut<PendingConsoleResponses>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let page = match page(&hud) {
        Ok(page) => page,
        Err(e) => return input.reply_failed(e),
    };
    calls.ask(
        page,
        Action::List,
        json!({ "role": command.role }),
        time.elapsed_secs_f64(),
        &mut pending,
        input.take_responder(),
        &mut commands,
    );
}

// --- /hud_read ---

/// Describe the HUD control a selector names, as JSON (`{"found": false}` when none does). The
/// selector is the control's accessible name, or JSON `{"name", "role", "index", "exact"}`
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/hud_read")]
struct HudReadCommand {
    /// the accessible name, or a JSON selector
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 1..)]
    json: Vec<String>,
}

fn hud_read_cmd(
    mut input: ConsoleCommand<HudReadCommand>,
    hud: Option<Res<ReactHudCef>>,
    mut calls: ResMut<HudCalls>,
    mut pending: ResMut<PendingConsoleResponses>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let page = match page(&hud) {
        Ok(page) => page,
        Err(e) => return input.reply_failed(e),
    };
    calls.ask(
        page,
        Action::Read,
        selector_from(&command.json),
        time.elapsed_secs_f64(),
        &mut pending,
        input.take_responder(),
        &mut commands,
    );
}

// --- /hud_wait ---

/// Wait until the HUD control a selector names is in a state, then describe it as JSON
/// (`{"found": false}` for hidden). The selector is as /hud_read takes it, plus "state" (visible,
/// hidden, enabled or actionable; default visible) and "timeout" in ms (default 10000); fails if
/// the timeout passes first
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/hud_wait")]
struct HudWaitCommand {
    /// the accessible name, or a JSON selector with "state" and "timeout"
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 1..)]
    json: Vec<String>,
}

fn hud_wait_cmd(
    mut input: ConsoleCommand<HudWaitCommand>,
    hud: Option<Res<ReactHudCef>>,
    mut calls: ResMut<HudCalls>,
    mut pending: ResMut<PendingConsoleResponses>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let mut selector = selector_from(&command.json);
    let timeout_ms = match wait_timeout_ms(&selector) {
        Ok(ms) => ms,
        Err(e) => return input.reply_failed(e),
    };
    // the page waits as long as this request's deadline counts on
    if let Some(object) = selector.as_object_mut() {
        object.insert("timeout".to_owned(), json!(timeout_ms));
    }
    let page = match page(&hud) {
        Ok(page) => page,
        Err(e) => return input.reply_failed(e),
    };
    calls.ask(
        page,
        Action::Wait(timeout_ms / 1000.0),
        selector,
        time.elapsed_secs_f64(),
        &mut pending,
        input.take_responder(),
        &mut commands,
    );
}

// --- /hud_click ---

/// Click the HUD control a selector names, with a real mouse click at its centre, if it is
/// visible, enabled and not covered. The selector is as /hud_read takes it
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/hud_click")]
struct HudClickCommand {
    /// the accessible name, or a JSON selector
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 1..)]
    json: Vec<String>,
}

fn hud_click_cmd(
    mut input: ConsoleCommand<HudClickCommand>,
    hud: Option<Res<ReactHudCef>>,
    mut calls: ResMut<HudCalls>,
    mut pending: ResMut<PendingConsoleResponses>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let page = match page(&hud) {
        Ok(page) => page,
        Err(e) => return input.reply_failed(e),
    };
    calls.ask(
        page,
        Action::Click,
        selector_from(&command.json),
        time.elapsed_secs_f64(),
        &mut pending,
        input.take_responder(),
        &mut commands,
    );
}

// --- /hud_fill ---

/// Type into the HUD field a selector names: click it, select its text and replace it, as typing
/// would. JSON `{"name", "role", "index", "exact", "text"}`
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/hud_fill")]
struct HudFillCommand {
    /// a JSON selector with the text to type
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 1..)]
    json: Vec<String>,
}

fn hud_fill_cmd(
    mut input: ConsoleCommand<HudFillCommand>,
    hud: Option<Res<ReactHudCef>>,
    mut calls: ResMut<HudCalls>,
    mut pending: ResMut<PendingConsoleResponses>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let selector = selector_from(&command.json);
    let Some(text) = selector
        .get("text")
        .and_then(Value::as_str)
        .map(String::from)
    else {
        return input.reply_failed("expected JSON {\"name\": ..., \"text\": ...}");
    };
    let page = match page(&hud) {
        Ok(page) => page,
        Err(e) => return input.reply_failed(e),
    };
    calls.ask(
        page,
        Action::Fill(text, selector.clone()),
        selector,
        time.elapsed_secs_f64(),
        &mut pending,
        input.take_responder(),
        &mut commands,
    );
}

// The page's answer: {to: 'engine', kind: 'hudAutomationReply', id, result}, or `error` instead of
// `result` for a wait that timed out or could not run.
fn on_page_reply(trigger: Trigger<PageEnvelope>, mut calls: ResMut<HudCalls>) {
    let env = &trigger.event().0;
    if env.get("to").and_then(Value::as_str) != Some("engine")
        || env.get("kind").and_then(Value::as_str) != Some("hudAutomationReply")
    {
        return;
    }
    let Some(id) = env.get("id").and_then(Value::as_u64) else {
        return;
    };
    let Some(index) = calls.waiting.iter().position(|call| call.id == id) else {
        return;
    };
    let call = calls.waiting.remove(index);
    if let Some(error) = env.get("error").and_then(Value::as_str) {
        return call.reply.send(Err(error.to_owned()));
    }
    let result = env.get("result").cloned().unwrap_or(Value::Null);
    match call.action {
        Action::List | Action::Read | Action::Wait(_) => call.reply.send(Ok(result.to_string())),
        // the field as the typing left it: keys that went nowhere leave the old text
        Action::Filled => call.reply.send(Ok(trusted(result))),
        action => match click_point(&result) {
            Err(e) => call.reply.send(Err(e)),
            Ok(at) => {
                calls.input.push_back(Step::Click(at));
                if let Action::Fill(text, selector) = action {
                    calls.input.push_back(Step::SelectAll);
                    calls.input.push_back(Step::Type {
                        text,
                        reply: call.reply,
                        selector,
                    });
                } else {
                    calls
                        .input
                        .push_back(Step::Reply(call.reply, trusted(result)));
                }
            }
        },
    }
}

/// A control's description marked as acted on with real input: CEF's reaches the page as a user's
/// does, and the web says the same when a test runner drives the browser (hudAutomation.ts).
fn trusted(mut result: Value) -> String {
    if let Some(control) = result.as_object_mut() {
        control.insert("input".to_owned(), json!("trusted"));
    }
    result.to_string()
}

fn expire_calls(mut calls: ResMut<HudCalls>, time: Res<Time>) {
    let now = time.elapsed_secs_f64();
    let (expired, waiting): (Vec<Call>, Vec<Call>) = std::mem::take(&mut calls.waiting)
        .into_iter()
        .partition(|call| now - call.sent_at > call.timeout);
    calls.waiting = waiting;
    for call in expired {
        call.reply.send(Err(format!(
            "the HUD page did not answer within {} s",
            call.timeout
        )));
    }
}

fn play_hud_input(
    mut calls: ResMut<HudCalls>,
    hud: Option<Res<ReactHudCef>>,
    browsers: NonSend<Browsers>,
    script: Option<ResMut<InputScript>>,
    mut wait: Local<u32>,
) {
    if *wait > 0 {
        *wait -= 1;
        return;
    }
    let Some(step) = calls.input.pop_front() else {
        return;
    };
    *wait = STEP_FRAMES;
    let (Some(hud), Some(mut script)) = (hud, script) else {
        if let Step::Reply(reply, _) | Step::Type { reply, .. } = step {
            reply.send(Err("there is no HUD page".to_owned()));
        }
        return;
    };
    match step {
        Step::Click(at) => {
            browsers.send_mouse_move(&hud.hud, std::iter::empty::<&MouseButton>(), at, false);
            browsers.send_mouse_click(&hud.hud, at, MouseButton::Left, false, 1);
            browsers.send_mouse_click(&hud.hud, at, MouseButton::Left, true, 1);
        }
        Step::SelectAll => browsers.execute_edit_command(&hud.hud, EditCommand::SelectAll),
        // typed through the window's key events, like a keyboard: the first key replaces the
        // selection, and CEF gets each key as the page would from a person
        Step::Type {
            text,
            reply,
            selector,
        } => {
            let (typed, done) = RpcResultSender::channel();
            queue_typing(&mut script, &text, typed, String::new());
            calls.typing.push(Typing {
                done,
                reply,
                selector,
            });
        }
        Step::Reply(reply, output) => reply.send(Ok(output)),
    }
}

/// Once a fill's keys are played, asks the page for the field again, to answer with.
fn reread_filled(
    mut calls: ResMut<HudCalls>,
    hud: Option<Res<ReactHudCef>>,
    time: Res<Time>,
    mut commands: Commands,
) {
    if calls.typing.is_empty() {
        return;
    }
    for mut typing in std::mem::take(&mut calls.typing) {
        match typing.done.poll_once() {
            Ok(None) => calls.typing.push(typing),
            Ok(Some(Ok(_))) => match page(&hud) {
                Ok(page) => calls.send(
                    page,
                    Action::Filled,
                    typing.selector,
                    typing.reply,
                    time.elapsed_secs_f64(),
                    &mut commands,
                ),
                Err(e) => typing.reply.send(Err(e.to_owned())),
            },
            Ok(Some(Err(e))) => typing.reply.send(Err(e)),
            Err(()) => typing.reply.send(Err("the typing was dropped".to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_selector_is_json_or_a_name() {
        let words = |s: &str| s.split_whitespace().map(String::from).collect::<Vec<_>>();
        assert_eq!(
            selector_from(&words("Explore as guest")),
            json!({ "name": "Explore as guest" })
        );
        assert_eq!(
            selector_from(&words(r#"{"name":"Backpack","role":"button"}"#)),
            json!({ "name": "Backpack", "role": "button" })
        );
        // a name that happens to be valid JSON but not an object stays a name
        assert_eq!(selector_from(&words("42")), json!({ "name": "42" }));
    }

    #[test]
    fn a_json_string_is_the_name() {
        // `window.engine.hudClick('Backpack')` sends its argument through JSON.stringify
        let words = |s: &str| s.split_whitespace().map(String::from).collect::<Vec<_>>();
        assert_eq!(
            selector_from(&words(r#""Backpack""#)),
            json!({ "name": "Backpack" })
        );
        assert_eq!(
            selector_from(&words(r#""Explore as guest""#)),
            json!({ "name": "Explore as guest" })
        );
        // an array is neither a selector nor a string: the text stays the name
        assert_eq!(
            selector_from(&words(r#"["Backpack"]"#)),
            json!({ "name": r#"["Backpack"]"# })
        );
    }

    #[test]
    fn a_wait_lasts_ten_seconds_unless_told_otherwise() {
        assert_eq!(
            wait_timeout_ms(&json!({ "name": "Backpack" })),
            Ok(10_000.0)
        );
        assert_eq!(wait_timeout_ms(&json!({ "timeout": null })), Ok(10_000.0));
        assert_eq!(wait_timeout_ms(&json!({ "timeout": 2500 })), Ok(2500.0));
        assert_eq!(wait_timeout_ms(&json!({ "timeout": 0 })), Ok(0.0));
        let refused = Err("expected \"timeout\" to be a number of milliseconds".to_owned());
        assert_eq!(wait_timeout_ms(&json!({ "timeout": "soon" })), refused);
        assert_eq!(wait_timeout_ms(&json!({ "timeout": -1 })), refused);
    }

    #[test]
    fn a_control_is_clicked_at_its_centre_only_when_a_user_could() {
        let control = json!({
            "found": true, "actionable": true, "x": 10.0, "y": 20.0, "width": 100.0, "height": 40.0
        });
        assert_eq!(click_point(&control), Ok(Vec2::new(60.0, 40.0)));
        assert_eq!(
            click_point(&json!({ "found": false })),
            Err("no HUD control matches".to_owned())
        );
        assert_eq!(
            click_point(&json!({ "found": false, "error": "2 controls match" })),
            Err("2 controls match".to_owned())
        );
        assert_eq!(
            click_point(&json!({ "found": true, "actionable": false, "reason": "covered" })),
            Err("the control can't be clicked: covered".to_owned())
        );
    }
}
