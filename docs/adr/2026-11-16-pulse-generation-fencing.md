# Pulse generation fencing and complete selection discovery

Status: accepted

Each deployment audience has one current Pulse generation. The root process owns
its PostgreSQL session lease and writes a fresh UUID and increasing sequence epoch
to `social.pulse_authority_generation` on that same session before starting Pulse.
The UUID remains the opaque source incarnation on existing messages. Pulse receives
an interface containing the committed binding and local loss cancellation; it imports
no Gatekeeper code and persists no assignments or takeover ledger.

Gatekeeper checks the current binding through its existing database pool. Generation
row share locks precede wallet safety and realm locks and remain held through local
SQL mutation or credential authorization. A successor's row commit linearizes the
cutover. The binding is durable until then, with no periodic renewal write competing
with those share locks. Unknown source UUIDs cannot allocate selected-state capacity.

Mint and final socket delivery are separate operations. Each obtains a fresh,
exact-stamp, single-use permission with an original two-second deadline including
pool checkout. A stale source cannot obtain another permission after cutover.
Pre-cutover permissions may complete within their original deadline. Already issued
credentials, cleanup quarantine and accepted transport bytes retain their existing
finite validity; local lease-loss detection is not instantaneous physical revocation.

Pulse serves one immutable bounded selection view on source-specific NATS subjects.
Readers share its UUID, digest and ordered pages during its lifetime. Only a complete
view with a final current-generation check can seed reconstruction or support known
older-generation retirement. Entries are hints; fresh per-target selection and SQL
checks still decide admission. Subscriber discovery is a cooperative future in its
existing job, retaining one bounded vector and rotating through all its targets.

Obsolete selected slots require a known lower epoch, complete newer view, elapsed
pre-cutover permission window, exact uncertain-SQL settlement and no outstanding slot
handles. Foreign realm rows and independent cleanup/takeover obligations are retained.
UUID inequality alone cannot retire unidentified historical state.

The schema is applied before Pulse startup. Incompatible Pulse and Gatekeeper writers
must be upgraded together with admissions stopped. Database restoration requires the
same stopped-writer boundary and a fresh acquisition before reopening admissions;
a restored row grants no permission to reuse an old capability. Public client packet
shapes and credential validity budgets remain unchanged.

The encoding and queue limits are safety bounds. Renewal throughput, process memory
and recovery duration require measured runtime evidence.

The internal room-authority HTTP request includes `X-Pulse-Authority-Source` as a
canonical UUID. Its HMAC preimage is method, exact path, timestamp, nonce, source
UUID and body hash separated by newlines. Gatekeeper checks the current source
before policy evaluation and again under a generation share lock at lease issuance
or renewal. Missing headers and unregistered sources fail closed. This internal
contract requires coordinated client/server upgrade; protobuf and native packets
are unchanged. The HTTP client takes an immutable root capability and refuses late
responses or cached admission after local loss. Its production activation is not
provided by this change; the mounted authority endpoint enforces the binding.
