//! The per-frame CSV of the benchmark run ([`crate::format::csv_row`]).
//!
//! The writer follows [`AutopilotPhase`], which run control moves:
//! - `Arriving`: nothing is written and no destination exists yet;
//! - `Measuring`: on the first such frame the destination is created and the header written;
//!   every frame, this one included, then adds one row;
//! - `Ending`: in the first such frame the destination is flushed and closed, with no row for
//!   that frame; nothing is written afterwards.
//!
//! A destination that cannot be created, written or flushed is closed and [`CsvFailed`] inserted,
//! so run control ends the run with a non-zero code instead of measuring into nothing.
//!
//! CPU is `Time<Real>`'s delta (on the web, the `web-time` clock), and `Time<Real>` advances to the
//! instant the render world posts when its frame ends, which the main world reads two frames
//! late. So row N holds the interval between two render completions, two to three frames before
//! frame N: a render-completion interval, present and vsync waits included, not frame N's own
//! time. Sums and percentiles are exact (the average is the run's length over its rows); the rows
//! are offset, so a hitch in the main world shows up two rows after its frame.
//! GPU is the latest [`GPU_FRAME_TIME`] measurement, or 0 while nothing registers or measures it
//! (an adapter without timestamp queries never does). When it is measured and the GPU finishes
//! each frame before the next one is submitted, row N holds frame N - 3's GPU time, which trails
//! CPU's interval by up to a frame; when the GPU falls behind, it holds an older frame's.

use std::{
    io::{self, Write},
    sync::Arc,
    time::Duration,
};

use bevy::{
    diagnostic::{update_frame_count, Diagnostic, DiagnosticsStore, FrameCount},
    prelude::*,
};
use common::{
    autopilot::{AutopilotPhase, CsvFailed},
    gpu_time::GPU_FRAME_TIME,
};

use crate::format::{csv_row, milliseconds_to_nanoseconds, CSV_HEADER};

/// How often the rows reach the destination while measuring, so a run that is killed keeps all
/// but the last second.
const FLUSH_EVERY: Duration = Duration::from_secs(1);

/// Where the rows are written. A destination that is slow to write buffers itself (the file does);
/// the rows are handed to it as they come, and it is flushed once a second and at `Ending`.
pub type Destination = Box<dyn Write + Send + Sync>;

/// Where the rows go: how to open the destination, which happens when `Measuring` begins.
#[derive(Clone)]
pub struct CsvDestination(Arc<dyn Fn() -> io::Result<Destination> + Send + Sync>);

impl CsvDestination {
    /// Any writer.
    pub fn new(open: impl Fn() -> io::Result<Destination> + Send + Sync + 'static) -> Self {
        Self(Arc::new(open))
    }

    /// Desktop: the file at `--csv`, buffered.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn file(path: std::path::PathBuf) -> Self {
        Self::new(move || Ok(Box::new(io::BufWriter::new(std::fs::File::create(&path)?))))
    }

    /// Web: the page's `csv` stream (`perfStream` with `perfCsv`), unbuffered: queueing a row is
    /// cheap, and a row held in a buffer would be lost with the engine when it traps, where the
    /// queue's rows can still be taken (`stream::try_drain`).
    #[cfg(target_arch = "wasm32")]
    pub fn stream() -> Self {
        Self::new(|| {
            crate::stream::csv_writer()
                .map(|writer| Box::new(writer) as Destination)
                .ok_or_else(|| io::Error::other("the page's CSV stream was not started"))
        })
    }

    fn create(&self) -> io::Result<Destination> {
        (self.0)()
    }
}

pub struct CsvPlugin {
    pub destination: CsvDestination,
}

impl Plugin for CsvPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CsvOutput {
            destination: self.destination.clone(),
            state: CsvState::Waiting,
            row: String::new(),
        })
        // before the count moves on, so a row carries the frame's own number
        .add_systems(
            Last,
            write_csv
                .before(update_frame_count)
                .run_if(resource_exists::<AutopilotPhase>),
        );
    }
}

#[derive(Resource)]
struct CsvOutput {
    destination: CsvDestination,
    state: CsvState,
    /// one row's bytes, kept to avoid an allocation per frame
    row: String,
}

enum CsvState {
    /// No `Measuring` frame yet.
    Waiting,
    Open {
        writer: Destination,
        flushed_at: Duration,
    },
    /// Ended, or the destination failed: nothing more is written.
    Closed,
}

