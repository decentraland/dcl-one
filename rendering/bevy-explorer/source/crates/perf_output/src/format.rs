//! The bytes of the benchmark's two files, in the formats its report parses. Both sinks (the file
//! on desktop, the page's stream on the web) take what is built here and add nothing, so a run's
//! CSV and log have the same bytes on every platform. The report (`benchmark report`,
//! `local-harness/benchmark`) reads both unchanged.
//!
//! The page writes one block of its own (`deploy/web/engine/perf_stream.js` `crashBlock`, for an
//! engine that stopped without handing over its own): [`exception`]'s layout for
//! [`ExceptionKind::Panic`] with no location, with a fixed message. Both are tested against the
//! same golden text.

use std::{borrow::Cow, fmt::Write};

use bevy::log::Level;
use common::autopilot::{EXIT_MARKER, LOG_TARGET};

/// The CSV's first line: quoted names, CRLF.
pub const CSV_HEADER: &str = "\"Frame\",\"CPU Time\",\"GPU Time\"\r\n";

/// Every exception's last line holds this: the benchmark's report counts an exception when it
/// sees it after the exception's type line. It is a format marker, not a stack frame.
pub const EXCEPTION_MARKER: &str = "DebugLogReportHandler:LogExceptionInternal";

/// Continuation lines of a multi-line message are indented by this, so none can look like an
/// entry line or a `Type: message` line.
const CONTINUATION_INDENT: &str = "    ";

/// What follows the first character of a marker that a message or a category holds, so the text
/// still reads the same and the parser, which matches the marker's bytes, does not find it.
const MARKER_BREAK: char = '\u{200B}';

/// One frame's CSV row: `frame,cpu_ms,gpu_ms`, CRLF. The times arrive in nanoseconds and are
/// written as exact decimal milliseconds in invariant form (`.` separator, no grouping, no
/// exponent, no trailing zeros), which `float.Parse` reads under any culture.
pub fn csv_row(out: &mut String, frame: u32, cpu_ns: u64, gpu_ns: u64) {
    let _ = write!(out, "{frame},");
    push_milliseconds(out, cpu_ns);
    out.push(',');
    push_milliseconds(out, gpu_ns);
    out.push_str("\r\n");
}

fn push_milliseconds(out: &mut String, nanoseconds: u64) {
    let (whole, fraction) = (nanoseconds / 1_000_000, nanoseconds % 1_000_000);
    let _ = write!(out, "{whole}");
    if fraction != 0 {
        let digits = format!("{fraction:06}");
        out.push('.');
        out.push_str(digits.trim_end_matches('0'));
    }
}

/// Milliseconds as the whole nanoseconds [`csv_row`] takes; 0 for anything that is not a
/// positive number.
pub fn milliseconds_to_nanoseconds(milliseconds: f64) -> u64 {
    if milliseconds.is_finite() && milliseconds > 0.0 {
        (milliseconds * 1e6).round() as u64
    } else {
        0
    }
}

/// What an exception block's second line calls the exception.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExceptionKind {
    /// An error-level log event.
    Error,
    /// A panic.
    Panic,
}

impl ExceptionKind {
    /// The type the harness groups by; it must end in `Exception` or `Error`.
    pub fn type_name(self) -> &'static str {
        match self {
            ExceptionKind::Error => "Error",
            ExceptionKind::Panic => "PanicException",
        }
    }
}

/// One log event as an entry line: `HH:MM:SS.mmm - <color=#RRGGBB>[CATEGORY]</color>: message`,
/// LF. The time is UTC. The colour carries the level, which the line has no place for; the
/// harness reads only its shape. A message of several lines continues on indented lines.
///
/// A message or category never holds a marker: scene errors, remote peers' text and content
/// servers' strings reach the log, and the parser counts by the markers' bytes. Both markers are
/// broken in them ([`MARKER_BREAK`]).
pub fn entry(out: &mut String, unix_ms: u64, level: Level, category: &str, message: &str) {
    push_entry(
        out,
        unix_ms,
        level,
        category,
        message,
        &[EXIT_MARKER, EXCEPTION_MARKER],
    );
}

/// Whether an event is the run's own exit-marker line, so far as its target and message say (the
/// caller checks info level): the target is [`LOG_TARGET`] and the message is [`EXIT_MARKER`],
/// ` at ` and a time, on one line. Only run control logs that target, and no text a scene or a
/// peer sends picks the target.
pub fn is_run_exit(category: &str, message: &str) -> bool {
    category == LOG_TARGET
        && message
            .strip_prefix(EXIT_MARKER)
            .is_some_and(|rest| rest.starts_with(" at ") && !rest.contains(['\r', '\n']))
}

