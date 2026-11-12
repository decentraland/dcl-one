# Shared room lifecycle in Explorer

- Status: Accepted
- Date: 2026-11-09

Explorer room implementations share admission, recovery and transport ownership in
the existing `comms` crate. Scene rooms, islands, private chat and on-demand voice
use the same LiveKit lifecycle and native/browser facade. Room-specific code supplies
credential sources, ownership, access scope and media capabilities. This keeps
cancellation, credential refresh and error handling consistent across room types.

## Ownership and recovery

Credential requests, pending connections, retry state and terminal failures belong
to ECS entities. Replacing or despawning an owner cancels its pending work. Completion
checks the current identity and owner before installing a transport. Successful
connection results own cleanup before ECS adoption, so cancellation cannot leave an
unobserved room connected.

One credential executor handles scene admission, private-chat admission and signed
login. Transient failures retry indefinitely with delays capped at five seconds;
server retry deadlines are respected. Refreshable sources obtain credentials before
an application reconnect. Fixed external credentials require replacement after
rejection. The LiveKit SDK retains responsibility for its internal reconnects.
Successful admission preserves retry history. One shared observer resets it after
30 continuous seconds of healthy transport state; interruptions restart that window.
This keeps brief successful reconnects from repeatedly restarting the one-second
delay. Island promotion retains the matching retry history until the active room
resets it. Explicit user and access resets remain immediate.
Credential responses cannot start another signed-login request; such responses
are invalid and stop recovery for that room.

Archipelago keeps its assignment protocol and uses the shared recovery policy. Its
owner tracks active and pending islands and rejects stale assignments. The active
island stays connected until the replacement is usable. Repeated assignments retain
an SDK connection already in progress, and credential replacement retains the island's
retry budget on its owner. Held-world transitions and
headless supervisor authority remain explicit connection gates.
Removal from an island requests a fresh assignment through its owner after
backoff. Removal from a credentialed scene or private room remains a scoped denial.

All adapter ingress uses one validator. Diagnostics expose safe failure categories
and connection phases rather than credentials or upstream response bodies. Signed
login, scene and private-chat admission include the installation identifier only
for HTTPS endpoints under decentraland.org or decentraland.zone.

## Access and media policy

The access-status contract lives in `comms` and contains room admission observations.
A denied admission stops automatic recovery for its scope without declaring a
platform-wide ban. Identity conflicts in on-demand voice and private chat stop only
that room; world-room identity conflicts retain the session gate. Identity changes
and explicit access updates reset the applicable state.

The social handler owns personal blocks in both directions and the readiness of its
snapshot. On-demand voice playback waits for a complete snapshot belonging to the
current wallet instance. Disconnects and same-address wallet replacements invalidate
that readiness. A snapshot raced by a newer block update is retried; incomplete
snapshots retain valid DM block entries without authorizing voice playback.
Subscription and playback use the same eligibility decision. This receive-side
filter does not gate microphone publication; preventing a modified peer from
receiving audio requires enforcement by the room's token issuer.

One wallet-owned on-demand session uses the shared room lifecycle. Trusted SystemApi
callers activate, replace or leave it. Its reservation suppresses nearby publication
while connecting, recovering or failed. Leaving preserves the saved nearby mute and
never overrides a current mute or unavailable microphone. Wallet replacement and
headless cancellation disable capture without restoring the previous session's state.

Social calls and community streams retain their existing controller, independent
call mute and call-ending retry budget. The existing owner keeps the microphone
reservation; call entry and on-demand activation reject competing requests. The call
controller runs before on-demand activation in the existing RestrictedActions slot,
so simultaneous requests have a deterministic result. A deferred community start
retains its reservation. Incoming ringing does not displace an on-demand session.

One publication decision serves both native and browser clients. An active on-demand
session permits only its selected room; otherwise the call and nearby policies apply.
An active call suppresses nearby publication without changing its saved preference.
Every replacement waits for acknowledged capture cleanup. Failed publication uses the
shared capped retry policy after cleanup; known native publish denial waits for
permission to return. Private chat remains data-only.

Browser teardown disposes remote playback independently of SDK disconnect events.
Each audio route has one gain writer: nearby audio combines spatial attenuation,
voice volume and transmitter gain; on-demand audio is centered and uses voice volume;
social calls use voice-chat volume; other tracks use scene volume. Native subtracks
apply scene and voice-chat gain once, while on-demand sounds use the main output.

## Connectivity feedback

One engine watchdog owns connectivity sampling, loss grace and reachability probes.
It requires previous connectivity and excludes intentional offline states and held
worlds. Terminal or suspended rooms are excluded from the required-room sample;
their failures do not suppress monitoring of other required rooms. A healthy required transport suppresses an
offline warning. The HUD presents this shared state through the existing engine
bridge stream, receiving an initial snapshot and subsequent changes, and supplies
Retry and Exit controls. The two HUD hosts use one connectivity component and session
hook in `library/ui/apps/hud`, with their wire types in `library/explorer-core`.
The HUD does not implement another recovery or probe policy.
Debug sections and the authenticated automation surface expose the same safe status
and actions. One room-state calculation reads the shared failure record, suspension
marker and transport phase for connectivity and room diagnostics. First admission
is distinct from recovery; a stable connection resets the failed-attempt count.

## Consequences

Room adapters share admission and recovery rather than acquiring independent retry
loops. The social call's bounded redial count remains its call-ending policy.
Cancellation, scoped access and credential refresh are tested through common ECS
boundaries; platform-specific SDK and audio operations stay behind existing facades.
The social call UI and trusted on-demand API remain separate consumers of these
shared mechanisms. The on-demand API does not add another calling product UI.
