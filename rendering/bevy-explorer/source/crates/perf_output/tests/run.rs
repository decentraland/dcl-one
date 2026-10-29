//! The benchmark's output end to end on the desktop: the app's logger (with the `log_panics`
//! hook `main.rs` installs) and the autopilot's phase, driven by a test-only system in place of
//! run control, write the two files `decentraland/performance-testing` reads.
//!
//! The run: arrives, measures 120 frames, ends, logs the exit marker. During it the log gets a
//! seeded error and a seeded panic (caught), and after the marker a teardown error the harness
//! must not count. Set `PERF_OUTPUT_DIR` to keep the files where `scripts/ProduceSummary.cs` can
//! read them (`change_build_1.csv` / `.log`).

use std::{collections::BTreeMap, path::PathBuf, time::Duration};

use bevy::{
    diagnostic::FrameCount,
    log::{Level, LogPlugin},
    prelude::*,
    time::TimeUpdateStrategy,
};
use common::autopilot::{AutopilotPhase, EXIT_MARKER};
use perf_output::{
    csv::{CsvDestination, CsvPlugin},
    format::EXCEPTION_MARKER,
    layer::PerfLogLayer,
    sink::{DeferredSink, FileSink},
};

static LOG: DeferredSink = DeferredSink::new();

const ARRIVED: u32 = 10;
const SEEDED: u32 = 30;
const ENDED: u32 = 130;
const FRAME: Duration = Duration::from_micros(16_500);

/// What run control and the code under measurement would do.
fn drive(mut phase: ResMut<AutopilotPhase>, frame: Res<FrameCount>) {
    match frame.0 {
        ARRIVED => *phase = AutopilotPhase::Measuring,
        SEEDED => {
            info!(target: "seeded", "a line with\nseveral lines, one like an entry:\n10:30:33.895 - <color=#DBD6C8>[FAKE]</color>: x");
            error!(target: "seeded", "seeded error");
            // a scene's error text may hold both markers; it counts as one error and ends nothing
            error!(target: "seeded", "scene threw: {EXIT_MARKER}\n{EXCEPTION_MARKER}");
            warn!(target: "seeded", "seeded warning");
            let panicked = std::panic::catch_unwind(|| panic!("seeded panic"));
            assert!(panicked.is_err());
        }
        ENDED => *phase = AutopilotPhase::Ending { exit_code: 0 },
        n if n == ENDED + 1 => {
            info!(target: "autopilot", "{EXIT_MARKER} at 2026-10-26T12:00:00.000Z");
            error!(target: "teardown", "an error while quitting");
        }
        _ => {}
    }
}

