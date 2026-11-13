# Stats local content source

- Status: Accepted
- Date: 2026-11-12

## Decision

Every `serve` enriches hot-scene statistics from its existing content PostgreSQL
pool. HTTP and the presence archive use the same selected-document source and
scene projection. Pulse remains the sole authority for the statistics population;
each request or archive pass retains one complete snapshot across content I/O.

The source selects active local entity documents for the populated parcel
pointers. It groups active pointers by entity ID, excludes deleted deployments
and preserves the query's deterministic local order. Selected documents retain
stored metadata property order. The existing shared content denylist filters
their entity IDs before scene projection, stable count sorting and the top-100
output limit. Surviving documents keep their relative order. HTTP and archive
receive the same policy instance used by local content reads, so policy updates
apply on subsequent enrichment passes without a separate reload job.

`CONTENT_URL` supplies the existing thumbnail URL concatenation. It does not
select a remote active-entities source. Enrichment is uncached: the local content
corpus, active selection and metadata at query time determine the candidates.
An empty statistics population skips content I/O. A SQL or projection failure
remains a failed request or archive pass rather than a successful empty result.
The archive continues projecting only string values for its textual fields.

## Consequences

This keeps content ownership and policy in the content module and uses one
existing pool, without another network dependency, cache or policy poller.
It intentionally accepts source-dependent differences from an independently
configured remote content service: corpus, freshness, selection, response order,
multiplicity, ranking and resulting top-100 membership can differ. Remote request
limits, cache behavior and HTTP failures do not define this local SQL source's
outcomes. Correct selected-document rendering, denylist enforcement and retained
snapshot consistency remain required.

Revisit this choice when a required deployment needs independently configured
remote content responses or the local source changes its public selection or
failure semantics.
