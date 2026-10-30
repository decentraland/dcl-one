//! What the runner prints. The job log is read by people and by the workflow runner, which
//! treats a line that starts with `::` as a command.

use std::fmt::Display;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

use rand::{rngs::OsRng, RngCore};

/// How much of a log is printed to the job log; the artifact has all of it.
pub const PRINTED_TAIL_BYTES: u64 = 1024 * 1024;

const SAFE_CHARS: usize = 200;

pub fn log(message: impl Display) {
    println!("[benchmark] {message}");
}

pub fn warn(message: impl Display) {
    println!("::warning::[benchmark] {message}");
}

/// Text that came from the page or the command line (a request path, a body, an address) as one
/// printable line: it goes into the job log, where a line starting with `::` is a workflow
/// command.
pub fn safe(text: &str) -> String {
    let shown: String = text
        .chars()
        .take(SAFE_CHARS)
        .map(|c| if c.is_control() { '?' } else { c })
        .collect();
    if text.chars().count() > SAFE_CHARS {
        shown + "..."
    } else {
        shown
    }
}

/// Prints the tail of a run's log. The text comes from the page (scene and network text reach a
/// log), so the runner's workflow commands (`::set-env::` and the like) are switched off while
/// it prints, and only the tail is printed: a page controls how long its log is.
pub fn print_file(path: &Path) -> io::Result<()> {
    write_file(path, &mut io::stdout().lock())
}

fn write_file(path: &Path, out: &mut impl Write) -> io::Result<()> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let length = file.metadata()?.len();
    let from = length.saturating_sub(PRINTED_TAIL_BYTES);
    file.seek(SeekFrom::Start(from))?;
    let mut bytes = Vec::new();
    file.take(length - from).read_to_end(&mut bytes)?;

    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if from > 0 {
        // Start at a line, not in the middle of one.
        if let Some(newline) = text.find('\n') {
            text.drain(..=newline);
        }
    }

    let mut token = [0u8; 16];
    OsRng.fill_bytes(&mut token);
    let token = hex(&token);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();

    writeln!(out, "::stop-commands::{token}")?;
    if from > 0 {
        writeln!(
            out,
            "----- {name} (the last {} KB of {length} B; the artifact has all of it)",
            PRINTED_TAIL_BYTES / 1024
        )?;
    } else {
        writeln!(out, "----- {name}")?;
    }
    writeln!(out, "{text}")?;
    writeln!(out, "-----")?;
    writeln!(out, "::{token}::")
}

pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_keeps_one_printable_line() {
        assert_eq!(safe("a\nb::c\r"), "a?b::c?");
    }

    #[test]
    fn safe_cuts_long_text() {
        let shown = safe(&"x".repeat(300));
        assert_eq!(shown.len(), SAFE_CHARS + 3);
        assert!(shown.ends_with("..."));
    }

    #[test]
    fn hex_is_lowercase_pairs() {
        assert_eq!(hex(&[0, 15, 255]), "000fff");
    }

    fn printed(name: &str, content: &[u8]) -> String {
        let path = crate::scratch("out", name).join("run.log");
        std::fs::write(&path, content).unwrap();
        let mut out = Vec::new();
        write_file(&path, &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn a_log_is_printed_between_a_stop_and_a_resume_of_workflow_commands() {
        let text = printed("whole", b"first\n::set-env name=A::b\nlast\n");
        let lines: Vec<&str> = text.lines().collect();

        let token = lines[0]
            .strip_prefix("::stop-commands::")
            .expect("commands stop first");
        assert_eq!(token.len(), 32);
        assert_eq!(lines[1], "----- run.log");
        assert_eq!(&lines[2..5], ["first", "::set-env name=A::b", "last"]);
        assert_eq!(lines.last().copied(), Some(format!("::{token}::").as_str()));
        assert!(
            lines.iter().position(|l| l.starts_with("::set-env"))
                < lines.iter().position(|l| *l == format!("::{token}::")),
            "the page's text is inside the stopped region"
        );
    }

    #[test]
    fn a_long_log_is_printed_from_a_line_inside_its_last_megabyte() {
        let mut log = String::new();
        let mut at = 0;
        while log.len() < 3 * 1024 * 1024 {
            log += &format!("{at:08} a line of the log\n");
            at += 1;
        }
        let text = printed("long", log.as_bytes());

        let header = text.lines().nth(1).unwrap();
        assert!(
            header.starts_with("----- run.log (the last 1024 KB of ")
                && header.ends_with("; the artifact has all of it)"),
            "{header}"
        );
        let body: Vec<&str> = text.lines().skip(2).collect();
        // the log ends in a newline, so an empty line, `-----` and the resume line follow it
        let printed = &body[..body.len() - 3];
        // 27 bytes a line: a megabyte holds about 38,800 of them, less the line it starts inside
        assert!(
            printed.len() > 38_000 && printed.len() < 39_000,
            "{}",
            printed.len()
        );
        assert!(
            printed.iter().all(|line| line.len() == 26),
            "no line is cut"
        );
        assert!(
            log.ends_with(&format!("{}\n", printed.last().unwrap())),
            "it is the tail"
        );
        let skipped = log.lines().count() - printed.len();
        assert!(skipped > 75_000, "{skipped} lines were left out");
    }

    #[test]
    fn a_log_that_is_not_there_prints_nothing() {
        let mut out = Vec::new();
        write_file(&crate::scratch("out", "missing").join("none.log"), &mut out).unwrap();
        assert!(out.is_empty());
    }
}
