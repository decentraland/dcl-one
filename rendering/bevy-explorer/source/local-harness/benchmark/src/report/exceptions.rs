//! A run's exceptions, counted in its log as `ProduceSummary.cs` counts them and grouped as
//! `[CATEGORY] Type`.
//!
//! An exception is two entries in the log: one whose message is empty (its category in the colour
//! tag), then `Type: message` with its stack, ending in a line that holds [`EXCEPTION_MARKER`]. An
//! exception that carries its category in its own message (`Type: <color=#…>[CATEGORY]…`) is one
//! entry. Counting stops at [`EXIT_MARKER`]: what follows is teardown, which says nothing about the
//! measured session. The explorer writes the log in this shape (`docs/guidelines/benchmark.md`,
//! *Files*); the C#'s regular expressions are matched here by hand, character for character.

use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;

use super::Lines;

/// The run's exit line holds this; nothing after it is counted.
pub const EXIT_MARKER: &str = "[ExitUtils] Exit requested";
/// The last line of every exception holds this.
pub const EXCEPTION_MARKER: &str = "DebugLogReportHandler:LogExceptionInternal";

/// Exception counts by kind, in the order each kind was first seen.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Kinds(Vec<(String, u64)>);

impl Kinds {
    pub fn add(&mut self, kind: &str, count: u64) {
        match self.0.iter_mut().find(|(k, _)| k == kind) {
            Some((_, total)) => *total += count,
            None => self.0.push((kind.to_string(), count)),
        }
    }

    /// 0 for a kind not seen.
    pub fn get(&self, kind: &str) -> u64 {
        self.0
            .iter()
            .find(|(k, _)| k == kind)
            .map_or(0, |(_, count)| *count)
    }

    pub fn contains(&self, kind: &str) -> bool {
        self.0.iter().any(|(k, _)| k == kind)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, u64)> {
        self.0.iter().map(|(kind, count)| (kind.as_str(), *count))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Per-kind totals of several runs.
    pub fn combine<'a>(all: impl IntoIterator<Item = &'a Kinds>) -> Kinds {
        let mut total = Kinds::default();
        for kinds in all {
            for (kind, count) in kinds.iter() {
                total.add(kind, count);
            }
        }
        total
    }
}

/// The exceptions a run's log holds before its exit line. A run without a log has none.
pub fn count(log: &Path) -> io::Result<(u64, Kinds)> {
    if !log.is_file() {
        return Ok((0, Kinds::default()));
    }
    let mut count = 0;
    let mut kinds = Kinds::default();
    // The pair being read: the category of an empty-message entry, then the exception's type.
    let mut category: Option<String> = None;
    let mut kind: Option<String> = None;

    for line in Lines::new(BufReader::new(File::open(log)?)) {
        let line = line?;
        if line.contains(EXIT_MARKER) {
            break;
        }
        if let Some((entry_category, message)) = entry_start(&line) {
            // a new entry: an empty message is (likely) an exception's first entry; anything
            // else ends the pair
            category = message.is_empty().then(|| entry_category.to_string());
            kind = None;
        } else if let Some(name) = kind.is_none().then(|| exception_type(&line)).flatten() {
            // `System.Net.Http.HttpRequestException` is grouped as `HttpRequestException`
            kind = name.rsplit(['.', '+']).next().map(String::from);
            if let Some(inline) = inline_category(&line) {
                category = Some(inline.to_string());
            }
        } else if kind.is_some() && line.contains(EXCEPTION_MARKER) {
            let name = format!(
                "[{}] {}",
                category.as_deref().unwrap_or("UNKNOWN"),
                kind.as_deref().unwrap_or_default()
            );
            kinds.add(&name, 1);
            count += 1;
            category = None;
            kind = None;
        }
    }
    Ok((count, kinds))
}

