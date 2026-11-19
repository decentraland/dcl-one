//! Scenarios about the explorer's uplink to Pulse over a slow, lossy or buffering path. Whether its
//! movement keeps flowing is read from the datagrams it sends, as the link in front of it sees
//! them before any fault acts, not from what the explorer says. Its own readout of its uplink
//! (`/pulse_state`, `uplink`) is the second signal, sampled while it walks.
//!
//! The link tells datagrams apart by size alone. While the explorer walks, nearly all it sends with
//! a message in it is movement; the transport's own datagrams (pings, acknowledgements) are
//! smaller. So the gaps between datagrams of at least [`DATA_BYTES`] show when movement stopped
//! going out. They cannot tell movement from another message of the same size (a teleport, a
//! resync request), nor a gap the explorer made for another reason (a long frame) from a gap its
//! uplink gate made: the readout is what says which.

use std::time::Duration;

use anyhow::{Result, bail};
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::engine::{Ctx, Scenario, expect_or};
use crate::fault::{Direction, Fate, Fault, Seen, Silences, cadence};
use crate::scenarios::explorer::{NEEDS, Subject, last_log_lines, observer, start};

/// The scenarios of this group, in the order a run takes them.
pub fn all() -> Vec<&'static Scenario> {
    vec![
        &UPLINK_CADENCE,
        &WALK_AT_800_MS,
        &WALK_AT_1200_MS,
        &WALK_ON_A_LOSSY_LINK,
        &QUEUED_STALL_AT_1200_MS,
        &QUEUED_STALL_AT_2000_MS,
    ]
}

static UPLINK_CADENCE: Scenario = Scenario {
    name: "explorer-uplink-cadence",
    tags: &["explorer", "uplink"],
    guards: &["dcl-one #1208"],
    summary: "On a plain link the explorer walks, then stands still: the datagrams it sends tell \
              the two apart, walking at its movement beat with no silence, standing at its \
              keepalive. The check of the measure the other uplink scenarios rely on.",
    needs: NEEDS,
    hostile: false,
    default_repeat: 1,
    run: |ctx| Box::pin(uplink_cadence(ctx)),
};

static WALK_AT_800_MS: Scenario = Scenario {
    name: "explorer-walk-800ms-round-trip",
    tags: &["explorer", "uplink", "faults"],
    guards: &["dcl-one #1208"],
    summary: "The explorer walks over a healthy link with an 800 ms round trip: its movement keeps \
              going out at its beat, with no recurring silence.",
    needs: NEEDS,
    hostile: false,
    default_repeat: 1,
    run: |ctx| Box::pin(walk_at_round_trip(ctx, Duration::from_millis(800))),
};

static WALK_AT_1200_MS: Scenario = Scenario {
    name: "explorer-walk-1200ms-round-trip",
    tags: &["explorer", "uplink", "faults"],
    guards: &["dcl-one #1208"],
    summary: "The explorer walks over a healthy link with a 1.2 s round trip: its movement keeps \
              going out at its beat, with no recurring silence.",
    needs: NEEDS,
    hostile: false,
    default_repeat: 1,
    run: |ctx| Box::pin(walk_at_round_trip(ctx, Duration::from_millis(1200))),
};

static WALK_ON_A_LOSSY_LINK: Scenario = Scenario {
    name: "explorer-walk-lossy-link",
    tags: &["explorer", "uplink", "faults"],
    guards: &["dcl-one #1208"],
    summary: "The explorer walks for a minute over a link with a round trip of 300 ms give or \
              take 100 and 2 % loss each way: a single lost datagram does not stop its movement. \
              At most one silence in the minute, none longer than 250 ms beyond its beat.",
    needs: NEEDS,
    hostile: false,
    default_repeat: 1,
    run: |ctx| Box::pin(walk_on_a_lossy_link(ctx)),
};

static QUEUED_STALL_AT_1200_MS: Scenario = Scenario {
    name: "explorer-queued-stall-1200ms-round-trip",
    tags: &["explorer", "uplink", "faults"],
    guards: &["dcl-one #1208"],
    summary: "Over a link with a 1.2 s round trip the uplink stalls for 4 s while the explorer \
              walks, and what it sent meanwhile reaches Pulse at once when the stall ends: the \
              session survives the burst and the observer sees the explorer walk on.",
    needs: NEEDS,
    hostile: false,
    default_repeat: 1,
    run: |ctx| Box::pin(queued_stall(ctx, Duration::from_millis(1200))),
};

