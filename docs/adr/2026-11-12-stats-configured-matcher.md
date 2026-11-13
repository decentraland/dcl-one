# Stats configured route matching

- Status: Accepted
- Date: 2026-11-12

## Decision

The Catalyst root owns the Stats metrics route compiler and evaluator in
`services/catalyst/src/metrics_matcher`. Comms owns the asynchronous, fallible
`stats::http::MetricsMatcher` interface. Its HTTP state receives a prepared matcher
and corresponding settings; connector native routing remains separate.
The adapter uses the existing pinned embedded V8 and the exact path-to-regexp
6.3.0 distribution, with its license and hashes beside the compiler. This module
belongs to the application root because it connects the lower-layer interface to
the process runtime; it needs no new crate or deployable.

Configured regex acceptance and matching follow that embedded engine. This is an
intentional semantic exception: some duplicate named-capture configurations
accepted by the original Stats runtime fail startup here. Invalid configurations
fail with compiler context before listener binding. UTF-8 configuration and request
paths pass as values, without generated JavaScript interpolation or lossy conversion.

One compiled isolate, context and regex serve every clone of the prepared matcher.
An asynchronous semaphore admits one blocking operation. Its owned permit moves
into the actual Tokio blocking closure and remains there until V8 scopes, Locker
and captured state are released. Canceling a request does not release a queued or
running operation's permit. Other waiters suspend before submitting work.
The supported SharedIsolate API serializes native access; retained Globals drop
before its final teardown. No scene state, callbacks or embedder slots enter it.

Serve initializes the common Worlds process platform on its startup thread before
constructing runtime workers. CLI parsing, posture expansion and secret-argument
refusal precede this setup; other commands do not prepare Stats. Low-level library
preparation is explicitly unsafe: its caller must maintain permitted thread ancestry
for every runtime worker, clone, canceled operation and final native destructor.
Calling the common Once on an older thread does not establish that ancestry. The
opaque prepared value and async serve API do not make arbitrary runtimes safe.

The large CLI tree is constructed and parsed on a short-lived thread with a
64 MiB stack, because it overflows the default Windows debug main stack. Posture
expansion runs before that thread starts; the parser returns only CLI values or
a Clap error and joins before validation, help/error exit, engine initialization
or runtime creation. It owns no matcher state and performs no runtime work.

Root routing first classifies the structural Stats candidates. Unrelated shared
routes bypass the matcher; a configured metrics collision suppresses a Stats root
claim. Selected requests retain distinct raw selection and normalized dispatch
inputs. Standalone Stats keeps its scope, methods, health precedence and bearer
authorization. Recoverable evaluator failures return an empty 500, with no success
fallback; the registry records an empty handler and code 200 before that outer
failure response. Observed request duration begins before configured route selection
and includes selection and handler work. Claimed root requests retain one interval
across both raw selection and normalized dispatch, including selection failures.
The dispatcher excludes health GET/HEAD and OPTIONS from observations; a root
metrics collision delegates without a Stats observation.

## Consequences

Comms builds remain independent of V8 and Worlds. Ordinary tests inject explicit
engine-free matchers. The dedicated engine integration target has one initializer
test: it creates a fresh child from the actual initializing thread and confines all
runtime and matcher state, including destruction and panic payloads, to that subtree.

The single submitted operation bounds blocking-pool occupation per matcher. It
does not bound regex duration, asynchronous waiter count, request memory or shutdown
time. Cancellation cannot stop native work; native fatal failures remain process
failures. No timeout, pattern restriction, global engine flag or alternate evaluator
is introduced. Finite Windows checks do not establish Linux memory-protection-key
coverage, every regex input, deployment readiness or scene-runtime isolation.

Revisit this choice when the compiler or engine pin changes, a configured pattern
exposes a material semantic difference, measured runtime costs violate an agreed
budget, or a required lifecycle boundary cannot preserve these constraints.
