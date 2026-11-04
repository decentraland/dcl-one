# Social analytics

Social uses the HTTP contract of `@dcl/analytics-component` 1.0.5 and
`@dcl/fetch-component` 1.1.1 installed by `decentraland/social-service-ea` at
`b6b2cefefdf727b2ed47ddb887e77d226a41eea6`. Its adapter receives the existing
`Outbound` handle and lazily derives one retained provider transport profile.
The communities state holds one adapter; voice uses that same state through
its existing context attachment.

Configure `ANALYTICS_CONTEXT`, `ANALYTICS_API_URL`, `ANALYTICS_API_TOKEN` and `ENV`.
The token also supports `ANALYTICS_API_TOKEN_FILE`, and cannot be supplied on
argv. Configured strings retain empty values. Partial analytics configuration
fails startup naming the missing value. Context, token, timeout or a nonempty
analytics URL activates configuration; `ENV` alone and an empty posture-injected
URL do not. A configured timeout also requires the four strings. The standalone
source requires all four strings unconditionally: supplying them is a
prerequisite for replacing its analytics behavior, and optional monolith
startup is a difference.

Each event starts one detached HTTP task, posting JSON
`{event, body: {...properties, env: ENV}, context: ANALYTICS_CONTEXT}` with
`x-token` and `Content-Type: application/json`. The configured environment wins
over a property named `env`. Mutations do not await the provider. HTTP errors,
timeouts and transport errors are logged and swallowed; POSTs are never retried.
The response body is cancelled without buffering or draining it. There is no
event queue, replay, durable delivery record or shutdown flush.

`ANALYTICS_REQUEST_TIMEOUT` uses source numeric-prefix parsing. A finite positive
value is used; zero, negative and infinite values fall back to 10,000 ms. Values
that cannot be parsed as a number fail configuration. The effective Node timer
bucket is preserved: fractional values truncate, and sub-millisecond or
greater-than-2,147,483,647 ms values use one millisecond. One deadline covers DNS
and the complete redirect chain until final response headers. Response-body
consumption is outside that deadline because the body is immediately cancelled.

Only analytics requests opt into Fetch's 20 redirects. HTTP 301, 302 and 303 turn
POST into GET and remove body headers; 307 and 308 preserve the method and body.
The source preserves `x-token` across host changes, and the adapter does too.
This operator-configured provider profile permits private destinations and
ordinary DNS, including redirect targets. Its destination policy must never
be used for URLs supplied by incoming requests or untrusted documents. Other
consumers retain their own host guards and redirect policies.

The provider profile creates an independent HTTP transport pool with no total
transport timeout, a ten-second connect timeout and a 300-second read timeout.
These replace the shared serving client's shorter limits for analytics only.
The native connection and per-hop header budgets operate inside the configured
SDK deadline across redirects, so the earlier applicable deadline wins. Local
source probes cover stalled TLS, headers and redirect chains; delayed DNS,
combined timing phases and deployed routing/provider behavior remain unverified.

The source fixtures execute its installed SDK, Node timer admission, actual
private-call PostgreSQL queries and voice domain functions with controlled media
responses. Target tests send mutations to a local HTTP provider, including
provider failures and cancellation of an unread response body. Redirect deadline
tests combine real HTTP hops with controlled timers; they do not measure kernel
buffering or production provider availability.