fn write_csv(
    mut commands: Commands,
    mut output: ResMut<CsvOutput>,
    phase: Res<AutopilotPhase>,
    frame: Res<FrameCount>,
    time: Res<Time<Real>>,
    diagnostics: Option<Res<DiagnosticsStore>>,
) {
    let output = &mut *output;
    let failure = match (&mut output.state, *phase) {
        (CsvState::Waiting, AutopilotPhase::Measuring) => {
            match output.destination.create().and_then(|mut writer| {
                writer.write_all(CSV_HEADER.as_bytes())?;
                Ok(writer)
            }) {
                Ok(writer) => {
                    output.state = CsvState::Open {
                        writer,
                        flushed_at: time.elapsed(),
                    };
                    write_row(output, &frame, &time, diagnostics.as_deref())
                        .err()
                        .map(|e| format!("can't write the benchmark CSV: {e}"))
                }
                Err(e) => Some(format!("can't create the benchmark CSV: {e}")),
            }
        }
        (CsvState::Open { .. }, AutopilotPhase::Measuring) => {
            write_row(output, &frame, &time, diagnostics.as_deref())
                .err()
                .map(|e| format!("can't write the benchmark CSV: {e}"))
        }
        (CsvState::Open { writer, .. }, AutopilotPhase::Ending { .. }) => {
            let flushed = writer.flush();
            output.state = CsvState::Closed;
            flushed
                .err()
                .map(|e| format!("can't finish the benchmark CSV: {e}"))
        }
        (CsvState::Waiting, AutopilotPhase::Ending { .. }) => {
            output.state = CsvState::Closed;
            None
        }
        _ => None,
    };
    if let Some(failure) = failure {
        error!("{failure}");
        output.state = CsvState::Closed;
        commands.insert_resource(CsvFailed);
    }
}