/// The run's own exit-marker line, the one line of the log that keeps [`EXIT_MARKER`]: the
/// harness stops counting exceptions at it. An info entry under [`LOG_TARGET`]; the caller has
/// checked [`is_run_exit`].
pub fn run_exit(out: &mut String, unix_ms: u64, message: &str) {
    push_entry(
        out,
        unix_ms,
        Level::INFO,
        LOG_TARGET,
        message,
        &[EXCEPTION_MARKER],
    );
}

fn push_entry(
    out: &mut String,
    unix_ms: u64,
    level: Level,
    category: &str,
    message: &str,
    markers: &[&str],
) {
    let message = broken(message, markers);
    let mut lines = message_lines(&message);
    push_entry_line(
        out,
        unix_ms,
        color(level),
        &broken(category, markers),
        lines.next().unwrap_or(""),
    );
    push_continuation(out, lines);
}

/// `text` with each of `markers` broken, by [`MARKER_BREAK`] after its first character. No
/// marker overlaps itself or the break, so none is left or made.
fn broken<'a>(text: &'a str, markers: &[&str]) -> Cow<'a, str> {
    let mut text = Cow::Borrowed(text);
    for marker in markers {
        if text.contains(marker) {
            let first = marker.chars().next().map_or(0, char::len_utf8);
            let (head, tail) = marker.split_at(first);
            text = Cow::Owned(text.replace(marker, &format!("{head}{MARKER_BREAK}{tail}")));
        }
    }
    text
}

/// One exception: three kinds of line, written in one piece so no other thread's line lands
/// between them.
///
/// 1. an entry line with an empty message (the trailing space after the colon is part of it);
/// 2. `Type: message`;
/// 3. a line holding [`EXCEPTION_MARKER`], then `file:line` when the event has them.
///
/// The message's own further lines, if any, sit between 2 and 3, indented. As in [`entry`], the
/// message and the category never hold a marker; line 3's is the block's own.
pub fn exception(
    out: &mut String,
    unix_ms: u64,
    category: &str,
    kind: ExceptionKind,
    message: &str,
    location: Option<(&str, u32)>,
) {
    let markers = [EXIT_MARKER, EXCEPTION_MARKER];
    push_entry_line(
        out,
        unix_ms,
        color(Level::ERROR),
        &broken(category, &markers),
        "",
    );
    let message = broken(message, &markers);
    let mut lines = message_lines(&message);
    let _ = writeln!(out, "{}: {}", kind.type_name(), lines.next().unwrap_or(""));
    push_continuation(out, lines);
    out.push_str(EXCEPTION_MARKER);
    if let Some((file, line)) = location {
        let _ = write!(out, " {file}:{line}");
    }
    out.push('\n');
}

fn push_entry_line(out: &mut String, unix_ms: u64, color: &str, category: &str, message: &str) {
    let day_ms = unix_ms % 86_400_000;
    let _ = write!(
        out,
        "{:02}:{:02}:{:02}.{:03} - <color=#{color}>[",
        day_ms / 3_600_000,
        day_ms / 60_000 % 60,
        day_ms / 1000 % 60,
        day_ms % 1000
    );
    // the category ends at the first `]` the harness sees
    out.extend(category.chars().map(|c| match c {
        ']' | '\r' | '\n' => '_',
        c => c,
    }));
    let _ = writeln!(out, "]</color>: {message}");
}

fn push_continuation<'a>(out: &mut String, lines: impl Iterator<Item = &'a str>) {
    for line in lines {
        let _ = writeln!(out, "{CONTINUATION_INDENT}{line}");
    }
}

/// The message's lines under every line ending .NET's `ReadLine` knows (`\n`, `\r\n`, `\r`),
/// without the trailing blank ones.
fn message_lines(message: &str) -> impl Iterator<Item = &str> {
    message
        .trim_end_matches(['\r', '\n'])
        .split("\r\n")
        .flat_map(|chunk| chunk.split(['\r', '\n']))
}

