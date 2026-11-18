//! What a run found: one result per scenario, printable and as JSON.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use super::timeline::Entry;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    Passed,
    Failed {
        /// The first expectation that did not hold.
        expectation: String,
        /// The seed that replays it.
        seed: u64,
        repeat: u32,
        timeline: Vec<Entry>,
    },
    /// Not run: the stack lacks something the scenario needs. Never counted as passed.
    Skipped { reason: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct Note {
    pub repeat: u32,
    pub key: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScenarioResult {
    pub name: String,
    pub guards: Vec<String>,
    #[serde(flatten)]
    pub outcome: Outcome,
    pub repeats_run: u32,
    pub duration_ms: u64,
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub run_id: String,
    pub first_seed: u64,
    pub hosts: Vec<String>,
    pub results: Vec<ScenarioResult>,
}

impl Report {
    pub fn new(run_id: &str, first_seed: u64, hosts: Vec<String>) -> Report {
        Report {
            run_id: run_id.to_owned(),
            first_seed,
            hosts,
            results: Vec::new(),
        }
    }

    pub fn count(&self) -> (usize, usize, usize) {
        let mut counts = (0, 0, 0);
        for result in &self.results {
            match result.outcome {
                Outcome::Passed => counts.0 += 1,
                Outcome::Failed { .. } => counts.1 += 1,
                Outcome::Skipped { .. } => counts.2 += 1,
            }
        }
        counts
    }

    /// Whether the run failed: any failure, or under `strict` any skip, or nothing run at all.
    pub fn failed(&self, strict: bool) -> bool {
        let (passed, failed, skipped) = self.count();
        failed > 0 || (strict && skipped > 0) || passed + failed + skipped == 0
    }

    pub fn write_json(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text)
            .with_context(|| format!("writing the report to {}", path.display()))
    }

    /// The report as lines for a terminal. A failure shows the end of its timeline.
    pub fn render(&self, timeline_tail: usize) -> String {
        let mut out = String::new();
        for result in &self.results {
            match &result.outcome {
                Outcome::Passed => out.push_str(&format!(
                    "PASS     {}  ({} repeat{}, {} ms)\n",
                    result.name,
                    result.repeats_run,
                    if result.repeats_run == 1 { "" } else { "s" },
                    result.duration_ms
                )),
                Outcome::Skipped { reason } => {
                    out.push_str(&format!("SKIPPED  {}  ({reason})\n", result.name))
                }
                Outcome::Failed {
                    expectation,
                    seed,
                    repeat,
                    timeline,
                } => {
                    out.push_str(&format!(
                        "FAIL     {}  (repeat {repeat}, replay with --seed {seed})\n",
                        result.name
                    ));
                    out.push_str(&format!("         {expectation}\n"));
                    if !result.guards.is_empty() {
                        out.push_str(&format!("         guards: {}\n", result.guards.join(", ")));
                    }
                    let skip = timeline.len().saturating_sub(timeline_tail);
                    if skip > 0 {
                        out.push_str(&format!("         … {skip} earlier timeline entries\n"));
                    }
                    for entry in &timeline[skip..] {
                        out.push_str(&format!(
                            "         {:>7} ms  {:<14} {:<18} {}\n",
                            entry.at_ms, entry.actor, entry.kind, entry.detail
                        ));
                    }
                }
            }
            for note in &result.notes {
                out.push_str(&format!(
                    "         note [{}] {} = {}\n",
                    note.repeat, note.key, note.value
                ));
            }
        }
        let (passed, failed, skipped) = self.count();
        out.push_str(&format!(
            "\n{passed} passed, {failed} failed, {skipped} skipped  (run {}, first seed {})\n",
            self.run_id, self.first_seed
        ));
        out
    }
}