static QUEUED_STALL_AT_2000_MS: Scenario = Scenario {
    name: "explorer-queued-stall-2000ms-round-trip",
    tags: &["explorer", "uplink", "faults"],
    guards: &["dcl-one #1208"],
    summary: "Over a link with a 2 s round trip the uplink stalls for 4 s while the explorer \
              walks, and what it sent meanwhile reaches Pulse at once when the stall ends: the \
              session survives the burst and the observer sees the explorer walk on.",
    needs: NEEDS,
    hostile: false,
    default_repeat: 1,
    run: |ctx| Box::pin(queued_stall(ctx, Duration::from_millis(2000))),
};

/// Uplink datagrams of at least this many bytes carry a message. On a plain link
/// (`explorer-uplink-cadence`) the transport's own datagrams (a ping, acknowledgements) are 8 to
/// 16 bytes, and a movement message 23 to 40.
const DATA_BYTES: usize = 20;
/// A gap between two data datagrams is a silence only past one and a half beats of the walk and
/// past this. Movement goes out at most every 100 ms (`movement_pacing`, 10 a second), on the
/// next frame after: about 133 ms at the headless explorer's frame rate, measured on a plain link.
const SILENCE_FLOOR: Duration = Duration::from_millis(180);
/// What each walk leg leaves out at its start and its end, where the explorer starts and stops.
const LEG_TRIM: Duration = Duration::from_millis(500);
/// How far each walk leg goes, along x, back and forth around where the explorer started.
const LEG_METRES: f32 = 16.0;
/// How long each walk leg takes. The explorer is moved along the leg over this time
/// (`/move_player_to` with a duration), which it sends as movement, not as a teleport.
const LEG_SECONDS: f32 = 4.0;
/// How long a walk on the plain link lasts before a fault is set: the measure's own control.
const BASELINE: Duration = Duration::from_secs(10);
/// How long a new path is walked on before it is measured, for the transports' estimates of the
/// round trip to reach it.
const SETTLE: Duration = Duration::from_secs(8);
/// How long the walk is measured over a slow path.
const MEASURE: Duration = Duration::from_secs(30);
/// How long the walk is measured over the lossy path.
const MEASURE_LOSSY: Duration = Duration::from_secs(60);
/// Movement a walk must send on the plain link, a second: under its beat of 10, over its
/// keepalive of 1.
const WALKING_RATE: f64 = 6.0;
/// Data datagrams a second the explorer may send standing still: its keepalive of 1, and room.
const STANDING_RATE: f64 = 2.5;
/// Silences a measured walk over a healthy slow path may have: a long frame on a busy machine
/// makes one now and then, a gate that stops movement every round trip makes dozens.
const SILENCES_ALLOWED: usize = 2;
/// The share of a measured walk its silences may last beyond the beat.
const SILENT_SHARE_ALLOWED: f64 = 0.02;
/// How long beyond the beat one silence on the lossy link may last: a hold that a single lost
/// datagram starts lasts until the transport resends, about a round trip and more past the
/// 600 ms the explorer waits for an acknowledgement; two losses in a row of a probe sent every
/// 250 ms hold for less than this.
const LOSSY_SILENCE_ALLOWED: Duration = Duration::from_millis(250);
/// Silences the minute on the lossy link may have: a gate that a single loss trips stops movement
/// once for every few dozen probes, several times a minute at this loss; two losses in a row, or a
/// long frame, make one now and then.
const LOSSY_SILENCES_ALLOWED: usize = 1;
/// How long before a silence a lost datagram counts as what may have started it: the 600 ms the
/// gate waits, and a round trip.
const LOSS_LOOKBACK: Duration = Duration::from_millis(1200);
/// How long the uplink stalls in the queued-stall scenarios: past the round trip and the 600 ms
/// the gate waits at 2 s, so movement sent before the explorer could notice is all queued.
const STALL: Duration = Duration::from_secs(4);
/// What the queue of a stall keeps at most: far more than a few seconds of the explorer.
const STALL_QUEUE: usize = 4096;
/// The input burst Pulse allows before it ends a session (`DEFAULT_INPUT_BURST`).
const SERVER_INPUT_BURST: usize = 16;

fn ms(duration: Duration) -> u64 {
    duration.as_millis() as u64
}

fn seconds(duration: Duration) -> f64 {
    (duration.as_secs_f64() * 10.0).round() / 10.0
}

/// One answer of `/pulse_state`, at the middle of the call on the Pulse link's clock.
#[derive(Debug, Clone, Serialize)]
struct Sample {
    at: Duration,
    state: String,
    stalled: Option<bool>,
    held_input: Option<bool>,
    probes: Option<u64>,
}

