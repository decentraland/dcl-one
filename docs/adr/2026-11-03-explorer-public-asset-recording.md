# Native Explorer public asset recordings

Status: Proposed

A developer reproducing a native Bevy asset-loader fault needs the same content bytes across
runs, including when the first run used a warm Explorer cache. A capture is local to the
operator. CI and the authoritative scene server do not consume this format.

The desktop entry point opens an optional recording before starting the runtime. The default
Bevy asset source wraps its reader only when capture or replay is selected. The wrapper
qualifies a read by the connected public Genesis realm, the configured base domain, the
selected content endpoint and the absence of a scene-supplied base or realm secret. A CID
check establishes byte identity. The logical asset reader has no HTTP response metadata.

The workload is one optional append per successful eligible load and a lookup by CID on
replay. Many loads can name the same bytes. One writer holds the directory lock. It publishes
verified immutable blobs before appending completed reads. Opening a writer validates the
bounded directory, detaches the existing journal once and removes any incomplete tail. Each
subsequent read appends and syncs one line. Filesystem work and hashing run on a blocking
worker, while readers waiting for the writer yield. Each captured load waits for its append.

Replay opens the completed journal as an immutable index. An orphan left by an interrupted
capture has no entry. Reads require journal membership, the recorded length and a matching
CID. Eligible misses fail locally. Other traffic and the renderer's normal asset cache have
their own lifecycles.

See the [native reproduction procedure](../../rendering/bevy-explorer/docs/guidelines/running.md#reproduce-a-native-public-asset-read)
for commands, directory quotas and coverage.
