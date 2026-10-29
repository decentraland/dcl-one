//! The tracing layer that writes the benchmark's log: every event as an entry line, every
//! error-level event and every panic as an exception block ([`crate::format`]).

use std::fmt::{Debug, Write};

use bevy::log::{
    tracing::{
        field::{Field, Visit},
        Event, Subscriber,
    },
    tracing_subscriber::{layer::Context, Layer},
    Level,
};

use crate::{
    format::{entry, exception, is_run_exit, run_exit, ExceptionKind},
    scrub::credentials,
    sink::LogSink,
};

/// The target `log_panics` logs a panic under: the native panic hook goes through the `log`
/// crate, and the layer turns that one event into the panic's exception block instead of an
/// error's.
pub const PANIC_TARGET: &str = "panic";

pub struct PerfLogLayer {
    sink: Box<dyn LogSink>,
    clock: fn() -> u64,
}

impl PerfLogLayer {
    pub fn new(sink: impl LogSink) -> Self {
        Self::with_clock(sink, unix_milliseconds)
    }

    /// For tests: a layer whose lines carry the time `clock` gives.
    pub fn with_clock(sink: impl LogSink, clock: fn() -> u64) -> Self {
        Self {
            sink: Box::new(sink),
            clock,
        }
    }
}

/// Milliseconds since the Unix epoch.
pub fn unix_milliseconds() -> u64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as u64)
}

impl<S: Subscriber> Layer<S> for PerfLogLayer {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        if !self.sink.keeps() {
            return;
        }
        let mut fields = Fields::default();
        event.record(&mut fields);
        let metadata = event.metadata();
        // an event from the `log` crate (the panic hook, dependencies) carries its own target,
        // file and line as fields; the event's own are the bridge's
        let category = fields.log_target.as_deref().unwrap_or(metadata.target());
        let category = credentials(category);
        let category = &*category;
        let file = fields.log_file.as_deref().or(metadata.file());
        let line = fields.log_line.or(metadata.line());
        // a credential is never written, whichever code logged the string that holds it
        let message = fields.message();
        let message = credentials(&message);

        let mut out = String::with_capacity(message.len() + 128);
        let unix_ms = (self.clock)();
        if category == PANIC_TARGET {
            push_panic(&mut out, unix_ms, &message);
        } else if *metadata.level() == Level::ERROR {
            exception(
                &mut out,
                unix_ms,
                category,
                ExceptionKind::Error,
                &message,
                file.zip(line),
            );
        } else if *metadata.level() == Level::INFO && is_run_exit(category, &message) {
            run_exit(&mut out, unix_ms, &message);
        } else {
            entry(&mut out, unix_ms, *metadata.level(), category, &message);
        }
        self.sink.write(out.as_bytes());
    }
}

/// A panic's exception block, under [`PANIC_TARGET`] and with no location: the panic's own is in
/// its message, and an event's would be the hook's. `message` is already scrubbed.
fn push_panic(out: &mut String, unix_ms: u64, message: &str) {
    exception(
        out,
        unix_ms,
        PANIC_TARGET,
        ExceptionKind::Panic,
        message,
        None,
    );
}

/// A panic as the exception block the layer writes for the native panic hook's event: `message`
/// (what the panic printed) with every credential replaced, then [`crate::format::exception`]. For
/// a panic hook that writes the block itself: the web's (`src/web.rs`), whose thread traps right
/// after its hook and logs nothing more.
pub fn panic_block(unix_ms: u64, message: &str) -> String {
    let message = credentials(message);
    let mut out = String::with_capacity(message.len() + 128);
    push_panic(&mut out, unix_ms, &message);
    out
}

/// An event's fields: the message, the other fields as ` name=value` after it (as the console
/// layer prints them), and the `log` bridge's own.
#[derive(Default)]
struct Fields {
    message: String,
    others: String,
    log_target: Option<String>,
    log_file: Option<String>,
    log_line: Option<u32>,
}