/// What the explorer did while a script ran: its walk legs and its readouts.
#[derive(Default)]
struct Watched {
    /// Each leg's start and end on the Pulse link's clock.
    legs: Mutex<Vec<(Duration, Duration)>>,
    samples: Mutex<Vec<Sample>>,
    /// The last `uplink` object the explorer reported, whatever fields it has.
    uplink: Mutex<Value>,
    /// Where the walk failed, if it did.
    failed: Mutex<Option<String>>,
}

/// Walks the explorer back and forth between `ends` until `stop` says so.
async fn walk(
    subject: &Subject,
    ends: [[f32; 3]; 2],
    mut stop: watch::Receiver<bool>,
    watched: &Watched,
) {
    let link = &subject.fronts.pulse;
    let mut leg = 0usize;
    while !*stop.borrow() {
        let target = ends[(leg + 1) % 2];
        let line = format!(
            "/move_player_to {} {} {} {LEG_SECONDS}",
            target[0], target[1], target[2]
        );
        let begun = link.clock();
        let answer = tokio::select! {
            answer = subject.explorer.command(&line, Duration::from_secs(15)) => Some(answer),
            _ = stop.changed() => None,
        };
        watched.legs.lock().push((begun, link.clock()));
        match answer {
            None => return,
            Some(Ok(answer)) if answer.ok => {}
            Some(Ok(answer)) => {
                *watched.failed.lock() =
                    Some(format!("the explorer refused `{line}`: {}", answer.output));
                return;
            }
            Some(Err(error)) => {
                *watched.failed.lock() = Some(format!("`{line}` failed: {error:#}"));
                return;
            }
        }
        leg += 1;
    }
}

/// Reads `/pulse_state` over and over until `stop` says so.
async fn sample(subject: &Subject, mut stop: watch::Receiver<bool>, watched: &Watched) {
    let link = &subject.fronts.pulse;
    while !*stop.borrow() {
        let asked = link.clock();
        let polled = tokio::select! {
            polled = subject.explorer.poll("/pulse_state") => polled,
            _ = stop.changed() => return,
        };
        let answered = link.clock();
        let Ok(state) = polled else {
            continue;
        };
        let uplink = &state["uplink"];
        watched.samples.lock().push(Sample {
            at: (asked + answered) / 2,
            state: state["state"].as_str().unwrap_or("unknown").to_owned(),
            stalled: uplink["stalled"].as_bool(),
            held_input: uplink["held"][2].as_bool(),
            probes: uplink["probes"].as_u64(),
        });
        if uplink.is_object() {
            *watched.uplink.lock() = uplink.clone();
        }
    }
}

/// Runs `script` while the explorer walks between `ends` (as long as `walking` says) and its
/// readout is sampled; both stop when the script is over.
async fn watch_while<T>(
    subject: &Subject,
    ends: [[f32; 3]; 2],
    watched: &Watched,
    script: impl AsyncFnOnce(&watch::Sender<bool>) -> T,
) -> T {
    let (walking, walk_stop) = watch::channel(false);
    let (sampling, sample_stop) = watch::channel(false);
    let script = async {
        let out = script(&walking).await;
        let _ = walking.send(true);
        let _ = sampling.send(true);
        out
    };
    let ((), (), out) = tokio::join!(
        walk(subject, ends, walk_stop, watched),
        sample(subject, sample_stop, watched),
        script
    );
    out
}

/// The two ends of the walk around where the explorer started.
fn leg_ends(home: [f32; 3]) -> [[f32; 3]; 2] {
    [
        [home[0] - LEG_METRES / 2.0, home[1], home[2]],
        [home[0] + LEG_METRES / 2.0, home[1], home[2]],
    ]
}

/// The parts of the legs, trimmed at both ends, inside `from..to`.
fn spans(legs: &[(Duration, Duration)], from: Duration, to: Duration) -> Vec<(Duration, Duration)> {
    legs.iter()
        .filter_map(|&(begun, ended)| {
            let start = (begun + LEG_TRIM).max(from);
            let end = ended.saturating_sub(LEG_TRIM).min(to);
            (end > start).then_some((start, end))
        })
        .collect()
}

