//! Serve's configured Stats route compiler. Comms owns the engine-free interface.
use crate::comms::stats::http::{MatcherError, MetricsMatcher, Settings};
use std::sync::Arc;
use tokio::sync::Semaphore;

const COMPILER: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/services/catalyst/src/metrics_matcher/path-to-regexp.cjs"
));

/// Opaque prepared Stats configuration. Its unsafe creator must maintain permitted
/// thread ancestry through every clone, canceled operation and final native teardown.
/// It is not safe to move this value onto an arbitrary preexisting runtime.
pub struct PreparedStats {
    pub(crate) settings: Settings,
    matcher: Arc<dyn MetricsMatcher>,
}

impl PreparedStats {
    pub(crate) fn into_parts(self) -> (Settings, Arc<dyn MetricsMatcher>) {
        (self.settings, self.matcher)
    }
}

/// Compile a configured Stats matcher before creating Serve's runtime workers.
/// Compilation errors retain their compiler context and never select a fallback.
///
/// # Safety
/// The caller must first initialize the common Worlds V8 platform. This thread
/// must perform that initialization or inherit its permitted PKRU state: merely
/// returning from the common Once on a preexisting thread does not establish this.
/// Throughout the returned value's entire lifetime, every thread entering,
/// executing, locking or performing final native teardown must have that ancestry.
/// Every Tokio runtime polling the matcher must send its blocking work to such
/// threads. All state/router/Arc clones, canceled submitted work and panic payloads
/// must remain inside that boundary. Join all submitted work and release all
/// retained handles and isolates on permitted threads before destroying the owning
/// runtime. Keep the process platform alive; never dispose or reinitialize it.
/// The caller must not use this API if it cannot uphold those lifetime obligations.
pub unsafe fn prepare(settings: Settings) -> Result<PreparedStats, MatcherError> {
    let compiled = Compiled::new(&settings.metrics_path)
        .map_err(|error| MatcherError(format!("compiling WKC_METRICS_PUBLIC_PATH: {error}")))?;
    Ok(PreparedStats {
        settings,
        matcher: Arc::new(Adapter {
            compiled: Arc::new(compiled),
            admission: Arc::new(Semaphore::new(1)),
        }),
    })
}

struct Adapter {
    compiled: Arc<Compiled>,
    admission: Arc<Semaphore>,
}

#[async_trait::async_trait]
impl MetricsMatcher for Adapter {
    async fn matches(&self, path: &str) -> Result<bool, MatcherError> {
        let compiled = self.compiled.clone();
        let subject = path.to_owned();
        admitted(self.admission.clone(), move || compiled.matches(&subject)).await
    }
}

async fn admitted<T: Send + 'static>(
    admission: Arc<Semaphore>,
    work: impl FnOnce() -> Result<T, MatcherError> + Send + 'static,
) -> Result<T, MatcherError> {
    let permit = admission
        .acquire_owned()
        .await
        .map_err(|_| MatcherError("metrics matcher admission closed".into()))?;
    // No await between acquiring and submitting. Only this closure owns the
    // permit after submission: canceling its caller cannot admit another job.
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        // work releases all V8 scopes, its Locker and captured Arc before permit.
        work()
    })
    .await
    .map_err(|error| MatcherError(format!("metrics matcher task failed: {error}")))?
}

// Fields drop in declaration order. Deferred Global resets are drained by final
// SharedIsolate teardown, after both retained handles have been released.
struct Compiled {
    context: v8::Global<v8::Context>,
    regex: v8::Global<v8::RegExp>,
    isolate: v8::SharedIsolate,
}

impl Compiled {
    fn new(pattern: &str) -> Result<Self, MatcherError> {
        let mut isolate = v8::Isolate::new(Default::default());
        let (context, regex) = {
            v8::scope!(let scope, &mut isolate);
            let context = v8::Context::new(scope, Default::default());
            let context_scope = &mut v8::ContextScope::new(scope, context);
            v8::tc_scope!(let scope, context_scope);
            let source = v8::String::new(
                scope,
                &format!("var exports = {{}};\n{COMPILER}\nexports.pathToRegexp;"),
            )
            .ok_or_else(|| MatcherError("compiler source allocation failed".into()))?;
            let compiler =
                v8::Script::compile(scope, source, None).and_then(|script| script.run(scope));
            let compiler = match compiler {
                Some(value) if !scope.has_caught() && !scope.has_terminated() => {
                    v8::Local::<v8::Function>::try_from(value)
                        .map_err(|_| MatcherError("compiler did not return a function".into()))?
                }
                _ => return Err(engine_error(scope, "loading path-to-regexp")),
            };
            let pattern = v8::String::new(scope, pattern)
                .ok_or_else(|| MatcherError("configuration string allocation failed".into()))?;
            let receiver = v8::undefined(scope).into();
            let value = compiler.call(scope, receiver, &[pattern.into()]);
            let regex = match value {
                Some(value) if !scope.has_caught() && !scope.has_terminated() => {
                    v8::Local::<v8::RegExp>::try_from(value)
                        .map_err(|_| MatcherError("compiler did not return a RegExp".into()))?
                }
                _ => return Err(engine_error(scope, "compiling configured route")),
            };
            (
                v8::Global::new(scope, context),
                v8::Global::new(scope, regex),
            )
        };
        // SAFETY: This clean isolate has no embedder slots, callbacks, custom
        // allocator, weak handles, snapshots or cppgc. This remains true for its
        // whole lifetime. Globals are opened only as Locals under its Locker.
        // prepare's unsafe contract separately establishes PKRU/thread ancestry.
        let isolate = match unsafe { isolate.try_into_shared() } {
            Ok(isolate) => isolate,
            Err(error) => {
                // The conversion error owns an OwnedIsolate. Release Globals and
                // destroy that error here, on its permitted construction thread.
                let message = format!("sharing metrics isolate: {error:?}");
                drop(context);
                drop(regex);
                drop(error);
                return Err(MatcherError(message));
            }
        };
        Ok(Self {
            context,
            regex,
            isolate,
        })
    }

    fn matches(&self, subject: &str) -> Result<bool, MatcherError> {
        let mut lock = self.isolate.lock();
        v8::scope!(let scope, &mut *lock);
        let context = v8::Local::new(scope, &self.context);
        let context_scope = &mut v8::ContextScope::new(scope, context);
        v8::tc_scope!(let scope, context_scope);
        let regex = v8::Local::new(scope, &self.regex);
        let subject = v8::String::new(scope, subject)
            .ok_or_else(|| MatcherError("subject string allocation failed".into()))?;
        let result = regex.exec(scope, subject);
        if scope.has_terminated() || scope.has_caught() {
            return Err(engine_error(scope, "evaluating configured route"));
        }
        match result {
            Some(value) => Ok(!value.is_null()),
            None => Err(MatcherError("metrics RegExp returned no handle".into())),
        }
    }
}

fn engine_error(
    scope: &mut v8::PinnedRef<v8::TryCatch<v8::HandleScope>>,
    operation: &str,
) -> MatcherError {
    let detail = if scope.has_terminated() {
        "execution terminated".into()
    } else if let Some(exception) = scope.exception() {
        exception
            .to_string(scope)
            .map(|s| s.to_rust_string_lossy(scope))
            .unwrap_or_else(|| "exception could not be converted to a string".into())
    } else {
        "engine returned no handle without an exception".into()
    };
    MatcherError(format!("{operation}: {detail}"))
}

#[cfg(test)]
mod tests;

#[cfg(feature = "test-support")]
pub mod test_support;
