# ADR: Shared asynchronous utilities

- Status: Accepted
- Date: 2026-11-07

## Decision

Keep dependency-free TypeScript timing mechanisms in `library/async/`, consumed directly by repository tooling for Shop parity, trade journeys and Redis conformance fixtures. Node tools import the source directly. No separately published package is required.

Use `waitUntil` for bounded condition waiting, `backoffDelay` for calculating a delay, and `sleep` for a cancellable duration wait. Use snake case for equivalent custom helpers in Rust, Python and Bash. Preserve names supplied by external libraries and the standard library. A condition returning `false` or `undefined` requests another check. Other results complete the generic TypeScript waiter, including a zero balance. Errors propagate.

The waiter checks immediately, doubles its delay up to the configured cap, clips sleeps to the remaining timeout and rejects on cancellation or timeout. Timeouts must be finite, greater than 0, and at most 2,147,483,647 milliseconds, the timer limit shared by Node and browsers; invalid budgets reject before checking the condition. The timeout includes asynchronous checks. Checks that perform I/O must also bound their work; a timed-out promise does not cancel its underlying operation automatically.

TypeScript `backoffDelay` numbers attempts from zero: attempt 0 returns the first delay. Bash `backoff_delay` and `retry_after_attempt` number attempts from one: attempt 1 uses the first delay. Python `backoff_delays` yields the first delay directly.

Share mechanisms rather than imposing one timing policy. UI updates, process startup, RPC receipts and network failures retain their own named delay and timeout policies at the caller. Jitter and growth factors belong to those policies. Existing operation budgets are preserved unless separate evidence justifies changing them; they are ceilings, not mandatory delays or measured service guarantees.

A browser consumer must add its own bundler resolution and include the shared runtime in its shipped-source fingerprints and Nix inputs when adopted. Edits confined to `library/async/` do not invalidate existing web engine or HUD artifacts. The Catalyst/API image's Nix fileset and image-build CI selection include all of `library/`, so those edits change its derivation and select image-build jobs.

The HTTP probe accepts responses below 500 and releases their bodies. A non-server-error response alone does not establish application readiness; callers retain their process and readiness conditions.