/// `^\d\d:\d\d:\d\d\.\d\d\d - <color=#[0-9A-Fa-f]{6}>\[(?<cat>[^\]]+)\]</color>: (?<msg>.*)$`:
/// the category and the message of an entry line. `\d` is an ASCII digit here, where .NET also
/// takes other scripts' digits; the explorer writes the time in ASCII.
fn entry_start(line: &str) -> Option<(&str, &str)> {
    let time = line.as_bytes().get(..12)?;
    let time_shaped = time.iter().enumerate().all(|(i, b)| match i {
        2 | 5 => *b == b':',
        8 => *b == b'.',
        _ => b.is_ascii_digit(),
    });
    if !time_shaped {
        return None;
    }
    let rest = line[12..].strip_prefix(" - ")?;
    let (category, rest) = color_tag(rest)?;
    let message = rest.strip_prefix("</color>: ")?;
    Some((category, message))
}

/// `<color=#[0-9A-Fa-f]{6}>\[(?<cat>[^\]]+)\]` at the start of `text`: the category, and what
/// follows its `]`.
fn color_tag(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix("<color=#")?;
    let hex = rest.get(..6)?;
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let rest = rest[6..].strip_prefix(">[")?;
    let end = rest.find(']')?;
    (end > 0).then(|| (&rest[..end], &rest[end + 1..]))
}

/// `^(?<type>[\w.+`]*(?:Exception|Error)): `: the type of an exception's first line. `:` ends
/// the type, so it is the whole run of type characters the line starts with. `\w` is a letter, a
/// digit or `_` here; .NET's class differs only in rare Unicode marks and punctuation.
fn exception_type(line: &str) -> Option<&str> {
    let end = line
        .find(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '.' | '+' | '`')))
        .unwrap_or(line.len());
    let (name, rest) = line.split_at(end);
    (rest.starts_with(": ") && (name.ends_with("Exception") || name.ends_with("Error")))
        .then_some(name)
}

