# Observing stalled Explorer core HTTP traffic

- Status: Accepted
- Date: 2026-11-03

Explorer starts trusted core requests immediately. Each attempt contributes one active
observation until its owner finishes reading the response, hands off a browser streaming
response at headers, or drops the attempt. Scene Fetch and IPFS content transfers have
their own scheduling owners and do not contribute a second observation here.

Rust's ordinary buffered fetch starts one guard. Callers that read a reqwest response
directly hold the guard from dispatch through their own body read. A retry starts a new
guard after its backoff. Browser callers use the shared JSON reader for ordinary reads
and return the original response at headers when another caller owns the stream.

The snapshot reports the number active, the highest number observed, the age of the oldest
active attempt in milliseconds, and a suspected-starvation flag. Crossing from 14 to 15
starts an observation window. If at least 15 remain active for five seconds without a
completion, the flag becomes true. Further starts leave that window running. A successful,
failed or cancelled attempt counts as progress and starts a new window if at least 15
remain. Falling below 15 clears it. An idle snapshot has an oldest age of zero.
An attempted dispatch that the client rejects locally, such as an invalid URL, counts
as a failed completion even when no bytes reach the network.

Native core requests report to their process. In the web Explorer the page combines trusted
HUD and worker observations. Worker and request events carry opaque identifiers. Instrumentation
records no URLs, headers or bodies. Poll `globalThis.__dclCoreHttp.snapshot()` in the HUD
and engine document (`/play/app.html` on Sites). Its host lives until that document closes
and survives a HUD remount. The outer shell forwards its activity to this host.
