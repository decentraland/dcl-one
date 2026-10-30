//! A desktop crash, reported on the next run: a run creates its crash marker with its log and
//! removes it on a graceful exit, so one left behind names a log to report as an Explorer Error,
//! under the session the marker holds.

use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};

use analytics::{
    data_definition::{SegmentEvent, SegmentEventCommonExplorerFields, SegmentEventExplorerError},
    segment_system::{SegmentConfig, SegmentMetricsEvents},
};
use bevy::prelude::*;
use uuid::Uuid;

pub struct CrashReportPlugin {
    pub file: PathBuf,
}

impl Plugin for CrashReportPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CrashReport(self.file.clone()));
        app.add_systems(OnEnter::<ui_core::State>(ui_core::State::Ready), setup);
    }
}

#[derive(Resource)]
pub struct CrashReport(PathBuf);

/// A run's crash marker, beside its log; it holds the run's analytics session id.
pub fn marker_of(log: &Path) -> PathBuf {
    let mut marker = log.as_os_str().to_owned();
    marker.push(".touch");
    PathBuf::from(marker)
}

/// The log of an earlier run that left its marker behind; this run's own marker is not a crash.
pub fn crashed_log(log_dir: &Path, own_log: &Path) -> Option<PathBuf> {
    let own_marker = marker_of(own_log);
    std::fs::read_dir(log_dir)
        .ok()?
        .filter_map(|f| f.ok())
        .map(|f| f.path())
        .find(|path| path.extension().is_some_and(|ext| ext == "touch") && *path != own_marker)
        .map(|marker| marker.with_extension(""))
}

/// The session id a crashed run's marker holds, if it holds one (main.rs writes a uuid). The
/// marker is consumed either way: one whose log cannot be read must not be found again on every
/// run.
fn take_marker(log: &Path) -> Option<String> {
    let marker = marker_of(log);
    let session_id = std::fs::read_to_string(&marker)
        .ok()
        .map(|id| id.trim().to_owned())
        .filter(|id| Uuid::parse_str(id).is_ok());
    if let Err(e) = std::fs::remove_file(&marker) {
        if e.kind() != ErrorKind::NotFound {
            warn!(
                "failed to remove the crash marker {}: {e}",
                marker.display()
            );
        }
    }
    session_id
}

/// The report for a crashed run's log: its tail as the error, under the run's own session when
/// its marker says which, else `common`'s.
fn crash_report(
    log: &Path,
    common: &SegmentEventCommonExplorerFields,
) -> Option<(SegmentEventExplorerError, SegmentEventCommonExplorerFields)> {
    let session_id = take_marker(log);
    let buf = match std::fs::read(log) {
        Ok(buf) => buf,
        Err(e) => {
            warn!("failed to read log for crash report: {e}");
            return None;
        }
    };
    let start = buf.len().saturating_sub(31000);
    let Ok(error_message) = std::str::from_utf8(&buf[start..]) else {
        warn!("failed to convert crash log to utf8");
        return None;
    };
    let mut common = common.clone();
    if let Some(session_id) = session_id {
        common.session_id = session_id;
    }
    Some((
        SegmentEventExplorerError {
            error_type: "Crash".to_owned(),
            error_message: error_message.to_owned(),
            error_stack: String::default(),
        },
        common,
    ))
}

fn setup(
    report: Res<CrashReport>,
    mut metrics: ResMut<SegmentMetricsEvents>,
    config: Res<SegmentConfig>,
) {
    let Some((error, common)) = crash_report(&report.0, config.common()) else {
        return;
    };
    metrics.add_event(SegmentEvent::ExplorerError(error), &common);
    info!("crash report sent");
}

#[cfg(test)]
mod tests {
    use super::*;

    // Run A crashed, its marker still there with its session; this run B has its own marker and
    // no crash: A's log is reported under A's session and A's marker consumed, while B's marker is
    // never taken for a crash. A marker that holds no session id, or whose log cannot be read, is
    // consumed too, so it is not found again on every run.
    #[test]
    fn an_earlier_crash_is_reported_under_its_own_session_and_its_marker_consumed() {
        let dir = std::env::temp_dir().join(format!("crash_report_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.log");
        let session_a = Uuid::new_v4().to_string();
        std::fs::write(&a, "a panicked").unwrap();
        std::fs::write(marker_of(&a), format!("{session_a}\n")).unwrap();
        let b = dir.join("b.log");
        std::fs::write(&b, "").unwrap();
        std::fs::write(marker_of(&b), "").unwrap();

        assert_eq!(crashed_log(&dir, &b), Some(a.clone()));
        let common = SegmentEventCommonExplorerFields::new("session-b".into(), "1.0".into(), None);
        let (error, under) = crash_report(&a, &common).unwrap();
        assert_eq!(under.session_id, session_a);
        assert_eq!(error.error_type, "Crash");
        assert_eq!(error.error_message, "a panicked");
        assert_eq!(crashed_log(&dir, &b), None);

        let c = dir.join("c.log");
        std::fs::write(&c, "c panicked").unwrap();
        std::fs::write(marker_of(&c), "not a session").unwrap();
        let (_, under) = crash_report(&c, &common).unwrap();
        assert_eq!(under.session_id, "session-b");
        assert_eq!(crashed_log(&dir, &b), None);

        let gone = dir.join("gone.log");
        std::fs::write(marker_of(&gone), "").unwrap();
        assert!(crash_report(&gone, &common).is_none());
        assert_eq!(crashed_log(&dir, &b), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