/// What the link and the readout showed over some spans of a walk.
#[derive(Debug, Clone, Serialize)]
struct Reading {
    walked_s: f64,
    data_per_second: f64,
    median_gap_ms: u64,
    longest_gap_ms: u64,
    silence_over_ms: u64,
    silences: usize,
    /// What the silences last beyond the beat, as a share of the walk.
    silent_share: f64,
    /// What every gap lasts beyond the beat, as a share of the walk.
    beyond_beat_share: f64,
    longest_silence_ms: u64,
    silence_period_ms: Option<u64>,
    /// The first silences: when (s on the link's clock) and how long the gap (ms).
    first_silences: Vec<(f64, u64)>,
    samples: usize,
    /// The share of readouts with an input held, among those that say.
    held_input_share: Option<f64>,
    stalled_share: Option<f64>,
    /// Silences with a readout inside them that says the gate held or saw a stall.
    silences_seen_held: usize,
    /// Readouts with an input held that fall in no silence.
    held_outside_silences: usize,
    probes_per_second: Option<f64>,
}

fn share<F: Fn(&Sample) -> Option<bool>>(samples: &[&Sample], field: F) -> Option<f64> {
    let said: Vec<bool> = samples.iter().filter_map(|sample| field(sample)).collect();
    (!said.is_empty()).then(|| said.iter().filter(|held| **held).count() as f64 / said.len() as f64)
}

/// Reads the uplink and the readouts over the trimmed legs inside `from..to`.
fn read(
    up: &[Seen],
    legs: &[(Duration, Duration)],
    from: Duration,
    to: Duration,
    samples: &[Sample],
) -> (Reading, Silences) {
    let spans = spans(legs, from, to);
    let cadence = cadence(up, &spans, DATA_BYTES);
    let silences = cadence.silences(cadence.silence_threshold(SILENCE_FLOOR));
    let inside = |at: Duration| spans.iter().any(|&(start, end)| at >= start && at < end);
    let walked: Vec<&Sample> = samples.iter().filter(|sample| inside(sample.at)).collect();
    let in_silence = |at: Duration| {
        silences
            .gaps
            .iter()
            .any(|gap| at >= gap.start && at <= gap.start + gap.length)
    };
    let silences_seen_held = silences
        .gaps
        .iter()
        .filter(|gap| {
            walked.iter().any(|sample| {
                sample.at >= gap.start
                    && sample.at <= gap.start + gap.length
                    && (sample.stalled == Some(true) || sample.held_input == Some(true))
            })
        })
        .count();
    let held_outside_silences = walked
        .iter()
        .filter(|sample| sample.held_input == Some(true) && !in_silence(sample.at))
        .count();
    let probed: Vec<(Duration, u64)> = samples
        .iter()
        .filter(|sample| sample.at >= from && sample.at < to)
        .filter_map(|sample| sample.probes.map(|probes| (sample.at, probes)))
        .collect();
    let probes_per_second = match (probed.first(), probed.last()) {
        (Some(first), Some(last)) if last.0 > first.0 => Some(
            ((last.1.saturating_sub(first.1)) as f64 / (last.0 - first.0).as_secs_f64() * 100.0)
                .round()
                / 100.0,
        ),
        _ => None,
    };
    let reading = Reading {
        walked_s: seconds(cadence.span),
        data_per_second: if cadence.span.is_zero() {
            0.0
        } else {
            (cadence.large as f64 / cadence.span.as_secs_f64() * 10.0).round() / 10.0
        },
        median_gap_ms: ms(cadence.median_gap),
        longest_gap_ms: ms(cadence.longest_gap),
        silence_over_ms: ms(silences.longer_than),
        silences: silences.gaps.len(),
        silent_share: (silences.share * 1000.0).round() / 1000.0,
        beyond_beat_share: (cadence.beyond_beat_share() * 1000.0).round() / 1000.0,
        longest_silence_ms: ms(silences.longest),
        silence_period_ms: silences.period.map(ms),
        first_silences: silences
            .gaps
            .iter()
            .take(12)
            .map(|gap| (seconds(gap.start), ms(gap.length)))
            .collect(),
        samples: walked.len(),
        held_input_share: share(&walked, |sample| sample.held_input),
        stalled_share: share(&walked, |sample| sample.stalled),
        silences_seen_held,
        held_outside_silences,
        probes_per_second,
    };
    (reading, silences)
}

/// Uplink datagram sizes from `from` to `to`, smallest first, with how many of each.
fn sizes(up: &[Seen], from: Duration, to: Duration) -> Vec<(usize, usize)> {
    let mut counts = std::collections::BTreeMap::new();
    for seen in up.iter().filter(|seen| seen.at >= from && seen.at < to) {
        *counts.entry(seen.bytes).or_insert(0usize) += 1;
    }
    counts.into_iter().collect()
}

