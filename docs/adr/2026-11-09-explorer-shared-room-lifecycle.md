# Shared room lifecycle in Explorer

- Status: Accepted
- Date: 2026-11-09

Explorer room implementations share admission, recovery and transport ownership in
the existing `comms` crate. Scene rooms, islands and private chat use the same
LiveKit lifecycle and native/browser facade. Room-specific code supplies
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

Archipelago keeps its assignment protocol and uses the shared recovery policy. Its
owner tracks active and pending islands and rejects stale assignments. The active
island stays connected until the replacement is usable. Repeated assignments retain
an SDK connection already in progress, and credential replacement retains the island's
retry budget on its owner. Held-world transitions and
headless supervisor authority remain explicit connection gates.

All adapter ingress uses one validator. Diagnostics expose safe failure categories
and connection phases rather than credentials or upstream response bodies. Signed
login includes the installation identifier only for HTTPS Decentraland endpoints.

## Access and media policy

The shared access-status contract lives in `common`. A denied admission stops
automatic recovery for its scope without declaring a platform-wide ban. An identity
conflict in private chat stops only that room. World-room identity conflicts retain
the session gate. Identity changes and explicit access updates reset the applicable
state.

Microphone publication requires a connected proximity transport. Native and browser
publication wait for acknowledged cleanup before starting replacement work. A failed
publication retries after cleanup, using the shared backoff capped at five seconds.
Known native publish denial suppresses attempts until permission returns.
Browser playback uses one mixer, and room teardown disposes its remote handles and
playback independently of asynchronous SDK disconnect events. Private chat remains data-only.

## Connectivity feedback

One engine watchdog owns connectivity sampling, loss grace and reachability probes.
It requires previous connectivity and excludes intentional offline states, held
worlds and terminal access failures. A healthy required transport suppresses an
offline warning. The HUD presents this shared state through the existing engine
bridge and supplies Retry and Exit controls; it does not implement another recovery
or probe policy. Debug sections and the authenticated automation surface expose the
same safe status and actions.

## Consequences

Room adapters remain small and do not acquire independent retry loops. Tests can
exercise cancellation, access isolation and credential refresh through common ECS
boundaries. Platform-specific SDK and audio operations remain behind their existing
facades.
