# Social analytics

Social uses the HTTP contract of `@dcl/analytics-component` 1.0.5 and
`@dcl/fetch-component` 1.1.1 installed by `decentraland/social-service-ea` at
`b6b2cefefdf727b2ed47ddb887e77d226a41eea6`. Its adapter shares the existing
`Outbound` handle. The communities state holds one adapter; voice uses that same
state through its existing context attachment.

Configure `ANALYTICS_CONTEXT`, `ANALYTICS_API_URL`, `ANALYTICS_API_TOKEN` and `ENV`.
The token also supports `ANALYTICS_API_TOKEN_FILE`, and cannot be supplied on
argv. Configured strings retain empty values. Partial analytics configuration
fails startup naming the missing value. With all three `ANALYTICS_*` strings
absent, the monolith disables this adapter. The standalone source requires all
four strings unconditionally: supplying them is a prerequisite for replacing
its analytics behavior, and optional monolith startup is a difference.

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
Every hop retains `Outbound`'s existing host guard and pinning. Other consumers
retain their configured redirect limits and no-follow behavior.

These transport differences remain: source Fetch permits destinations that the
shared guard refuses, and the shared client has an eight-second connect timeout
and a 25-second read timeout. Those limits can expire before the analytics
deadline, especially with larger configured values. No separate client or pool
is created to remove them. Configuration and destination policy need checking at
cutover; these differences are not a claim of complete transport equivalence.

The source fixtures execute its installed SDK, Node timer admission, actual
private-call PostgreSQL queries and voice domain functions with controlled media
responses. Target tests send mutations to a local HTTP provider, including
provider failures and cancellation of an unread response body. Redirect deadline
tests combine real HTTP hops with controlled timers; they do not measure kernel
buffering or production provider availability.