/// Fails unless the plain-link walk shows the measure working: movement at its beat, no silence.
fn expect_baseline(baseline: &Reading) -> Result<()> {
    if baseline.data_per_second < WALKING_RATE {
        bail!(
            "on the plain link the walking explorer sent {} data datagrams a second, not the {WALKING_RATE} or more of a walk: the walk or the measure does not work ({baseline:?})",
            baseline.data_per_second
        );
    }
    if baseline.silences > SILENCES_ALLOWED {
        bail!(
            "on the plain link the walking explorer already fell silent {} times for over {} ms: this machine is too busy to measure a gate ({baseline:?})",
            baseline.silences,
            baseline.silence_over_ms
        );
    }
    Ok(())
}

async fn uplink_cadence(ctx: Ctx) -> Result<()> {
    let subject = start(&ctx, true).await?;
    let link = &subject.fronts.pulse;
    let watched = Watched::default();
    let ends = leg_ends(subject.position);
    let (walked_from, walked_to, stood_from, stood_to) =
        watch_while(&subject, ends, &watched, async |walking| {
            let from = link.clock();
            tokio::time::sleep(Duration::from_secs(15)).await;
            let to = link.clock();
            // stop walking, and let the last leg end
            let _ = walking.send(true);
            tokio::time::sleep(Duration::from_secs(2)).await;
            let stood = link.clock();
            tokio::time::sleep(Duration::from_secs(10)).await;
            (from, to, stood, link.clock())
        })
        .await;
    let up = link.history(Direction::Uplink);
    let legs = watched.legs.lock().clone();
    let samples = watched.samples.lock().clone();
    let (walking, _) = read(&up, &legs, walked_from, walked_to, &samples);
    let standing_data = up
        .iter()
        .filter(|seen| seen.at >= stood_from && seen.at < stood_to && seen.bytes >= DATA_BYTES)
        .count();
    let standing_rate = standing_data as f64 / (stood_to - stood_from).as_secs_f64();
    let position = subject.explorer.probe("/world_state").await.ok();
    ctx.note("walk legs", legs.len());
    ctx.note("walking", &walking);
    ctx.note(
        "walking: uplink datagram sizes (bytes, count)",
        sizes(&up, walked_from, walked_to),
    );
    ctx.note(
        "standing: data datagrams a second",
        (standing_rate * 100.0).round() / 100.0,
    );
    ctx.note(
        "standing: uplink datagram sizes (bytes, count)",
        sizes(&up, stood_from, stood_to),
    );
    ctx.note(
        "the explorer's last uplink readout",
        watched.uplink.lock().clone(),
    );
    ctx.note(
        "where the explorer stands at the end",
        position.map(|state| state["position"].clone()),
    );
    if let Some(failed) = watched.failed.lock().clone() {
        bail!("the walk failed: {failed}");
    }
    expect_baseline(&walking)?;
    if standing_rate > STANDING_RATE {
        bail!(
            "standing still, the explorer sent {standing_rate:.2} data datagrams a second, more than the {STANDING_RATE} of its keepalive: the measure cannot tell walking from standing"
        );
    }
    subject.explorer.stop().await;
    Ok(())
}

/// A path with `round_trip` and nothing else wrong with it.
fn slow_path(round_trip: Duration) -> Fault {
    Fault::Link {
        delay: round_trip / 2,
        jitter: Duration::ZERO,
        loss: 0.0,
    }
}

async fn walk_at_round_trip(ctx: Ctx, round_trip: Duration) -> Result<()> {
    let subject = start(&ctx, true).await?;
    let link = &subject.fronts.pulse;
    let watched = Watched::default();
    let ends = leg_ends(subject.position);
    let (marks, delayed) = watch_while(&subject, ends, &watched, async |_| {
        let plain = link.clock();
        tokio::time::sleep(BASELINE).await;
        let slowed = link.clock();
        link.set(Direction::Both, slow_path(round_trip));
        tokio::time::sleep(SETTLE).await;
        let measured = link.clock();
        let before = link.counters().delayed;
        tokio::time::sleep(MEASURE).await;
        (
            [plain, slowed, measured, link.clock()],
            link.counters().delayed - before,
        )
    })
    .await;
    let up = link.history(Direction::Uplink);
    let legs = watched.legs.lock().clone();
    let samples = watched.samples.lock().clone();
    let (baseline, _) = read(&up, &legs, marks[0], marks[1], &samples);
    let (measured, silences) = read(&up, &legs, marks[2], marks[3], &samples);
    ctx.note("round trip set, ms", ms(round_trip));
    ctx.note("datagrams delayed while measured, both ways", delayed);
    ctx.note("plain link", &baseline);
    ctx.note("slow link", &measured);
    ctx.note(
        "the explorer's last uplink readout",
        watched.uplink.lock().clone(),
    );
    if let Some(failed) = watched.failed.lock().clone() {
        bail!("the walk failed: {failed}");
    }
    if delayed == 0 {
        bail!("the link delayed nothing while it was measured: the fault touched nothing");
    }
    expect_baseline(&baseline)?;
    if measured.silences > SILENCES_ALLOWED || silences.share > SILENT_SHARE_ALLOWED {
        bail!(
            "over a healthy link with a {} ms round trip the walking explorer's movement does not keep flowing: {} silences of over {} ms in {} s (at most {SILENCES_ALLOWED} allowed), together {:.1} % of the walk beyond the {} ms beat (at most {:.0} % allowed), the longest {} ms, one every {} ms; the explorer's readout showed an input held in {} of those silences",
            ms(round_trip),
            measured.silences,
            measured.silence_over_ms,
            measured.walked_s,
            silences.share * 100.0,
            measured.median_gap_ms,
            SILENT_SHARE_ALLOWED * 100.0,
            measured.longest_silence_ms,
            measured
                .silence_period_ms
                .map_or("-".to_owned(), |period| period.to_string()),
            measured.silences_seen_held,
        );
    }
    subject.explorer.stop().await;
    Ok(())
}

