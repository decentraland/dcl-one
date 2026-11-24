//! How the matrices run their cells, and their audit.
//!
//! [`in_worlds`] spreads a matrix's cells over up to [`WORLDS`] worlds, one
//! for every [`CELLS_PER_WORLD`] cells, each with its own scratch database,
//! and runs each world's cells one at a time: a cell's whole-database
//! snapshot sees only its own writes.
//!
//! With `BUILDER_MATRIX_CELLS` naming a file, every matrix cell a test
//! evaluates appends one tab-separated line to it ([`record`]): the test's
//! condition, the operation, the resource state, the caller, the mount, the
//! expected answer, the answer and the checks the cell ran. Two runs' sorted
//! files list the same lines exactly when they assert the same cells.

use std::future::Future;
use std::io::Write;
use std::sync::{Arc, Mutex};

/// The most worlds a matrix test runs its cells on at once.
pub const WORLDS: usize = 3;

/// The cells that earn a world of their own.
pub const CELLS_PER_WORLD: usize = 1000;

/// Runs every cell of `cells` with `run` on one of the worlds that `world`
/// makes (one per [`CELLS_PER_WORLD`] cells, at most [`WORLDS`]), and
/// returns the failures `run` reports; `None` (skip) when `world` makes
/// none.
pub async fn in_worlds<W, C, WF, WFut, R, RF>(
    cells: Vec<C>,
    world: WF,
    run: R,
) -> Option<Vec<String>>
where
    W: Send + Sync + 'static,
    C: Send + 'static,
    WF: Fn() -> WFut + Clone + Send + 'static,
    WFut: Future<Output = Option<W>> + Send,
    R: Fn(Arc<W>, C) -> RF + Clone + Send + Sync + 'static,
    RF: Future<Output = Option<String>> + Send,
{
    let count = cells.len().div_ceil(CELLS_PER_WORLD).clamp(1, WORLDS);
    let queue = Arc::new(Mutex::new(cells));
    let mut worlds = tokio::task::JoinSet::new();
    for _ in 0..count {
        let (queue, world, run) = (queue.clone(), world.clone(), run.clone());
        worlds.spawn(async move {
            let world = Arc::new(world().await?);
            let mut failures = Vec::new();
            loop {
                let next = queue.lock().unwrap().pop();
                let Some(cell) = next else { break };
                failures.extend(run(world.clone(), cell).await);
            }
            Some(failures)
        });
    }
    let mut failures = Vec::new();
    for world in worlds.join_all().await {
        failures.extend(world?);
    }
    Some(failures)
}

/// A pool over a fresh scratch database for one world, whose sessions
/// commit without waiting for the disk.
pub async fn world_pool(connections: u32) -> Option<sqlx::PgPool> {
    let pool = crate::db::scratch::isolated_pool(connections).await?;
    let options = pool
        .connect_options()
        .as_ref()
        .clone()
        .options([("synchronous_commit", "off")]);
    pool.close().await;
    Some(
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(connections)
            .connect_lazy_with(options),
    )
}

/// Appends the line `line` makes to the `BUILDER_MATRIX_CELLS` file, when
/// set.
pub fn record(line: impl FnOnce() -> String) {
    let Some(path) = std::env::var_os("BUILDER_MATRIX_CELLS") else {
        return;
    };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("BUILDER_MATRIX_CELLS names a writable file");
    file.write_all(format!("{}\n", line()).as_bytes())
        .expect("appending a matrix cell");
}