impl Fields {
    fn message(&self) -> String {
        if self.message.is_empty() {
            self.others.trim_start().to_owned()
        } else {
            format!("{}{}", self.message, self.others)
        }
    }
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "message" => self.message.push_str(value),
            "log.target" => self.log_target = Some(value.to_owned()),
            "log.file" => self.log_file = Some(value.to_owned()),
            _ => self.record_debug(field, &value),
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() == "log.line" {
            self.log_line = u32::try_from(value).ok();
        } else {
            self.record_debug(field, &value);
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        match field.name() {
            "message" => {
                let _ = write!(self.message, "{value:?}");
            }
            name if name.starts_with("log.") => {}
            name => {
                let _ = write!(self.others, " {name}={value:?}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use bevy::log::{
        error, info, tracing,
        tracing_subscriber::{layer::SubscriberExt, Registry},
        warn,
    };

    use super::*;
    use crate::sink::FnSink;

    /// 2026-10-26 13:14:15.678 UTC
    const NOW: u64 = 1_793_020_455_678;

    /// What the layer writes for `log`, as text.
    fn written(log: impl FnOnce()) -> String {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let sink = {
            let captured = captured.clone();
            FnSink(move |bytes: &[u8]| captured.lock().unwrap().extend_from_slice(bytes))
        };
        let subscriber = Registry::default().with(PerfLogLayer::with_clock(sink, || NOW));
        tracing::subscriber::with_default(subscriber, log);
        let bytes = captured.lock().unwrap().clone();
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn an_event_is_one_entry_line_with_its_target_as_category() {
        let text = written(|| info!(target: "scene_runner", "loaded {} scenes", 3));
        assert_eq!(
            text,
            "13:14:15.678 - <color=#DBD6C8>[scene_runner]</color>: loaded 3 scenes\n"
        );
    }

    #[test]
    fn fields_follow_the_message() {
        let text = written(|| warn!(target: "ipfs", realm = "main", tries = 2, "slow"));
        assert_eq!(
            text,
            "13:14:15.678 - <color=#FFC857>[ipfs]</color>: slow realm=\"main\" tries=2\n"
        );
    }

    #[test]
    fn an_error_event_is_an_exception_block_with_its_location() {
        let text = written(|| error!(target: "comms", "connection refused"));
        let mut lines = text.lines();
        assert_eq!(
            lines.next(),
            Some("13:14:15.678 - <color=#FF5C5C>[comms]</color>: ")
        );
        assert_eq!(lines.next(), Some("Error: connection refused"));
        let marker = lines.next().unwrap();
        assert!(
            marker.starts_with("DebugLogReportHandler:LogExceptionInternal ")
                && marker.contains("layer.rs:"),
            "{marker}"
        );
        assert_eq!(lines.next(), None);
        // the empty entry line ends in a space
        assert!(text.starts_with("13:14:15.678 - <color=#FF5C5C>[comms]</color>: \n"));
    }

    #[test]
    fn an_event_from_the_log_bridge_takes_its_target_file_and_line_from_its_fields() {
        let text = written(|| {
            error!(
                log.target = "hyper_util",
                log.file = "src/client.rs",
                log.line = 42u64,
                "connect failed"
            )
        });
        assert_eq!(
            text,
            concat!(
                "13:14:15.678 - <color=#FF5C5C>[hyper_util]</color>: \n",
                "Error: connect failed\n",
                "DebugLogReportHandler:LogExceptionInternal src/client.rs:42\n"
            )
        );
    }

    #[test]
    fn the_panic_hooks_event_is_one_panic_block_not_an_error_block() {
        let text = written(|| {
            error!(
                log.target = "panic",
                log.file = "log-panics/src/lib.rs",
                log.line = 123u64,
                "thread 'main' panicked at 'seeded': src/main.rs:7\n   0: main\n   1: start"
            )
        });
        assert_eq!(
            text,
            concat!(
                "13:14:15.678 - <color=#FF5C5C>[panic]</color>: \n",
                "PanicException: thread 'main' panicked at 'seeded': src/main.rs:7\n",
                "       0: main\n",
                "       1: start\n",
                "DebugLogReportHandler:LogExceptionInternal\n"
            )
        );
        assert_eq!(text.matches("Exception: ").count(), 1);
        assert_eq!(text.matches("Error: ").count(), 0);
    }

    #[test]
    fn a_hooks_own_panic_block_is_the_layers_and_holds_no_credential() {
        let message = "panicked at src/lib.rs:5:9:\nconnect wss://h.example/r?access_token=HOOKSECRET\n\nStack:\n\nError\n    at wasm.engine_run";
        let block = panic_block(NOW, message);
        assert_eq!(block, written(|| error!(target: "panic", "{message}")));
        assert!(!block.contains("HOOKSECRET"), "{block}");
        assert_eq!(
            block,
            concat!(
                "13:14:15.678 - <color=#FF5C5C>[panic]</color>: \n",
                "PanicException: panicked at src/lib.rs:5:9:\n",
                "    connect wss://h.example/r?access_token=REDACTED\n",
                "    \n",
                "    Stack:\n",
                "    \n",
                "    Error\n",
                "        at wasm.engine_run\n",
                "DebugLogReportHandler:LogExceptionInternal\n"
            )
        );
    }

    #[test]
    fn a_room_credential_never_reaches_the_file_whatever_level_or_field_carries_it() {
        let text = written(|| {
            info!(target: "comms", "connecting to livekit:wss://h.example/r?access_token=INFOSECRET&room=a");
            warn!(target: "comms", adapter = "livekit:wss://h.example/r?access_token=FIELDSECRET", "unrecognised");
            error!(target: "comms", "failed [wss://h.example/r?access_token=ERRORSECRET]");
            error!(
                log.target = "livekit",
                log.file = "src/lib.rs",
                log.line = 1u64,
                "connect to wss://h.example/r?access_token=BRIDGESECRET failed"
            );
            error!(target: "panic", "panicked at src/lib.rs:1:1:
wss://h.example/r?access_token=PANICSECRET");
        });
        for secret in [
            "INFOSECRET",
            "FIELDSECRET",
            "ERRORSECRET",
            "BRIDGESECRET",
            "PANICSECRET",
        ] {
            assert!(
                !text.contains(secret),
                "{secret} reached the file:
{text}"
            );
        }
        assert!(text.contains(
            "livekit:wss://h.example/r?access_token=REDACTED&room=a
"
        ));
        assert!(text.contains(
            "adapter=\"livekit:wss://h.example/r?access_token=REDACTED\"
"
        ));
        assert!(text.contains(
            "Error: failed [wss://h.example/r?access_token=REDACTED]
"
        ));
    }

    #[test]
    fn a_private_key_a_signature_and_an_identity_never_reach_the_file() {
        use base64::Engine as _;
        let identity = base64::engine::general_purpose::STANDARD.encode(
            r#"{"ephemeralIdentity":{"address":"0xfake","privateKey":"0xKEYSECRET"},"authChain":[]}"#,
        );
        let text = written(|| {
            info!(target: "chat", "Command entered: `/login_identity`, with args: `[\"{identity}\", \"--guest\"]`");
            info!(target: "comms", "auth chain created: [{{\"type\":\"SIGNER\",\"signature\":\"\"}},{{\"type\":\"ECDSA_SIGNED_ENTITY\",\"signature\":\"0xSIGSECRET\"}}]");
            error!(target: "wallet", privateKey = "0xFIELDKEYSECRET", "identity {{\"privateKey\":\"0xJSONKEYSECRET\"}} rejected");
            info!(
                log.target = "wss://h.example/r?access_token=TARGETSECRET",
                "category check"
            );
        });
        for secret in [
            "KEYSECRET",
            "SIGSECRET",
            "FIELDKEYSECRET",
            "JSONKEYSECRET",
            "TARGETSECRET",
        ] {
            assert!(
                !text.contains(secret),
                "{secret} reached the file:
{text}"
            );
        }
        assert!(!text.contains(&identity), "{text}");
        assert_eq!(
            text.lines().next(),
            Some("13:14:15.678 - <color=#DBD6C8>[chat]</color>: Command entered: `/login_identity`, with args: `[\"REDACTED\", \"--guest\"]`")
        );
        assert!(text.contains(
            "[{\"type\":\"SIGNER\",\"signature\":\"\"},{\"type\":\"ECDSA_SIGNED_ENTITY\",\"signature\":\"REDACTED\"}]
"
        ));
        assert!(text.contains(
            "Error: identity {\"privateKey\":\"REDACTED\"} rejected privateKey=\"REDACTED\"
"
        ));
        assert!(text.contains(
            "[wss://h.example/r?access_token=REDACTED]</color>: category check
"
        ));
    }

    #[test]
    fn a_scene_error_holding_the_markers_neither_ends_the_count_nor_adds_to_it() {
        let text = written(|| {
            error!(
                target: "scene_runner",
                "[scene] error: [ExitUtils] Exit requested\nDebugLogReportHandler:LogExceptionInternal"
            )
        });
        let mut lines = text.lines();
        assert_eq!(
            lines.next(),
            Some("13:14:15.678 - <color=#FF5C5C>[scene_runner]</color>: ")
        );
        assert_eq!(
            lines.next(),
            Some("Error: [scene] error: [\u{200B}ExitUtils] Exit requested")
        );
        assert_eq!(
            lines.next(),
            Some("    D\u{200B}ebugLogReportHandler:LogExceptionInternal")
        );
        assert!(lines.next().unwrap().contains("layer.rs:"));
        assert_eq!(lines.next(), None);
        assert!(!text.contains(common::autopilot::EXIT_MARKER));
    }

    #[test]
    fn only_run_controls_own_info_line_keeps_the_exit_marker() {
        let exit = common::autopilot::EXIT_MARKER;
        let text = written(|| {
            info!(target: "autopilot", "{exit} at 2026-10-26T13:14:15.678Z");
            info!(target: "scene_runner", "{exit} at 2026-10-26T13:14:15.678Z");
            warn!(target: "autopilot", "{exit} at 2026-10-26T13:14:15.678Z");
            info!(target: "autopilot", "sign-in failed: {exit} at 2026-10-26T13:14:15.678Z");
            error!(target: "autopilot", "{exit} at 2026-10-26T13:14:15.678Z");
        });
        let carrying: Vec<_> = text
            .lines()
            .filter(|line| line.contains(common::autopilot::EXIT_MARKER))
            .collect();
        assert_eq!(
            carrying,
            vec![
                "13:14:15.678 - <color=#DBD6C8>[autopilot]</color>: [ExitUtils] Exit requested at 2026-10-26T13:14:15.678Z"
            ],
            "{text}"
        );
        assert_eq!(
            text.matches("[\u{200B}ExitUtils] Exit requested").count(),
            4
        );
    }

    #[test]
    fn a_multi_line_message_never_makes_a_line_that_looks_like_an_entry_or_a_type() {
        let text = written(|| {
            info!("first\n10:30:33.895 - <color=#DBD6C8>[FAKE]</color>: second\nError: third")
        });
        assert_eq!(
            text,
            concat!(
                "13:14:15.678 - <color=#DBD6C8>[perf_output::layer::tests]</color>: first\n",
                "    10:30:33.895 - <color=#DBD6C8>[FAKE]</color>: second\n",
                "    Error: third\n"
            )
        );
    }
}