/// The datagrams lost in `from..to` of one direction: (smaller than data, data).
fn lost_between(seen: &[Seen], from: Duration, to: Duration) -> (usize, usize) {
    let lost: Vec<&Seen> = seen
        .iter()
        .filter(|seen| seen.fate == Fate::Lost && seen.at >= from && seen.at < to)
        .collect();
    let data = lost.iter().filter(|seen| seen.bytes >= DATA_BYTES).count();
    (lost.len() - data, data)
}

async fn walk_on_a_lossy_link(ctx: Ctx) -> Result<()> {
    let subject = start(&ctx, true).await?;
    let link = &subject.fronts.pulse;
    let watched = Watched::default();
    let ends = leg_ends(subject.position);
    let lossy = Fault::Link {
        delay: Duration::from_millis(100),
        jitter: Duration::from_millis(100),
        loss: 0.02,
    };
    let (marks, before, after) = watch_while(&subject, ends, &watched, async |_| {
        let plain = link.clock();
        tokio::time::sleep(BASELINE).await;
        let slowed = link.clock();
        link.set(Direction::Both, lossy);
        tokio::time::sleep(SETTLE).await;
        let measured = link.clock();
        let before = link.counters();
        tokio::time::sleep(MEASURE_LOSSY).await;
        (
            [plain, slowed, measured, link.clock()],
            before,
            link.counters(),
        )
    })
    .await;
    let up = link.history(Direction::Uplink);
    let down = link.history(Direction::Downlink);
    let legs = watched.legs.lock().clone();
    let samples = watched.samples.lock().clone();
    let (baseline, _) = read(&up, &legs, marks[0], marks[1], &samples);
    let (measured, silences) = read(&up, &legs, marks[2], marks[3], &samples);
    // each silence beside the losses that came before it: a hold a loss started follows it
    let attributed: Vec<Value> = silences
        .gaps
        .iter()
        .map(|gap| {
            let from = gap.start.saturating_sub(LOSS_LOOKBACK);
            let (up_small, up_data) = lost_between(&up, from, gap.start);
            let (down_small, down_data) = lost_between(&down, from, gap.start);
            json!({
                "at_s": seconds(gap.start),
                "gap_ms": ms(gap.length),
                "beyond_beat_ms": ms(gap.length.saturating_sub(Duration::from_millis(measured.median_gap_ms))),
                "lost_before": { "up_small": up_small, "up_data": up_data, "down_small": down_small, "down_data": down_data },
            })
        })
        .collect();
    let after_a_loss = attributed
        .iter()
        .filter(|silence| {
            let lost = &silence["lost_before"];
            ["up_small", "up_data", "down_small", "down_data"]
                .iter()
                .any(|key| lost[key].as_u64().unwrap_or(0) > 0)
        })
        .count();
    // how often a loss falls that close before any moment of the walk, for comparison
    let spans = spans(&legs, marks[2], marks[3]);
    let moments: Vec<Duration> = spans
        .iter()
        .flat_map(|&(start, end)| {
            let mut at = start;
            std::iter::from_fn(move || {
                at += Duration::from_millis(250);
                (at < end).then_some(at)
            })
        })
        .collect();
    let chance = if moments.is_empty() {
        0.0
    } else {
        moments
            .iter()
            .filter(|&&at| {
                let from = at.saturating_sub(LOSS_LOOKBACK);
                let (a, b) = lost_between(&up, from, at);
                let (c, d) = lost_between(&down, from, at);
                a + b + c + d > 0
            })
            .count() as f64
            / moments.len() as f64
    };
    let lost = after.dropped - before.dropped;
    let beat = Duration::from_millis(measured.median_gap_ms);
    let long: Vec<&Value> = attributed
        .iter()
        .filter(|silence| {
            silence["beyond_beat_ms"].as_u64().unwrap_or(0) > ms(LOSSY_SILENCE_ALLOWED)
        })
        .collect();
    ctx.note(
        "path",
        json!({ "delay_ms": 100, "jitter_ms": 100, "loss": 0.02, "each_way": true, "round_trip_ms": [200, 400] }),
    );
    ctx.note(
        "datagrams lost and delayed while measured, both ways",
        (lost, after.delayed - before.delayed),
    );
    let measured_up: Vec<&Seen> = up
        .iter()
        .filter(|seen| seen.at >= marks[2] && seen.at < marks[3])
        .collect();
    let measured_down: Vec<&Seen> = down
        .iter()
        .filter(|seen| seen.at >= marks[2] && seen.at < marks[3])
        .collect();
    let lost_of = |seen: &[&Seen]| {
        (
            seen.iter().filter(|seen| seen.fate == Fate::Lost).count(),
            seen.len(),
        )
    };
    ctx.note(
        "lost of all while measured: uplink, downlink",
        (lost_of(&measured_up), lost_of(&measured_down)),
    );
    ctx.note("plain link", &baseline);
    ctx.note("lossy link", &measured);
    ctx.note(
        "silences and the losses in the 1.2 s before each",
        &attributed,
    );
    ctx.note(
        "silences with a loss in the 1.2 s before, of all",
        (after_a_loss, attributed.len()),
    );
    ctx.note(
        "share of moments of the walk with a loss in the 1.2 s before",
        (chance * 1000.0).round() / 1000.0,
    );
    ctx.note(
        "the explorer's last uplink readout",
        watched.uplink.lock().clone(),
    );
    if let Some(failed) = watched.failed.lock().clone() {
        bail!("the walk failed: {failed}");
    }
    if lost == 0 {
        bail!("the link lost nothing while it was measured: the fault touched nothing");
    }
    expect_baseline(&baseline)?;
    if !long.is_empty() || measured.silences > LOSSY_SILENCES_ALLOWED {
        bail!(
            "over a link with a round trip of 300 ms give or take 100 and 2 % loss each way, the walking explorer's movement stopped {} times for more than {} ms beyond its {} ms beat (none allowed), and {} silences of over {} ms in all in {} s (at most {LOSSY_SILENCES_ALLOWED} allowed); {} of them after a loss in the 1.2 s before, the readout showed an input held in {}; the longest gap {} ms",
            long.len(),
            ms(LOSSY_SILENCE_ALLOWED),
            ms(beat),
            measured.silences,
            measured.silence_over_ms,
            measured.walked_s,
            after_a_loss,
            measured.silences_seen_held,
            measured.longest_silence_ms,
        );
    }
    subject.explorer.stop().await;
    Ok(())
}

