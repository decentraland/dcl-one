//! Task spawning behind one boxed future type.
//!
//! `tokio::spawn` compiles tokio's task harness once per future type, in the
//! crate that names the future. Boxing the future first compiles it once here
//! for unit tasks and once per output type otherwise. `JoinHandle` semantics
//! (abort, panics surfacing as `JoinError`) are tokio's own.

use std::future::Future;
use std::pin::Pin;

use tokio::task::JoinHandle;

/// A spawned future boxed to one type per output.
pub type BoxTask<T = ()> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// `tokio::spawn` for a task with no output.
#[track_caller]
pub fn spawn<F>(future: F) -> JoinHandle<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    spawn_boxed(Box::pin(future))
}

/// `tokio::spawn` for a task whose output the caller reads.
#[track_caller]
pub fn spawn_with<F, T>(future: F) -> JoinHandle<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    tokio::spawn(Box::pin(future) as BoxTask<T>)
}

#[track_caller]
fn spawn_boxed(future: BoxTask) -> JoinHandle<()> {
    tokio::spawn(future)
}