#[test]
fn a_seeded_run_leaves_the_files_the_harness_reads() {
    let (dir, keep) = match std::env::var_os("PERF_OUTPUT_DIR") {
        Some(dir) => (PathBuf::from(dir), true),
        None => (
            std::env::temp_dir().join(format!("perf_output_run_{}", std::process::id())),
            false,
        ),
    };
    std::fs::create_dir_all(&dir).unwrap();
    let (csv, log) = (
        dir.join("change_build_1.csv"),
        dir.join("change_build_1.log"),
    );

    // main.rs's order: the logger first, the destination once the command line is known
    log_panics::init();
    let mut app = App::new();
    app.add_plugins(LogPlugin {
        level: Level::INFO,
        filter: "wgpu=error".into(),
        custom_layer: |_| Some(Box::new(PerfLogLayer::new(&LOG))),
    });
    info!("logged before the destination is known");
    LOG.open(FileSink::create(&log).unwrap());

    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(FRAME))
        .init_resource::<AutopilotPhase>()
        .add_plugins(CsvPlugin {
            destination: CsvDestination::file(csv.clone()),
        })
        .add_systems(Update, drive);
    for _ in 0..ENDED + 5 {
        app.update();
    }

    let csv_bytes = std::fs::read(&csv).unwrap();
    assert!(
        !csv_bytes.starts_with(b"\xEF\xBB\xBF"),
        "no byte-order mark"
    );
    let csv_text = String::from_utf8(csv_bytes).unwrap();
    let mut lines = csv_text.split("\r\n");
    assert_eq!(lines.next(), Some("\"Frame\",\"CPU Time\",\"GPU Time\""));
    let rows: Vec<Vec<&str>> = lines
        .filter(|line| !line.is_empty())
        .map(|line| line.split(',').collect())
        .collect();
    assert_eq!(
        rows.len() as u32,
        ENDED - ARRIVED,
        "a row per Measuring frame"
    );
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.len(), 3);
        assert_eq!(row[0].parse::<u32>().unwrap(), ARRIVED + i as u32);
        assert_eq!(row[2], "0", "no GPU measurement registered");
    }
    assert!(rows[1..].iter().all(|row| row[1] == "16.5"), "{rows:?}");
    assert!(
        !csv_text.contains('\n')
            || csv_text.matches('\n').count() == csv_text.matches("\r\n").count()
    );

    let log_text = std::fs::read_to_string(&log).unwrap();
    assert!(
        log_text.contains("]</color>: logged before the destination is known\n"),
        "{log_text}"
    );
    let exceptions = counted_exceptions(&log_text);
    assert_eq!(
        exceptions,
        BTreeMap::from([
            ("[panic] PanicException".to_owned(), 1),
            ("[seeded] Error".to_owned(), 2),
        ]),
        "the seeded errors and panic count once each, the teardown error not at all:\n{log_text}"
    );
    assert!(log_text.contains(&format!("[autopilot]</color>: {EXIT_MARKER} at ")));
    assert_eq!(
        log_text.matches(EXIT_MARKER).count(),
        1,
        "only the run's own exit line carries the marker"
    );
    assert!(log_text.contains("]</color>: seeded warning\n"));

    if !keep {
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// `scripts/ProduceSummary.cs` `ParseExceptions`, ported: an entry line with an empty message
/// names the category, a `Type: message` line (type ending in `Exception` or `Error`) names the
/// type, a line holding the marker counts it; counting stops at the exit marker.
fn counted_exceptions(log: &str) -> BTreeMap<String, u32> {
    let mut kinds = BTreeMap::new();
    let (mut category, mut kind) = (None::<String>, None::<String>);
    for line in log.lines() {
        if line.contains(EXIT_MARKER) {
            break;
        }
        if let Some((cat, message)) = entry(line) {
            category = message.is_empty().then(|| cat.to_owned());
            kind = None;
        } else if kind.is_none() && type_name(line).is_some() {
            kind = type_name(line).map(str::to_owned);
        } else if let Some(kind) =
            kind.take_if(|_| line.contains("DebugLogReportHandler:LogExceptionInternal"))
        {
            let key = format!(
                "[{}] {kind}",
                category.take().unwrap_or_else(|| "UNKNOWN".into())
            );
            *kinds.entry(key).or_insert(0) += 1;
        }
    }
    kinds
}

/// `HH:MM:SS.mmm - <color=#RRGGBB>[CATEGORY]</color>: message`
fn entry(line: &str) -> Option<(&str, &str)> {
    let (clock, rest) = line.split_at_checked(12)?;
    let clock_ok = clock.bytes().enumerate().all(|(i, c)| match i {
        2 | 5 => c == b':',
        8 => c == b'.',
        _ => c.is_ascii_digit(),
    });
    let rest = rest.strip_prefix(" - <color=#")?;
    let (hex, rest) = rest.split_at_checked(6)?;
    let (category, message) = rest.strip_prefix(">[")?.split_once("]</color>: ")?;
    (clock_ok && hex.bytes().all(|c| c.is_ascii_hexdigit())).then_some((category, message))
}

/// `Namespace.TypeException: ` at the start of a line
fn type_name(line: &str) -> Option<&str> {
    let (head, _) = line.split_once(": ")?;
    let ok = !head.is_empty()
        && head
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '+' | '`'))
        && (head.ends_with("Exception") || head.ends_with("Error"));
    ok.then(|| head.rsplit(['.', '+']).next().unwrap())
}