fn color(level: Level) -> &'static str {
    match level {
        Level::ERROR => "FF5C5C",
        Level::WARN => "FFC857",
        Level::INFO => "DBD6C8",
        _ => "8C8C8C",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-26 13:14:15.678 UTC
    const NOW: u64 = 1_793_020_455_678;

    fn row(frame: u32, cpu_ns: u64, gpu_ns: u64) -> String {
        let mut out = String::new();
        csv_row(&mut out, frame, cpu_ns, gpu_ns);
        out
    }

    #[test]
    fn csv_header_has_the_harness_bytes() {
        assert_eq!(
            CSV_HEADER.as_bytes(),
            b"\"Frame\",\"CPU Time\",\"GPU Time\"\r\n"
        );
        assert_ne!(
            &CSV_HEADER.as_bytes()[..3],
            b"\xEF\xBB\xBF",
            "no byte-order mark"
        );
    }

    #[test]
    fn csv_rows_are_crlf_invariant_decimals_without_quotes_grouping_or_exponent() {
        assert_eq!(row(7, 16_666_700, 0), "7,16.6667,0\r\n");
        assert_eq!(row(1_234_567, 1_000_000, 12_500_000), "1234567,1,12.5\r\n");
        // nanosecond precision is kept, trailing zeros are not written
        assert_eq!(row(0, 1, 120_000), "0,0.000001,0.12\r\n");
        assert_eq!(row(1, 0, 0), "1,0,0\r\n");
        // no exponent, however long the frame: 25 minutes
        assert_eq!(row(2, 1_500_000_000_000, 0), "2,1500000,0\r\n");
        assert_eq!(
            row(u32::MAX, 999_999, 999_999_999),
            "4294967295,0.999999,999.999999\r\n"
        );
    }

    #[test]
    fn milliseconds_become_whole_nanoseconds() {
        assert_eq!(milliseconds_to_nanoseconds(9.0625), 9_062_500);
        assert_eq!(milliseconds_to_nanoseconds(0.0), 0);
        assert_eq!(milliseconds_to_nanoseconds(-1.0), 0);
        assert_eq!(milliseconds_to_nanoseconds(f64::NAN), 0);
        assert_eq!(milliseconds_to_nanoseconds(f64::INFINITY), 0);
    }

    fn entry_text(level: Level, category: &str, message: &str) -> String {
        let mut out = String::new();
        entry(&mut out, NOW, level, category, message);
        out
    }

    #[test]
    fn an_entry_is_one_lf_line_in_the_parsed_shape() {
        assert_eq!(
            entry_text(Level::INFO, "scene_runner", "ready"),
            "13:14:15.678 - <color=#DBD6C8>[scene_runner]</color>: ready\n"
        );
        // midnight, and a time with every field short
        let mut out = String::new();
        entry(&mut out, 86_400_000 * 20_000 + 5_006, Level::WARN, "a", "b");
        assert_eq!(out, "00:00:05.006 - <color=#FFC857>[a]</color>: b\n");
    }

    #[test]
    fn the_level_is_a_colour() {
        for (level, color) in [
            (Level::TRACE, "8C8C8C"),
            (Level::DEBUG, "8C8C8C"),
            (Level::INFO, "DBD6C8"),
            (Level::WARN, "FFC857"),
            (Level::ERROR, "FF5C5C"),
        ] {
            assert!(entry_text(level, "c", "m").contains(&format!("<color=#{color}>")));
        }
    }

    #[test]
    fn a_category_cannot_end_early_or_span_lines() {
        assert_eq!(
            entry_text(Level::INFO, "a]b\nc", "m"),
            "13:14:15.678 - <color=#DBD6C8>[a_b_c]</color>: m\n"
        );
    }

    #[test]
    fn an_empty_message_still_has_its_space_after_the_colon() {
        assert_eq!(
            entry_text(Level::INFO, "c", ""),
            "13:14:15.678 - <color=#DBD6C8>[c]</color>: \n"
        );
    }

    #[test]
    fn further_lines_are_indented_under_every_line_ending() {
        for message in ["a\nb\n\nc", "a\r\nb\r\n\r\nc", "a\rb\r\rc", "a\nb\n\nc\n\n"] {
            assert_eq!(
                entry_text(Level::INFO, "c", message),
                "13:14:15.678 - <color=#DBD6C8>[c]</color>: a\n    b\n    \n    c\n",
                "{message:?}"
            );
        }
    }

    fn exception_text(kind: ExceptionKind, message: &str, location: Option<(&str, u32)>) -> String {
        let mut out = String::new();
        exception(&mut out, NOW, "comms", kind, message, location);
        out
    }

    #[test]
    fn an_error_is_three_lines_the_first_ending_in_a_space() {
        let text = exception_text(
            ExceptionKind::Error,
            "connection refused",
            Some(("src/lib.rs", 505)),
        );
        assert_eq!(
            text,
            "13:14:15.678 - <color=#FF5C5C>[comms]</color>: \n\
             Error: connection refused\n\
             DebugLogReportHandler:LogExceptionInternal src/lib.rs:505\n"
        );
        assert!(text.starts_with("13:14:15.678 - <color=#FF5C5C>[comms]</color>: \nError: "));
    }

    /// A panic's text as the web's panic hook prints it (`console_error_panic_hook`'s layout:
    /// the message, then the JS stack).
    #[test]
    fn a_panic_is_the_same_three_lines_under_panic_exception() {
        let mut text = String::new();
        exception(
            &mut text,
            NOW,
            "panic",
            ExceptionKind::Panic,
            "panicked at src/lib.rs:5:9:\nseeded panic\n\nStack:\n\nError\n    at wasm.engine_run",
            None,
        );
        assert_eq!(
            text,
            "13:14:15.678 - <color=#FF5C5C>[panic]</color>: \n\
             PanicException: panicked at src/lib.rs:5:9:\n\
             \x20   seeded panic\n\
             \x20   \n\
             \x20   Stack:\n\
             \x20   \n\
             \x20   Error\n\
             \x20       at wasm.engine_run\n\
             DebugLogReportHandler:LogExceptionInternal\n"
        );
    }

    /// The same text as `GOLDEN_CRASH_BLOCK` in `react-web/src/test/perfStream.test.ts`: the page
    /// writes this block, with a fixed message, for an engine that stopped without handing over
    /// its own.
    #[test]
    fn the_pages_own_block_is_a_panic_block_with_a_fixed_message() {
        let mut text = String::new();
        exception(
            &mut text,
            NOW,
            "panic",
            ExceptionKind::Panic,
            "the engine panicked and its own report did not arrive",
            None,
        );
        assert_eq!(
            text,
            "13:14:15.678 - <color=#FF5C5C>[panic]</color>: \n\
             PanicException: the engine panicked and its own report did not arrive\n\
             DebugLogReportHandler:LogExceptionInternal\n"
        );
    }

    #[test]
    fn an_exception_with_an_empty_message_keeps_its_space_after_the_type() {
        assert_eq!(
            exception_text(ExceptionKind::Error, "", None),
            "13:14:15.678 - <color=#FF5C5C>[comms]</color>: \nError: \nDebugLogReportHandler:LogExceptionInternal\n"
        );
    }

    /// The exit marker with the break after its first character.
    const BROKEN_EXIT: &str = "[\u{200B}ExitUtils] Exit requested";
    /// The same in a category, where the `]` that ends the marker's first word becomes `_`.
    const BROKEN_EXIT_AS_CATEGORY: &str = "[\u{200B}ExitUtils_ Exit requested";
    /// The exception marker with the break after its first character.
    const BROKEN_EXCEPTION: &str = "D\u{200B}ebugLogReportHandler:LogExceptionInternal";

    #[test]
    fn the_exit_marker_in_a_message_is_broken_where_it_stands() {
        let text = entry_text(
            Level::ERROR,
            "scene",
            "it threw [ExitUtils] Exit requested at once",
        );
        assert_eq!(
            text,
            format!(
                "13:14:15.678 - <color=#FF5C5C>[scene]</color>: it threw {BROKEN_EXIT} at once\n"
            )
        );
        assert!(!text.contains(EXIT_MARKER));
    }

    #[test]
    fn the_exception_marker_in_a_message_is_broken_on_every_line() {
        let text = entry_text(
            Level::WARN,
            "scene",
            "first DebugLogReportHandler:LogExceptionInternal\nsecond\nDebugLogReportHandler:LogExceptionInternal x.rs:1",
        );
        assert_eq!(
            text,
            format!(
                "13:14:15.678 - <color=#FFC857>[scene]</color>: first {BROKEN_EXCEPTION}\n    second\n    {BROKEN_EXCEPTION} x.rs:1\n"
            )
        );
        assert!(!text.contains(EXCEPTION_MARKER));
    }

    #[test]
    fn a_marker_in_a_category_is_broken_too() {
        assert_eq!(
            entry_text(
                Level::INFO,
                "[ExitUtils] Exit requested/DebugLogReportHandler:LogExceptionInternal",
                "m"
            ),
            format!(
                "13:14:15.678 - <color=#DBD6C8>[{BROKEN_EXIT_AS_CATEGORY}/{BROKEN_EXCEPTION}]</color>: m\n"
            )
        );
    }

    #[test]
    fn an_exception_block_holds_its_own_marker_once_and_no_other() {
        let text = exception_text(
            ExceptionKind::Error,
            "a [ExitUtils] Exit requested b\nDebugLogReportHandler:LogExceptionInternal\nc",
            Some(("src/lib.rs", 7)),
        );
        assert_eq!(
            text,
            format!(
                "13:14:15.678 - <color=#FF5C5C>[comms]</color>: \n\
                 Error: a {BROKEN_EXIT} b\n    {BROKEN_EXCEPTION}\n    c\n\
                 DebugLogReportHandler:LogExceptionInternal src/lib.rs:7\n"
            )
        );
        let mut out = String::new();
        exception(
            &mut out,
            NOW,
            "[ExitUtils] Exit requested",
            ExceptionKind::Panic,
            "m",
            None,
        );
        assert_eq!(
            out,
            format!(
                "13:14:15.678 - <color=#FF5C5C>[{BROKEN_EXIT_AS_CATEGORY}]</color>: \n\
                 PanicException: m\n\
                 DebugLogReportHandler:LogExceptionInternal\n"
            )
        );
    }

    #[test]
    fn no_text_leaves_or_makes_a_marker_however_it_nests_them() {
        for message in [
            "[ExitUtils] Exit [ExitUtils] Exit requestedrequested",
            "DebugLogReportHandler:LogDebugLogReportHandler:LogExceptionInternalExceptionInternal",
            "[ExitUtils] Exit requested[ExitUtils] Exit requested",
            "DebugLogReportHandler:LogExceptionInternalDebugLogReportHandler:LogExceptionInternal",
            "[ExitUtils] Exit requestedDebugLogReportHandler:LogExceptionInternal",
            "[ExitUtils] Exit \u{200B}requested [\u{200B}ExitUtils] Exit requested",
            "[ExitUtils]\r\n Exit requested\r[ExitUtils] Exit requested\n",
        ] {
            let entry = entry_text(Level::INFO, message, message);
            assert!(
                !entry.contains(EXIT_MARKER) && !entry.contains(EXCEPTION_MARKER),
                "{entry:?}"
            );
            let block = exception_text(ExceptionKind::Error, message, None);
            assert!(!block.contains(EXIT_MARKER), "{block:?}");
            assert_eq!(block.matches(EXCEPTION_MARKER).count(), 1, "{block:?}");
            assert!(
                block.ends_with(&format!("\n{EXCEPTION_MARKER}\n")),
                "{block:?}"
            );
        }
    }

    #[test]
    fn only_the_run_control_target_with_the_exit_message_is_the_run_exit() {
        let exit = format!("{EXIT_MARKER} at 2026-10-26T13:14:15.678Z");
        assert!(is_run_exit("autopilot", &exit));
        for (category, message) in [
            ("scene_runner", exit.as_str()),
            ("autopilot", "phase Measuring at 3.0 s"),
            (
                "autopilot",
                "sign-in failed: [ExitUtils] Exit requested at x",
            ),
            ("autopilot", "[ExitUtils] Exit requested"),
            ("autopilot", "[ExitUtils] Exit requested at x\nsecond line"),
            ("autopilot", "[ExitUtils] Exit requested at x\r"),
            ("autopilot", " [ExitUtils] Exit requested at x"),
        ] {
            assert!(!is_run_exit(category, message), "{category} {message:?}");
        }
    }

    #[test]
    fn the_run_exit_is_the_one_entry_that_keeps_the_exit_marker() {
        let mut out = String::new();
        run_exit(
            &mut out,
            NOW,
            &format!("{EXIT_MARKER} at 2026-10-26T13:14:15.678Z"),
        );
        assert_eq!(
            out,
            "13:14:15.678 - <color=#DBD6C8>[autopilot]</color>: [ExitUtils] Exit requested at 2026-10-26T13:14:15.678Z\n"
        );
        assert_eq!(out.matches(EXIT_MARKER).count(), 1);

        // it is no way to write the other marker
        let mut out = String::new();
        run_exit(
            &mut out,
            NOW,
            "x DebugLogReportHandler:LogExceptionInternal",
        );
        assert!(!out.contains(EXCEPTION_MARKER), "{out:?}");
    }
}