/// Data datagrams of the uplink a queue kept: when each arrived.
fn queued_data(up: &[Seen], from: Duration, to: Duration) -> Vec<Duration> {
    up.iter()
        .filter(|seen| {
            seen.fate == Fate::Queued && seen.bytes >= DATA_BYTES && seen.at >= from && seen.at < to
        })
        .map(|seen| seen.at)
        .collect()
}

async fn queued_stall(ctx: Ctx, round_trip: Duration) -> Result<()> {
    let subject = start(&ctx, true).await?;
    let bot = observer(&ctx, &subject).await?;
    let link = &subject.fronts.pulse;
    let wallet = subject.wallet.clone();
    let watched = Watched::default();
    let home = subject.position;
    let ends = [
        [home[0] - LEG_METRES / 4.0, home[1], home[2]],
        [home[0] + LEG_METRES / 4.0, home[1], home[2]],
    ];
    let disconnects_before = subject.explorer.log_count("pulse: disconnected");
    let leaves_before = bot.view().leaves_of(&wallet);
    let slow = slow_path(round_trip);

    let outcome = watch_while(&subject, ends, &watched, async |_| {
        let plain = link.clock();
        tokio::time::sleep(BASELINE).await;
        let slowed = link.clock();
        link.set(Direction::Both, slow);
        tokio::time::sleep(SETTLE).await;
        let before = link.counters();
        let stalled = link.clock();
        link.set(Direction::Uplink, Fault::Queue { limit: STALL_QUEUE });
        tokio::time::sleep(STALL).await;
        link.set(Direction::Uplink, slow);
        let released = link.clock();
        let counters = link.counters();
        // the observer saw the explorer here when the burst went: it must see it walk on
        let seen_at_release = bot.view().of(&wallet).map(|seen| seen.world);
        let walked_on = expect_or(
            &ctx,
            "after the stall the observer sees the explorer walk on: 2 m from where it saw it when the queue let go",
            Duration::from_secs(20),
            || {
                bot.alive()?;
                let now = bot
                    .view()
                    .of(&wallet)
                    .map(|seen| seen.world)
                    .ok_or("the observer no longer holds the explorer")?;
                let from = seen_at_release.ok_or("the observer did not hold the explorer when the queue let go")?;
                let moved = ((now[0] - from[0]).powi(2) + (now[2] - from[2]).powi(2)).sqrt();
                (moved >= 2.0)
                    .then_some(())
                    .ok_or_else(|| format!("it moved {moved:.2} m, to {now:?}"))
            },
        )
        .await;
        // and the session stays up for a while after the burst
        tokio::time::sleep(Duration::from_secs(8)).await;
        (
            [plain, slowed, stalled, released, link.clock()],
            before,
            counters,
            walked_on,
        )
    })
    .await;
    let (marks, before, at_release, walked_on) = outcome;
    let up = link.history(Direction::Uplink);
    let legs = watched.legs.lock().clone();
    let samples = watched.samples.lock().clone();
    let (baseline, _) = read(&up, &legs, marks[0], marks[1], &samples);
    let queued = at_release.queued - before.queued;
    let released = at_release.released - before.released;
    let data = queued_data(&up, marks[2], marks[3]);
    let last_data_after = data.last().map(|at| ms(*at - marks[2]));
    let first_stall_seen = samples
        .iter()
        .find(|sample| sample.at >= marks[2] && sample.stalled == Some(true))
        .map(|sample| ms(sample.at - marks[2]));
    let after: Vec<&Sample> = samples
        .iter()
        .filter(|sample| sample.at >= marks[3])
        .collect();
    let not_authenticated: Vec<(f64, String)> = after
        .iter()
        .filter(|sample| sample.state != "authenticated")
        .map(|sample| (seconds(sample.at), sample.state.clone()))
        .collect();
    let disconnects = subject.explorer.log_count("pulse: disconnected") - disconnects_before;
    let leaves = bot.view().leaves_of(&wallet) - leaves_before;
    ctx.note("round trip set, ms", ms(round_trip));
    ctx.note("stall, ms", ms(STALL));
    ctx.note("plain link", &baseline);
    ctx.note(
        "datagrams the stall queued and released",
        (queued, released),
    );
    ctx.note(
        "data datagrams among them, released together (Pulse allows a burst of 16 inputs)",
        data.len(),
    );
    ctx.note(
        "the last data datagram the stall queued came this long after it began, ms",
        last_data_after,
    );
    ctx.note(
        "the explorer first said its uplink stalled this long after the stall began, ms",
        first_stall_seen,
    );
    ctx.note(
        "readouts after the release that were not authenticated (s, state)",
        not_authenticated.iter().take(10).collect::<Vec<_>>(),
    );
    ctx.note("times the observer saw the explorer leave", leaves);
    ctx.note(
        "explorer log, Pulse disconnects",
        last_log_lines(&subject.explorer, "pulse: disconnected", 4),
    );
    ctx.note(
        "the explorer's last uplink readout",
        watched.uplink.lock().clone(),
    );
    if let Some(failed) = watched.failed.lock().clone() {
        bail!("the walk failed: {failed}");
    }
    if queued == 0 || released != queued {
        bail!(
            "the stall queued {queued} datagrams and released {released}: the fault did not act as a queue"
        );
    }
    expect_baseline(&baseline)?;
    if disconnects > 0
        || after
            .iter()
            .any(|sample| sample.state == "terminal disconnect")
    {
        bail!(
            "the explorer's Pulse session ended after the burst of {} data datagrams the stall released (Pulse allows {SERVER_INPUT_BURST}): {:?}",
            data.len(),
            last_log_lines(&subject.explorer, "pulse: disconnected", 2)
        );
    }
    walked_on?;
    if let Some(&(at, ref state)) = not_authenticated.last() {
        bail!("the explorer reported Pulse as {state} at {at} s, after the stall");
    }
    subject.explorer.stop().await;
    Ok(())
}