/// Adds the frame's row to an open destination, and flushes it once a second.
fn write_row(
    output: &mut CsvOutput,
    frame: &FrameCount,
    time: &Time<Real>,
    diagnostics: Option<&DiagnosticsStore>,
) -> io::Result<()> {
    let CsvState::Open { writer, flushed_at } = &mut output.state else {
        return Ok(());
    };
    let cpu = u64::try_from(time.delta().as_nanos()).unwrap_or(u64::MAX);
    let gpu = diagnostics
        .and_then(|store| store.get(&GPU_FRAME_TIME))
        .and_then(Diagnostic::value)
        .map_or(0, milliseconds_to_nanoseconds);
    output.row.clear();
    csv_row(&mut output.row, frame.0, cpu, gpu);
    writer.write_all(output.row.as_bytes())?;
    if time.elapsed().saturating_sub(*flushed_at) >= FLUSH_EVERY {
        *flushed_at = time.elapsed();
        writer.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use bevy::{diagnostic::RegisterDiagnostic, time::TimeUpdateStrategy};

    use super::*;

    /// A writer into memory.
    struct Memory(Arc<Mutex<Vec<u8>>>);

    impl Write for Memory {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// A destination that takes its first `writes` writes and fails every later one, and fails
    /// every flush when `flush_fails`: a full disk or a file another process locked.
    struct Failing {
        writes: usize,
        flush_fails: bool,
    }

    impl Write for Failing {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.writes == 0 {
                return Err(io::Error::other("the disk is full"));
            }
            self.writes -= 1;
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            if self.flush_fails {
                Err(io::Error::other("the disk is full"))
            } else {
                Ok(())
            }
        }
    }

    fn app_with(destination: CsvDestination) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::diagnostic::DiagnosticsPlugin))
            .init_resource::<AutopilotPhase>()
            // every update is 16.5 ms of real time
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_micros(
                16_500,
            )))
            .add_plugins(CsvPlugin { destination });
        app
    }

    fn app() -> (App, Arc<Mutex<Vec<u8>>>) {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let app = app_with(CsvDestination::new({
            let bytes = bytes.clone();
            move || Ok(Box::new(Memory(bytes.clone())))
        }));
        (app, bytes)
    }

    fn text(bytes: &Arc<Mutex<Vec<u8>>>) -> String {
        String::from_utf8(bytes.lock().unwrap().clone()).unwrap()
    }

    fn failed(app: &App) -> bool {
        app.world().contains_resource::<CsvFailed>()
    }

    fn set_phase(app: &mut App, phase: AutopilotPhase) {
        *app.world_mut().resource_mut::<AutopilotPhase>() = phase;
    }

    #[test]
    fn nothing_is_created_until_measuring_and_nothing_after_ending() {
        let (mut app, bytes) = app();
        app.update();
        app.update();
        assert_eq!(text(&bytes), "", "Arriving writes nothing");

        *app.world_mut().resource_mut::<AutopilotPhase>() = AutopilotPhase::Measuring;
        app.update();
        app.update();
        // the first update's delta is 0: the clock starts then
        *app.world_mut().resource_mut::<AutopilotPhase>() = AutopilotPhase::Ending { exit_code: 0 };
        app.update();
        app.update();

        let text = text(&bytes);
        let mut lines = text.split("\r\n");
        assert_eq!(lines.next(), Some("\"Frame\",\"CPU Time\",\"GPU Time\""));
        let rows: Vec<_> = lines.collect();
        // two Measuring frames, a row each; the Ending frame adds none; the text ends in CRLF
        assert_eq!(rows.len(), 3, "{rows:?}");
        assert_eq!(rows[2], "");
        let frames: Vec<u32> = rows[..2]
            .iter()
            .map(|row| row.split(',').next().unwrap().parse().unwrap())
            .collect();
        assert_eq!(frames[1], frames[0] + 1, "Frame is Bevy's FrameCount");
        assert!(
            rows[0].ends_with(",0"),
            "GPU is 0 without a measurement: {rows:?}"
        );
        assert_eq!(rows[1], format!("{},16.5,0", frames[1]));
    }

    #[test]
    fn a_run_that_never_arrives_leaves_no_csv() {
        let (mut app, bytes) = app();
        app.update();
        *app.world_mut().resource_mut::<AutopilotPhase>() = AutopilotPhase::Ending { exit_code: 1 };
        app.update();
        // a late Measuring does not reopen it
        *app.world_mut().resource_mut::<AutopilotPhase>() = AutopilotPhase::Measuring;
        app.update();
        assert_eq!(text(&bytes), "");
    }

    #[test]
    fn gpu_is_the_latest_gpu_frame_time_measurement() {
        let (mut app, bytes) = app();
        app.register_diagnostic(Diagnostic::new(GPU_FRAME_TIME).with_suffix("ms"));
        *app.world_mut().resource_mut::<AutopilotPhase>() = AutopilotPhase::Measuring;
        app.update();
        // two measurements in one frame: the row carries the later one, not a smoothed value
        for value in [4.0, 7.5] {
            app.world_mut()
                .resource_mut::<DiagnosticsStore>()
                .get_mut(&GPU_FRAME_TIME)
                .unwrap()
                .add_measurement(bevy::diagnostic::DiagnosticMeasurement {
                    time: bevy::platform::time::Instant::now(),
                    value,
                });
        }
        app.update();
        *app.world_mut().resource_mut::<AutopilotPhase>() = AutopilotPhase::Ending { exit_code: 0 };
        app.update();
        let text = text(&bytes);
        let rows: Vec<_> = text.lines().skip(1).collect();
        assert!(rows[0].ends_with(",0"), "no measurement yet: {rows:?}");
        assert!(rows[1].ends_with(",7.5"), "{rows:?}");
    }

    #[test]
    fn a_csv_written_whole_does_not_fail_the_run() {
        let (mut app, _) = app();
        set_phase(&mut app, AutopilotPhase::Measuring);
        app.update();
        app.update();
        set_phase(&mut app, AutopilotPhase::Ending { exit_code: 0 });
        app.update();
        assert!(!failed(&app));
    }

    #[test]
    fn a_csv_that_cannot_be_created_fails_the_run() {
        let mut app = app_with(CsvDestination::new(|| {
            Err(io::Error::other("the folder does not exist"))
        }));
        app.update();
        assert!(!failed(&app), "nothing is created while arriving");
        set_phase(&mut app, AutopilotPhase::Measuring);
        app.update();
        assert!(failed(&app));
    }

    #[test]
    fn a_row_that_cannot_be_written_fails_the_run() {
        // the header goes in, the first row does not
        let mut app = app_with(CsvDestination::new(|| {
            Ok(Box::new(Failing {
                writes: 1,
                flush_fails: false,
            }))
        }));
        set_phase(&mut app, AutopilotPhase::Measuring);
        app.update();
        assert!(failed(&app));
    }

    #[test]
    fn a_csv_that_cannot_be_finished_fails_the_run() {
        let mut app = app_with(CsvDestination::new(|| {
            Ok(Box::new(Failing {
                writes: usize::MAX,
                flush_fails: true,
            }))
        }));
        set_phase(&mut app, AutopilotPhase::Measuring);
        app.update();
        app.update();
        // under a second of rows: nothing was flushed yet
        assert!(!failed(&app));
        set_phase(&mut app, AutopilotPhase::Ending { exit_code: 0 });
        app.update();
        assert!(failed(&app));
    }
}