/// The first `<color=#…>[CATEGORY]` anywhere in the line.
fn inline_category(line: &str) -> Option<&str> {
    line.match_indices("<color=#")
        .find_map(|(at, _)| color_tag(&line[at..]).map(|(category, _)| category))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counted(name: &str, log: &str) -> (u64, Vec<(String, u64)>) {
        let path = crate::scratch("report-exceptions", name).join("run.log");
        std::fs::write(&path, log).unwrap();
        let (count, kinds) = count(&path).unwrap();
        (
            count,
            kinds.iter().map(|(k, n)| (k.to_string(), n)).collect(),
        )
    }

    fn kinds(list: &[(&str, u64)]) -> Vec<(String, u64)> {
        list.iter().map(|(k, n)| (k.to_string(), *n)).collect()
    }

    #[test]
    fn an_exception_is_an_empty_entry_its_type_and_the_marker() {
        let log = "10:00:01.000 - <color=#FF5C5C>[comms]</color>: \n\
                   System.Net.Http.HttpRequestException: Connection refused\n\
                   \x20  at Foo.BarException:Method()\n\
                   DebugLogReportHandler:LogExceptionInternal src/lib.rs:5\n\
                   10:00:02.000 - <color=#FF5C5C>[scene]</color>: \n\
                   Error: the scene threw\n\
                   DebugLogReportHandler:LogExceptionInternal\n\
                   10:00:03.000 - <color=#FF5C5C>[scene]</color>: \n\
                   Error: again\n\
                   DebugLogReportHandler:LogExceptionInternal\n";
        assert_eq!(
            counted("pairs", log),
            (
                3,
                kinds(&[("[comms] HttpRequestException", 1), ("[scene] Error", 2)])
            )
        );
    }

    #[test]
    fn a_category_in_the_exception_line_wins_and_none_is_unknown() {
        let log = "Some.Nested+EcsSystemException: <color=#DBD6C8>[ECS]</color> system failed\n\
                   DebugLogReportHandler:LogExceptionInternal\n\
                   10:00:02.000 - <color=#DBD6C8>[engine]</color>: a message\n\
                   PanicException: no category\n\
                   DebugLogReportHandler:LogExceptionInternal\n";
        assert_eq!(
            counted("inline", log),
            (
                2,
                kinds(&[
                    ("[ECS] EcsSystemException", 1),
                    ("[UNKNOWN] PanicException", 1)
                ])
            )
        );
    }

    #[test]
    fn counting_stops_at_the_exit_line_and_a_broken_marker_is_text() {
        let log = "10:00:00.000 - <color=#DBD6C8>[scene]</color>: says [\u{200B}ExitUtils] Exit requested\n\
                   10:00:01.000 - <color=#FF5C5C>[comms]</color>: \n\
                   Error: before\n\
                   DebugLogReportHandler:LogExceptionInternal\n\
                   10:00:02.000 - <color=#DBD6C8>[autopilot]</color>: [ExitUtils] Exit requested at 2026-10-28T10:00:02.000Z\n\
                   10:00:03.000 - <color=#FF5C5C>[comms]</color>: \n\
                   Error: teardown\n\
                   DebugLogReportHandler:LogExceptionInternal\n";
        assert_eq!(counted("exit", log), (1, kinds(&[("[comms] Error", 1)])));
    }

    #[test]
    fn a_marker_without_a_type_and_an_entry_between_count_nothing() {
        let log = "10:00:01.000 - <color=#FF5C5C>[comms]</color>: \n\
                   DebugLogReportHandler:LogExceptionInternal\n\
                   10:00:02.000 - <color=#FF5C5C>[comms]</color>: \n\
                   Error: first\n\
                   10:00:03.000 - <color=#DBD6C8>[comms]</color>: in between\n\
                   DebugLogReportHandler:LogExceptionInternal\n\
                   ErrorCode: x\n\
                   DebugLogReportHandler:LogExceptionInternal\n";
        assert_eq!(counted("unpaired", log), (0, Vec::new()));
    }

    #[test]
    fn a_lone_carriage_return_ends_a_line() {
        let log = "10:00:01.000 - <color=#FF5C5C>[comms]</color>: \rError: x\r\nDebugLogReportHandler:LogExceptionInternal";
        assert_eq!(counted("cr", log), (1, kinds(&[("[comms] Error", 1)])));
    }

    #[test]
    fn a_run_without_a_log_has_no_exception() {
        let path = crate::scratch("report-exceptions", "none").join("run.log");
        assert_eq!(count(&path).unwrap(), (0, Kinds::default()));
    }

    #[test]
    fn entry_lines_are_shaped_exactly() {
        let entry = "10:30:33.895 - <color=#DBD6C8>[ENGINE]</color>: message";
        assert_eq!(entry_start(entry), Some(("ENGINE", "message")));
        assert_eq!(
            entry_start("10:30:33.895 - <color=#DBD6C8>[a[b]</color>: "),
            Some(("a[b", ""))
        );
        for not_entry in [
            "    10:30:33.895 - <color=#DBD6C8>[ENGINE]</color>: indented",
            "10:30:33,895 - <color=#DBD6C8>[ENGINE]</color>: comma",
            "10:30:33.895 - <color=#DBD6CX>[ENGINE]</color>: not hex",
            "10:30:33.895 - <color=#DBD6C8>[]</color>: no category",
            "10:30:33.895 - <color=#DBD6C8>[ENGINE]</color>:no space",
            "10:30:3é.895 - <color=#DBD6C8>[ENGINE]</color>: é",
        ] {
            assert_eq!(entry_start(not_entry), None, "{not_entry}");
        }
    }

    #[test]
    fn a_type_ends_in_exception_or_error_before_a_colon_and_a_space() {
        assert_eq!(exception_type("Error: x"), Some("Error"));
        assert_eq!(exception_type("A.B+CException: x"), Some("A.B+CException"));
        assert_eq!(exception_type("Foo.BarException:Method()"), None);
        assert_eq!(exception_type("ErrorCode: x"), None);
        assert_eq!(exception_type("  Error: indented"), None);
    }
}
