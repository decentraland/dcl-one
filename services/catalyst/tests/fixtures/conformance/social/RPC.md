# social-service-ea RPC behaviour: upstream specs and Catalyst tests

Reference: `decentraland/social-service-ea` at `b6b2cefefdf727b2ed47ddb887e77d226a41eea6`. The spec
inventory below was counted at `fb3dcb8`; the HTTP routes and the `/v1/stream` socket that `b6b2cef` adds
(upstream #503-#507) are in "HTTP twins and `/v1/stream`" at the end. Local:
`services/catalyst/social/src/rpc` (the `/social-rpc` socket), `services/catalyst/shared/src/rpc` (handshake,
transport, dcl-rpc) and the voice runtime in `services/catalyst/comms/src/gatekeeper/voice`.

The handshake's header semantics come from `@dcl/crypto-middleware` 6.3.0, the version the reference's `yarn.lock`
resolves for its direct dependency (`yarn.lock:1020-1023`; not the transitive 4.1.1). Source: the published
tarball `https://registry.yarnpkg.com/@dcl/crypto-middleware/-/crypto-middleware-6.3.0.tgz`, fetched read-only on
2026-10-28, sha512 `HUwaPBuXjf3xSGIcnwm52NIZchaPg0rDe/bsjlYdo4ZpjuTlTeQ8m7fjTT3V14/pRTa55T404eHGZURU5jJDog==`
(equal to the `yarn.lock` integrity) and sha1 `855174cab339df1d62e288d2b533fdef6f3ea4d3` (the resolved URL's
fragment), extracted with `tar` and never run. Citations below are lines of its `package/dist/verify.js`. The
same functions read the same in `decentraland/core-libs` at tag `@dcl/crypto-middleware@6.3.0`
(`libs/crypto-middleware/src/verify.ts`).

Scope: the upstream unit specs of the RPC surface at the pin, 50 files and 811 cases. A case is a line that calls
`it(` or `test(`; the 10 `it.each`/`test.each` tables (2 in `friendships`, 6 in `settings.utils`, 2 in
`notifications`) do not match that and are not in the count, though their behaviour is mapped below. This prints
`50 files, 811 cases` from a checkout of the reference repository:

```sh
git ls-tree -r --name-only fb3dcb8 test/unit \
  | grep -E '^test/unit/(controllers/handlers/ws/ws-handler|adapters/rpc-server|adapters/rpc-server/services/[^/]+|logic/(blocks|friends-logic|friends|friendships|settings/settings|settings/settings\.utils|voice-logic|community-voice|community-voice-cache|community-voice-polling|updates|ws-pool-component|peers-stats|profiles|notifications))\.spec\.ts$' \
  | while read -r f; do git show "fb3dcb8:$f" | grep -cE '\b(it|test)\('; done \
  | awk '{ n++; s += $1 } END { print n " files, " s " cases" }'
```

The files, with their case counts:

| spec file at `fb3dcb8` | cases |
| --- | --- |
| `test/unit/controllers/handlers/ws/ws-handler.spec.ts` | 43 |
| `test/unit/adapters/rpc-server.spec.ts` | 15 |
| `test/unit/adapters/rpc-server/services/accept-private-voice-chat.spec.ts` | 4 |
| `test/unit/adapters/rpc-server/services/block-user.spec.ts` | 4 |
| `test/unit/adapters/rpc-server/services/demote-speaker-in-community-voice-chat.spec.ts` | 37 |
| `test/unit/adapters/rpc-server/services/end-community-voice-chat.spec.ts` | 7 |
| `test/unit/adapters/rpc-server/services/end-private-voice-chat.spec.ts` | 3 |
| `test/unit/adapters/rpc-server/services/get-blocked-users.spec.ts` | 3 |
| `test/unit/adapters/rpc-server/services/get-blocking-status.spec.ts` | 5 |
| `test/unit/adapters/rpc-server/services/get-friends.spec.ts` | 3 |
| `test/unit/adapters/rpc-server/services/get-friendship-status.spec.ts` | 8 |
| `test/unit/adapters/rpc-server/services/get-incoming-private-voice-chat-requests.spec.ts` | 3 |
| `test/unit/adapters/rpc-server/services/get-mutual-friends.spec.ts` | 5 |
| `test/unit/adapters/rpc-server/services/get-pending-friendship-requests.spec.ts` | 5 |
| `test/unit/adapters/rpc-server/services/get-private-messages-settings.spec.ts` | 5 |
| `test/unit/adapters/rpc-server/services/get-sent-friendship-requests.spec.ts` | 5 |
| `test/unit/adapters/rpc-server/services/get-social-settings.spec.ts` | 3 |
| `test/unit/adapters/rpc-server/services/join-community-voice-chat.spec.ts` | 6 |
| `test/unit/adapters/rpc-server/services/kick-player-from-community-voice-chat.spec.ts` | 28 |
| `test/unit/adapters/rpc-server/services/mute-speaker-from-community-voice-chat.spec.ts` | 9 |
| `test/unit/adapters/rpc-server/services/promote-speaker-in-community-voice-chat.spec.ts` | 38 |
| `test/unit/adapters/rpc-server/services/reject-private-voice-chat.spec.ts` | 3 |
| `test/unit/adapters/rpc-server/services/reject-speak-request-in-community-voice-chat.spec.ts` | 33 |
| `test/unit/adapters/rpc-server/services/request-to-speak-in-community-voice-chat.spec.ts` | 11 |
| `test/unit/adapters/rpc-server/services/start-community-voice-chat.spec.ts` | 5 |
| `test/unit/adapters/rpc-server/services/start-private-voice-chat.spec.ts` | 7 |
| `test/unit/adapters/rpc-server/services/subscribe-to-block-updates.spec.ts` | 6 |
| `test/unit/adapters/rpc-server/services/subscribe-to-community-member-connectivity-updates.spec.ts` | 7 |
| `test/unit/adapters/rpc-server/services/subscribe-to-community-voice-chat-updates.spec.ts` | 11 |
| `test/unit/adapters/rpc-server/services/subscribe-to-friend-connectivity-updates.spec.ts` | 11 |
| `test/unit/adapters/rpc-server/services/subscribe-to-friendship-updates.spec.ts` | 6 |
| `test/unit/adapters/rpc-server/services/subscribe-to-private-voice-chat-updates.spec.ts` | 8 |
| `test/unit/adapters/rpc-server/services/unblock-user.spec.ts` | 5 |
| `test/unit/adapters/rpc-server/services/upsert-friendship.spec.ts` | 7 |
| `test/unit/adapters/rpc-server/services/upsert-social-settings.spec.ts` | 15 |
| `test/unit/logic/blocks.spec.ts` | 3 |
| `test/unit/logic/community-voice-cache.spec.ts` | 18 |
| `test/unit/logic/community-voice-polling.spec.ts` | 8 |
| `test/unit/logic/community-voice.spec.ts` | 88 |
| `test/unit/logic/friends-logic.spec.ts` | 83 |
| `test/unit/logic/friends.spec.ts` | 2 |
| `test/unit/logic/friendships.spec.ts` | 39 |
| `test/unit/logic/notifications.spec.ts` | 6 |
| `test/unit/logic/peers-stats.spec.ts` | 8 |
| `test/unit/logic/profiles.spec.ts` | 16 |
| `test/unit/logic/settings/settings.spec.ts` | 4 |
| `test/unit/logic/settings/settings.utils.spec.ts` | 10 |
| `test/unit/logic/updates.spec.ts` | 94 |
| `test/unit/logic/voice-logic.spec.ts` | 42 |
| `test/unit/logic/ws-pool-component.spec.ts` | 16 |
| total: 50 files | 811 |

Selection rule for `test/unit/logic/` (34 specs at the pin): the specs of the logic the RPC procedures and the
socket run on (friends, blocks, settings, voice, the update fan-out, the connection pool, the online peers, profile
parsing and the friendship notifications), 15 of them. The other 19 are not RPC behaviour:
- the communities HTTP API on the social-api host (`ENDPOINTS.md`): `communities`, `community-bans`,
  `community-compliance-validator`, `community-fields-validator`, `community-image-signature`, `community-members`,
  `community-owners`, `community-places`, `community-posts`, `community-requests`, `community-roles` (the voice
  gates read roles from the database in `src/logic/community-voice/validation.ts`, not through it),
  `community-thumbnails` and `community/utils`;
- SQS consumers and scheduled jobs, not the socket: `community-broadcaster`, `community-events`, `community-ranking`;
- `referral` (HTTP and SQS; an intentional gap) and `user-mutes` (HTTP routes);
- `queries`: it asserts the text of generated SQL, with no observable counterpart.

There is no `test/unit/controllers/handlers/rpc/` at this revision. The cases are grouped below by the behaviour a
client observes.

Status:
- **covered**: a local test asserts it.
- **ported**: a test added for this table asserts it (`rpc::tests::<file>::<test>`).
- **differs**: Catalyst behaves otherwise, including where it has no counterpart. Each names the upstream lines
  (paths in the reference repository, under `src/`) and the local lines (paths under `services/catalyst/`).
- **n/a**: the case checks an implementation detail with no observable counterpart (mock call counts, logging,
  Sentry, metrics, Redis or wiring).

Test files, relative to `services/catalyst/social/src/rpc/` unless a crate is named:
- `tests.rs` (T);
- `tests/socket.rs` (S), `tests/friends.rs` (F), `tests/settings.rs` (ST), `tests/private_voice.rs` (PV),
  `tests/community_voice.rs` (CV), `tests/failures.rs` (FA), `tests/voice_runtime.rs` (VR),
  `tests/connectivity.rs` (CN), `tests/upstream_reads.rs` (UR) and `tests/voice_bugs.rs` (VB, the accepted call
  through the real voice runtime);
- `service/domain.rs`, `service/helpers.rs`, `service/subscriptions.rs`, `limits.rs`, `profiles.rs`, `pubsub.rs`,
  `context.rs` (their `tests` modules);
- `shared/src/rpc/handshake.rs` (HS);
- `services/catalyst/social/src/communities/tests.rs` (CT) and `services/catalyst/social/src/communities/tests/upstream_writes.rs` (CW).

The database tests skip without `CATALYST_TEST_DATABASE_URL`.

Differences that apply throughout:
- **Error messages.** An `internalServerError` carries `"internal error"` (local `social/src/rpc/service/helpers.rs:203-210`)
  where upstream carries the error's message (upstream `src/controllers/handlers/rpc/upsert-friendship.ts:69-71`).
  The pending and sent request lists send an empty one on both sides (upstream
  `src/controllers/handlers/rpc/get-pending-friendship-requests.ts:45-46`; local `social/src/rpc/service/domain.rs:245`).
  A database or voice-runtime error raised with `?` (the private and community voice procedures, for example local
  `social/src/rpc/service/private_voice.rs:57`) is a dcl-rpc remote error, code 500, whose message is exactly
  `Internal Server Error` for every procedure; the detail is logged, never sent (local
  `social/src/rpc/service/helpers.rs:24-32`, `:98-107`; FA `an_internal_refusal_carries_no_database_text`). Upstream
  answers the oneof's `internalServerError` there with the error's message (upstream
  `src/controllers/handlers/rpc/end-private-voice-chat.ts:42-46`): the carrier differs, an open difference, since
  moving to the oneof would change the wire. The community voice procedures answer a runtime failure in the oneof's
  `internalServerError` with upstream's fixed message ("Failed to start community voice chat", …; local
  `social/src/rpc/service/community_voice.rs:436-453`).
- **Profiles.** No procedure answers `profileNotFound` on either side at this revision. With its profile source
  (the active profile entities, as in every deployment), Catalyst leaves out a wallet without a usable profile where
  upstream's registry drops it: a listed friend or request (the total stays the row count), a friendship update or a
  friend connectivity update it sends (upstream `src/logic/friends/friendships.ts:247-268`,
  `src/logic/updates.ts:540`), and an upsert naming it answers `internalServerError` "Profile not found for address …"
  after the write and the update (upstream `src/logic/friends/component.ts:495-508`; local
  `social/src/rpc/profiles.rs:207-260`, `social/src/rpc/service/domain.rs:252-270`,
  `social/src/rpc/service/friends.rs:390-400`, `social/src/rpc/context.rs:152-172`, `:480`; F
  `a_wallet_without_a_profile_is_left_out_with_a_profile_source`). Block and unblock answer an address-only profile
  on both sides. A node without a profile source (no content; the tests' pool-less reader) answers the placeholder
  profile everywhere (local `social/src/rpc/profiles.rs:311-328`).
- **Stream closed.** As upstream (`src/logic/updates.ts:463`, `:476-477`), a second subscription of one event on one
  connection receives the duplicate-subscription `stream_closed` notice and ends; dropping or detaching a stream
  cancels its producer (local `social/src/rpc/service/mod.rs:138-179`, `social/src/rpc/context.rs:205-237`;
  `tests/subscription_lifecycle.rs` `duplicate_rpc_streams_send_the_pinned_final_notice`,
  `detaching_a_connection_cancels_its_idle_stream_and_keeps_its_sibling`).

Production capture: the eight read procedures against `wss://rpc-social-service-ea.decentraland.org`
(2026-10-28 09:53 UTC, `social-rpc/_socket/reads.json`, 10 calls of a wallet created for the run, the status reads
on the zero address and a second address created for the run). The local answers are byte for byte production's
(UR `the_reads_answer_as_production_does`); the capture itself is the ignored `capture` test there, run only with
`CATALYST_SOCIAL_RPC_CAPTURE=1`.

## ws-handler.spec.ts (43)

| behaviour | upstream cases | local test | status |
| --- | --- | --- | --- |
| A valid chain authenticates and SERVER_READY follows | successful authentication | T `ws_handshake_valid_chain_receives_server_ready`; HS `verify_handshake_accepts_valid_chain` | covered |
| Any verification failure closes 3003 "Unauthorized" | authentication failure; middleware rejects the credentials | T `ws_handshake_garbage_or_scene_signed_frame_gets_close_3003`; S `an_expired_chain_closes_unauthorized` | covered, ported |
| A malformed first frame (`""`, not JSON, `null`, number, string, `[]`, numeric timestamp, object chain link) closes 3003 without verification | the `describe.each` of 8 frames | S `a_malformed_first_frame_closes_unauthorized`; HS `verify_handshake_rejects_malformed_envelope` | ported |
| A non-string field outside `x-identity-*` is left to verification | leave it to the middleware | S `a_field_outside_the_identity_headers_does_not_stop_the_handshake` | ported |
| An `x-identity-*` field (any case) that is neither a string nor an array of strings is refused before verification, metadata included (#501); an array reads as its first element, an empty one as absent (`firstOf`, `dist/verify.js:21-23`) | header values must be strings | HS `an_identity_field_that_is_not_a_string_is_refused_up_front`, `an_array_of_strings_reads_as_its_first_element`; S `a_malformed_first_frame_closes_unauthorized`, `an_array_of_strings_authenticates` | ported |
| A chain link present at index 10 is refused as too long, whatever its value | (`verify()` tests the raw value: an empty array is truthy) | HS `a_link_at_the_cap_is_too_long_whatever_its_value` | ported; an empty link before it ends the chain (`if (!raw) break`, `@dcl/crypto-middleware` 6.3.0 `dist/verify.js:71-89`; local `shared/src/rpc/handshake.rs:110-141`) |
| The identity headers are read under their exact lowercase names; a re-cased duplicate is type-checked, then never read | (`verify()` reads `headers['x-identity-timestamp']`, `dist/verify.js:72`, `:321-322`) | HS `a_re_cased_identity_header_is_never_read`; S `a_re_cased_identity_header_is_never_read_on_the_socket` | covered |
| An absent `x-identity-metadata` reads as `{}` and the payload ends `:` | (`createPayload` joins `undefined` as empty) | HS `an_absent_metadata_is_signed_as_nothing`, `an_array_of_strings_reads_as_its_first_element` (an empty metadata array) | ported; a frame without metadata signed over `{}` is refused, as upstream refuses it (`@dcl/crypto-middleware` `dist/verify.js:173`, `:218`; local `shared/src/rpc/handshake.rs:247-259`) |
| The scene signer is refused (re-cased value, re-spelled key); `{}`, `dcl:explorer` and absent metadata pass | the 7 metadata cases | HS `scene_signer_gate_matches_upstream_reject_if_signer`, `a_scene_signed_frame_is_refused_before_the_signature_is_checked` | covered |
| No auth frame within the window closes the socket 3004 "Authorization timeout"; the timer runs through verification | timeout for non-authenticated connections | T `ws_handshake_timeout_closes_connection`; S `the_auth_window_runs_through_verification` | covered, ported (upstream `src/controllers/handlers/uws/ws-handler.ts:371-376`; local `social/src/rpc/ws.rs:33`, `:106-111`, `:146`) |
| A successful auth clears the timeout | clear timeout when authenticated | S `an_authenticated_socket_outlives_the_handshake_timeout` | ported |
| A frame sent while auth is in progress gets `{"error":"Authentication already in progress, please try again later"}` | reject messages during authentication | S `a_frame_during_authentication_is_answered_and_dropped` | ported; the frame is dropped (upstream `src/controllers/handlers/uws/ws-handler.ts:414-421`; local `social/src/rpc/ws.rs:35`, `:130-147`) |
| The catalyst being unreachable during verification closes 3003 | (the 503 describe) | S `an_unreachable_contract_validator_closes_unauthorized` | ported |
| Closing an authenticated socket detaches the user and frees its slot | cleanup authenticated connection | S `closing_an_authenticated_socket_releases_its_connection`; `context.rs` `presence_counts_sockets_per_address` | ported |
| The RPC layer closing the transport ends the socket with 1011 "RPC transport closed" | detach the user and end the socket | `shared/src/rpc/ws.rs` `closing_the_transport_ends_a_live_socket_with_1011` | ported; the first close asked for wins, and a socket whose peer left gets no frame (upstream `src/controllers/handlers/uws/ws-handler.ts:210-229`; local `shared/src/rpc/ws.rs:22`, `:31-62`, `:207-211`) |
| A socket closed during verification is never attached | abort user attachment (3 cases) | S `a_socket_closed_during_verification_is_never_attached` | ported (upstream `src/controllers/handlers/uws/ws-handler.ts:175-181`, `:192-199`; local `social/src/rpc/ws.rs:135-143`) |
| After auth, binary frames reach the RPC server | process the message | T `two_explorers_exchange_a_friend_request_over_the_socket` | covered |
| Upgrade state, pool registration, disconnected flags, drain, emitter throws, send failures, Sentry and metrics | 19 cases | — | n/a |

## rpc-server.spec.ts (15)

| behaviour | upstream cases | local test | status |
| --- | --- | --- | --- |
| Procedures run as the wallet the socket authenticated | attach a user | `context.rs` `presence_counts_sockets_per_address`; T `two_explorers_exchange_a_friend_request_over_the_socket` | covered |
| Two sockets of one address share its events; an unknown detach is harmless | shared subscriber; detach a non-existent user | `pubsub.rs` `publish_is_shared_by_every_receiver`; `context.rs` `presence_counts_sockets_per_address` | covered |
| Different addresses have separate channels | separate subscribers | `pubsub.rs` `each_address_has_its_own_channel` | ported |
| A detached address's channel goes | remove subscriber on detach (2) | `pubsub.rs` `a_channel_without_subscribers_is_forgotten_on_the_next_publish` | covered |
| The private call ends when the address's last socket detaches, not before | 3 cases | `context.rs` `presence_counts_sockets_per_address` (the predicate); PV `a_caller_that_disconnects_ends_the_call_for_the_callee` (the end) | covered, ported |
| Start without creators, service registration, Redis channels, emitter internals | 6 cases | — | n/a |

## Friends, blocks and settings

The current per-method source review is [PARITY.md](PARITY.md), generated from
[parity.json](parity.json). Its thirteen unary friend/block/settings rows name
source scope, regression tests and remaining verification separately. The
registry contains 27 unary methods, six RPC streams and six JSON stream names.
A route registration or a local regression alone does not establish parity.

`source-cases.json` contains 27 responses produced by the actual handlers and
friendship/settings components at the baseline pin with deterministic injected
dependencies. `rpc_wire_answers_match_executed_upstream_source_cases` compares
the local responses byte for byte as Protobuf; the capture builder is
`tooling/social-service-source-cases.cjs`. It covers invalid action/target/self,
missing profiles, synthesized block status, defaults, duplicate settings and
invalid enum branches. It does not exercise a deployed registry, Redis, SNS or
upstream PostgreSQL.

| Contract | Current implementation | Regression evidence |
| --- | --- | --- |
| Friendship action and consent | Shared HTTP/RPC state machine, message code-point bound, pair and actor budgets, blocked-pair gate; empty message is absent metadata. | `rpc/tests/friends.rs`: illegal transitions, rate/message boundaries, cancel/reject/delete and blocked-pair tests |
| Creation and action identity | A mutation response identifies the friendship and keeps its original creation time; request-list rows and request events identify the action UUID. | `friendship_responses_keep_the_original_creation_time_after_later_actions`, `pending_requests_list_the_latest_request_with_its_message`, `a_friendship_runs_through_the_http_routes` |
| Profile failure after mutation | The action commits before RPC profile resolution; either missing profile yields `internalServerError`. HTTP address-only writes do not require profiles. | source golden case `upsert-friendship.missing-profiles-after-commit`; `missing_profiles_commit_friendship_but_fail_rpc_enrichment_and_skip_notifications` |
| Status | The last friendship action wins; a lone block supplies the fallback, matching `src/adapters/friends-db.ts`'s synthetic `block_action`. | `friendship_status_names_a_missing_or_invalid_user_and_reads_a_lone_block`, `friendship_action_outranks_a_block_row` |
| Blocks | Block/unblock plus the existing friendship action commit atomically. Blocking a stranger emits only the block update; retrying an unblock writes and publishes nothing. | `block_and_unblock_roll_back_when_the_friendship_action_cannot_be_recorded`, `blocking_without_friendship_only_emits_a_block_update`, `unblocking_an_unblocked_friend_changes_nothing` |
| Settings | Empty or unknown enums fail before SQL. Absent settings read defaults without insertion; partial writes preserve other fields and new-row column defaults. Privacy enforcement reads the same SQL authority in process. | `settings_reject_empty_and_unknown_wire_values_before_storage`, `settings_read_their_defaults_and_round_trip_every_value`, source golden setting cases |
| Per-address privacy | Lowercase, first-occurrence deduplication, at most 50 inputs before deduplication, active/unblocked friendship. HTTP additionally validates every address. | `private_message_settings_read_per_address_with_friendship`, `settings_run_through_the_http_routes`, source golden duplicate case |

## Private voice

### logic/voice-logic.spec.ts (42) and rpc-server/services: private voice

| spec | behaviour | local test | status |
| --- | --- | --- | --- |
| voice-logic | Calling yourself is refused before any read, in any letter case | T `self_call_is_forbidden_before_any_read`; PV `a_start_follows_the_callers_privacy_and_refuses_yourself` | covered, ported |
| voice-logic | A blocked pair is refused and writes no call | T `blocked_pair_cannot_start_call_and_friends_can`; PV `a_start_follows_the_callers_privacy_and_refuses_yourself` | covered, ported |
| voice-logic | The callee's and the caller's `only_friends` refuse a stranger, admit a friend | T `blocked_pair_cannot_start_call_and_friends_can`; PV `a_start_follows_the_callers_privacy_and_refuses_yourself` | covered, ported |
| voice-logic | Either party busy is a conflict | T `blocked_pair_cannot_start_call_and_friends_can`, `live_call_still_blocks_symmetrically`, `expired_prior_call_does_not_block_new_one`; VB `an_old_accepted_call_keeps_its_parties_busy` | covered |
| voice-logic, start-private-voice-chat | Either party in a voice chat (private or community) is a conflict naming it | PV `either_party_already_in_a_voice_room_is_a_conflict_naming_it` | ported: the community check comes after the self-call, the block and the privacy check, as upstream orders it (`src/logic/voice/voice.ts:61-68`: presence is disclosed only once the call is authorized). Local `social/src/rpc/service/private_voice.rs:44`, `:55`, `:58-60`, `:62-72` |
| voice-logic | The callee is rung with the call id | T `blocked_pair_cannot_start_call_and_friends_can` | covered |
| voice-logic | A competing reservation is a conflict without ringing | comms `voice::db` `pending_calls_serialize_shared_participants_and_return_deleted_rows`; VR `a_lost_reservation_is_busy_and_rings_no_one` | covered, ported |
| voice-logic | Accept: an unknown call is not found; only the callee accepts; each party gets its own grant | PV `an_unknown_call_is_not_found`, `each_party_gets_its_own_grant_and_the_callee_sees_the_incoming_call`; T `blocked_pair_cannot_start_call_and_friends_can` | covered, ported |
| voice-logic | Accept: a credentials failure is an error with no ACCEPTED | VR `a_failed_private_grant_is_an_internal_error_without_accepted` | ported |
| voice-logic | Accept deletes the pending call | VB `an_accepted_call_is_never_swept_as_expired`, `an_old_accepted_call_keeps_its_parties_busy`, `an_old_accepted_call_does_not_arm_the_sweep` | ported; differs: the call row is kept while the call lasts (its participant rows mark it accepted), so the incoming request still returns it; it never expires (the sweep, the start's reclaim and the sweep scheduler skip it) and keeps both parties busy whatever its age. Upstream `src/logic/voice/voice.ts:120-121`; local `comms/src/gatekeeper/voice/db.rs:381`, `:262`, `:278`, `:308`, `social/src/rpc/db.rs:241-242`, `:964-965` |
| voice-logic | Reject: an unknown call or a second reject is not found; the caller hears REJECTED | PV `an_unknown_call_is_not_found`; T `reject_and_disconnect_delete_through_voice_owner_before_signalling` | covered, ported |
| voice-logic | Reject by someone other than the callee is not found | PV `only_the_callee_rejects_and_only_a_party_ends` | ported; an accepted call is not found either, as upstream deletes the pending call on accept. Upstream `src/logic/voice/voice.ts:156-157`; local `social/src/rpc/service/private_voice.rs:164`, `comms/src/gatekeeper/voice/db.rs:332-336` |
| voice-logic | End: an unknown call is not found; the caller ends | PV `an_unknown_call_is_not_found`; T `blocked_pair_cannot_start_call_and_friends_can` | covered, ported |
| voice-logic | End by a non-party is not found; only the other party hears ENDED | PV `only_the_callee_rejects_and_only_a_party_ends`, `updates_name_the_caller_of_a_request_only_and_an_end_tells_the_other_party`; VB `a_stranger_cannot_end_an_accepted_call` | ported. Upstream `src/logic/voice/voice.ts:189-208`; local `social/src/rpc/service/private_voice.rs:216`, `social/src/rpc/context.rs:576-604` |
| voice-logic | End deletes the LiveKit room | VB `ending_an_accepted_call_deletes_its_room`, `a_failed_room_delete_still_removes_the_call` | ported: an accepted call ends through the runtime's end path, which deletes the row while membership is still checkable, then the room. Upstream `src/logic/voice/voice.ts:217`; local `social/src/rpc/service/private_voice.rs:198`, `comms/src/gatekeeper/voice/mod.rs:285-303` |
| voice-logic | End: a runtime failure is an error | VB `a_failed_room_delete_still_removes_the_call`; FA `an_internal_refusal_carries_no_database_text` | differs, kept: the accepted call's row is deleted before its room, so a failed room delete is logged and the answer is ok (answering an error for a call that has ended would invite a retry of nothing); a database failure is the remote error 500 (throughout). Upstream `src/logic/voice/voice.ts:217`, `src/controllers/handlers/rpc/end-private-voice-chat.ts:42-46`; local `comms/src/gatekeeper/voice/mod.rs:300-302`, `social/src/rpc/service/private_voice.rs:194-218` |
| voice-logic | Incoming: none is not found; a pending call names its caller | PV `each_party_gets_its_own_grant_and_the_callee_sees_the_incoming_call` | ported |
| voice-logic | Disconnect: the other party hears ENDED; no call does nothing | T `reject_and_disconnect_delete_through_voice_owner_before_signalling`; PV `a_caller_that_disconnects_ends_the_call_for_the_callee`; VB `ending_an_accepted_call_deletes_its_room` (the room) | covered, ported: the party that dropped is not told. Upstream `src/logic/voice/voice.ts:263-269`; local `social/src/rpc/context.rs:545-574` |
| voice-logic | Expiry pages through batches and tells both parties | T `sweep_reclaims_the_same_rows_the_busy_check_skips`, `sweep_tick_issues_no_delete_until_a_call_is_due`; PV `the_expiry_sweep_pages_through_and_tells_both_parties` | covered, ported |
| voice-logic | Gatekeeper fallbacks and delete races | 7 cases | n/a: one table, atomic deletes |
| start-private-voice-chat (7) | Ok, refused, busy, in a voice chat, unknown error; the messages | T `blocked_pair_cannot_start_call_and_friends_can`, `a_missing_voice_runtime_refuses_the_call_explicitly`; PV `a_refused_start_says_why`, `either_party_already_in_a_voice_room_is_a_conflict_naming_it`; VR `a_lost_reservation_is_busy_and_rings_no_one` | covered, ported: a self-call and a blocked pair answer the not-friends message, as upstream's one `VoiceChatNotAllowedError`. Upstream `src/logic/voice/voice.ts:36-58`, `src/logic/voice/errors.ts:1-5`; local `social/src/rpc/service/private_voice.rs:41-61` |
| accept-, reject-, end-private-voice-chat (10) | Ok with the call id; not found; refused | PV, T (above) | covered, ported; the not-found messages are upstream's sentences naming the id or address, and accepting another's call is upstream's `VoiceChatNotAllowedError` (local `social/src/rpc/service/private_voice.rs:21-28`, `:127-129`) |
| get-incoming-private-voice-chat-requests (3) | Ok, not found, internal | PV `each_party_gets_its_own_grant_and_the_callee_sees_the_incoming_call`; FA | ported |
| subscribe-to-private-voice-chat-updates (8) | Who receives each status | T `blocked_pair_cannot_start_call_and_friends_can`, `reject_and_disconnect_delete_through_voice_owner_before_signalling`; PV `updates_name_the_caller_of_a_request_only_and_an_end_tells_the_other_party` | covered, ported: the update names the caller on REQUESTED only, never the callee. Upstream `src/controllers/handlers/rpc/subscribe-to-private-voice-chat-updates.ts:36-41`; local `social/src/rpc/service/subscriptions.rs:288-313`; stream closed covered (throughout) |

## Community voice

### logic/community-voice.spec.ts (88), community-voice-cache (18), community-voice-polling (8) and rpc-server/services: community voice

| spec | behaviour | local test | status |
| --- | --- | --- | --- |
| community-voice | The owner or a moderator starts; members hear STARTED; a member or non-member cannot | T `community_voice_start_join_and_ban_semantics`; CV `moderators_start_and_end_and_members_cannot` | covered, ported |
| community-voice | The owner or a moderator ends; members hear ENDED; a member cannot | T `a_former_owner_cannot_start_or_end_voice_chat_in_a_deleted_community`; CV `moderators_start_and_end_and_members_cannot` | covered, ported |
| community-voice | Join: a private member joins; a non-member of a private community and a banned wallet cannot | T `community_voice_start_join_and_ban_semantics` | covered |
| community-voice | Join: a public non-member joins | T `community_voice_start_join_and_ban_semantics`; CV `joining_public_and_private_communities_respects_membership_and_bans` | ported: with no role (`none`) and no speaker rights. Upstream `src/logic/community-voice/validation.ts:28-56`; local `social/src/rpc/service/community_voice.rs:105-134` |
| community-voice | Mute: who may mute whom; the refused never reach the runtime; a checksummed target arrives lowercased | `domain.rs` `owner_protection_only_bites_a_non_owner_targeting_the_owner`, `the_moderator_gate_reads_both_role_spellings_and_refuses_members`; CV `who_may_mute_whom` | covered, ported |
| community-voice | Self-mute is free; self-unmute needs the audience | CV `who_may_mute_whom` | ported |
| community-voice | Raising a hand needs the audience; lowering never does | CV `who_may_raise_a_hand` | ported |
| community-voice | The active list hides an unlisted room from non-members | CT `voice_status_derives_from_the_presence_rows` (listed), `an_unlisted_room_is_listed_only_for_members` | covered, ported |
| promote-, demote-, reject-speak-request-, kick- (136) | A moderator or the owner acts on the audience, a moderator never on the owner, a member or guest never; a private target must be a member | T `community_voice_start_join_and_ban_semantics` (kick); CV `who_may_moderate_the_room` | covered, ported |
| promote-, demote- | A banned target is refused | CV `bans_override_retained_roles_without_preventing_revocation`, `ban_and_self_demotion_answers_carry_upstreams_messages`; `domain.rs` `every_gate_reads_the_ban_and_names_the_refusal_as_upstream` | ported: "User {target} is not a member of community {id}" in a public community too; a reject still reaches a banned target. Upstream `src/logic/community-voice/validation.ts:198-212`, `moderation.ts:159-160`; local `social/src/rpc/service/domain.rs:351-363` |
| demote- | Demoting yourself needs no moderator | CV `speakers_can_demote_themselves_without_moderator_authority`, `ban_and_self_demotion_answers_carry_upstreams_messages` | ported: "You have been demoted to listener successfully"; a private community still requires the actor's membership. Upstream `src/logic/community-voice/moderation.ts:73-130`, `src/controllers/handlers/rpc/demote-speaker-in-community-voice-chat.ts:46-57`; local `social/src/rpc/service/community_voice.rs:267-314` |
| start-, end-, promote-, demote-, reject-, kick-, muting another | A banned actor whose role row survived is refused | CV `bans_override_retained_roles_without_preventing_revocation`, `ban_and_self_demotion_answers_carry_upstreams_messages`; `domain.rs` `every_gate_reads_the_ban_and_names_the_refusal_as_upstream` | ported: every gate reads the ban with the roles in one statement, "Banned users cannot {action}". Upstream `src/logic/community-voice/validation.ts:72-155`; local `social/src/rpc/db.rs:1180-1229`, `social/src/rpc/service/domain.rs:285-333` |
| all | An empty community id or target answers `invalidRequest`; a missing community or an unparsable id fails the first step that reads it | CV `identifiers_and_missing_communities_answer_as_upstream` | ported: an empty id "Community ID is required and cannot be empty", an empty target "User address is required and cannot be empty"; a missing community is forbidden (start, end, promote, reject, kick, muting another: the role gates), not found (join, raising a hand, self-unmute: no live room) or an invalid id (demote); an id that is not a uuid is the internal error where upstream's database reads it first and "Invalid Gatekeeper community ID" where its gatekeeper adapter does. Upstream `src/logic/community-voice/moderation.ts:18-26`, `:51-53`, `:86-89`, `validation.ts:41-43`, `src/adapters/comms-gatekeeper.ts:41-46`; local `social/src/rpc/service/community_voice.rs:66-92` |
| start-, end-, join-, raise a hand, self-unmute | No active room answers not found; an active room answers conflict on start | CV `moderators_start_and_end_and_members_cannot`, `who_may_mute_whom`, `who_may_raise_a_hand` | ported: the runtime's room status (a moderator connected or still connecting) is read before start ("Community {id} already has an active voice chat", nothing minted), end, join, raising a hand and self-unmute ("Community voice chat not found for community {id}"). Upstream `src/logic/community-voice/community-voice.ts:116-122`, `:316-321`, `validation.ts:35-43`; local `social/src/rpc/service/community_voice.rs:105-121`, `:189-206`, `:357-371`, `comms/src/gatekeeper/voice/rooms.rs:410-418` |
| start-, join- | A runtime failure is an internal error | VR `a_failed_community_grant_is_an_internal_error` | ported |
| promote-, demote-, reject-, kick-, mute-, request-to-speak- | The gatekeeper's 404 (no room or no such participant) answers `notFound` "Community voice chat or participant not found for community …"; lowering a hand stays ok (#503) | `tests/social_api.rs` `a_community_room_runs_through_the_http_routes` (kick through both transports, raise and lower a hand) | ported: the runtime probes the LiveKit participant first (local `comms/src/gatekeeper/voice/rooms.rs:299`, `social/src/rpc/service/community_voice.rs:31-60`, `:262-265`) |
| promote-, demote-, reject-, kick-, mute-, request-to-speak-, end- | Any other runtime error answers an internal error; end on a missing room answers not found | CV `runtime_failures_are_internal_errors`, `moderators_start_and_end_and_members_cannot`; FA `community_executor_failures_are_reported_and_do_not_announce_an_end` | ported: each procedure's upstream message ("Failed to promote speaker in community voice chat", …); an end the runtime refuses leaves the room live and tells no one; the success answers carry upstream's messages. Upstream `src/controllers/handlers/rpc/*-community-voice-chat.ts`, `src/adapters/comms-gatekeeper.ts:10-14`; local `social/src/rpc/service/community_voice.rs:47-60`, `:436-500` |
| community-voice | A ban landing during a join refuses it and evicts the seat | CV `a_ban_landing_during_a_join_evicts_the_seat` | ported: the ban is read again once the grant is minted. Upstream `src/logic/community-voice/community-voice.ts:282-299`; local `social/src/rpc/service/community_voice.rs:230-245` |
| community-voice | STARTED carries positions, worlds and image, and reaches every subscriber of a public listed community | CV `moderators_start_and_end_and_members_cannot`; CW `a_started_room_carries_the_places_and_the_thumbnail` | ported: the places' positions and worlds and the thumbnail, `isMember` per recipient, every subscriber but the starter for a public listed community, its members otherwise; the notification's `thumbnailUrl` is the thumbnail. Upstream `src/logic/community-voice/community-voice.ts:156-230`, `src/logic/updates.ts:330-433`; local `social/src/rpc/service/community_voice.rs:136-187`, `social/src/rpc/pubsub.rs:205-260`, `social/src/communities/voice.rs:20-53` |
| community-voice | The profile reaches the room metadata | CV `the_profile_reaches_the_room_metadata` | ported: the starter's and a joiner's name, claimed-name flag and picture; none without a profile. Upstream `src/logic/community-voice/community-voice.ts:69-100`, `:126`, `:272`; local `social/src/rpc/service/community_voice.rs:94-103`, `comms/src/gatekeeper/voice/rooms.rs:390-408` |
| community-voice-polling (8) | A room ending out of band reaches subscribers as ENDED | CV `a_room_closed_out_of_band_reaches_its_audience_as_ended`; comms `gatekeeper::tests::a_community_room_lives_while_a_moderator_is_present_and_sweeps_otherwise` | ported: a room the gatekeeper closes (expired, finished or ended through its route) reaches the audience its start recorded as ENDED, once; the gatekeeper's own notification rows stay. Upstream `src/logic/community-voice/community-voice-polling.ts:104-136`; local `comms/src/gatekeeper/voice/mod.rs:925`, `:1076`, `:1208`, `social/src/comms_directory.rs:67-73`, `social/src/rpc/pubsub.rs:262-277` |
| community-voice-cache (18) | The Redis cache | — | n/a: room state is derived from `social.community_voice_chat_users` |
| subscribe-to-community-voice-chat-updates (11) | Updates in order with their fields | `service/helpers.rs` `pubsub_routing_tests::one_publish_reaches_only_the_matching_stream_in_order` | covered (routing); stream closed covered (throughout) |

## The update fan-out and the logic behind every answer

### logic/updates.spec.ts (94)

| behaviour | upstream cases | local test | status |
| --- | --- | --- | --- |
| A friendship update reaches the other party; with no subscriber it goes nowhere | 3 | T `two_explorers_exchange_a_friend_request_over_the_socket`; F `cancel_reject_and_delete_reach_the_other_party`; `pubsub.rs` `a_channel_without_subscribers_is_forgotten_on_the_next_publish` | covered; the unparseable-message case n/a (events are typed and in process) |
| Accepting a friendship tells each party's open connectivity stream that the other is ONLINE, when online | 7 | F `accept_announces_only_current_online_friends_without_topology_change` | ported. Upstream `src/logic/updates.ts:108-139`; local `social/src/rpc/service/friends.rs:89-120` |
| A friend's connectivity reaches each online friend; none when no friend is online | 4 | T `two_explorers_exchange_a_friend_request_over_the_socket`, `connectivity_friend_pages_and_delivery_predicate_revoke_blocks_in_both_directions`; `topology.rs` `bounded_page_progress_survives_retry_rounds_without_restarting_first_page` | covered; differs: a failed recipient read is retried from its cursor, where upstream logs and drops the update. Upstream `src/logic/updates.ts:145-158`; local `social/src/rpc/topology.rs:40-42` |
| A member's status reaches its online co-members in each of its communities, in batches | 6 | CN `a_members_status_reaches_its_co_members_in_each_community`; T `connectivity_replay_members_pages_exclude_self_and_cover_multiple_communities` | covered, ported; differs: a failed read is retried (as above). Upstream `src/logic/updates.ts:165-183`; local `social/src/rpc/context.rs:505-537` |
| A join, leave, kick or ban reaches the community's online members and the affected member | 7 | CW `membership_changes_and_deletion_reach_the_open_member_streams`; CN `a_membership_change_reaches_the_departed_wallet_and_the_members` | ported: a join (directly or by an accepted request) ONLINE, a leave, kick or ban (of a non-member too) OFFLINE, delivered as published, with no co-member or presence check. Upstream `src/logic/community/members.ts:270`, `:330`, `:362`, `requests.ts:177`, `:278`, `bans.ts:110`, `src/logic/updates.ts:254-300`; local `social/src/rpc/pubsub.rs:124-174`, `social/src/rpc/service/subscriptions.rs:254-264`, `social/src/communities/mod.rs:322-347`, `social/src/communities/members.rs:581`, `:652`, `:904`, `social/src/communities/requests.rs:402`, `:624` |
| A block update reaches the blocked wallet | 3 | F `blocking_a_friend_ends_the_friendship_and_tells_them`, `unblocking_a_stranger_only_lifts_the_block` | covered |
| Private voice: REQUESTED to the callee, ACCEPTED and REJECTED to the caller, ENDED and EXPIRED to both | 14 | T `blocked_pair_cannot_start_call_and_friends_can`, `reject_and_disconnect_delete_through_voice_owner_before_signalling`; PV `each_party_gets_its_own_grant_and_the_callee_sees_the_incoming_call`, `a_caller_that_disconnects_ends_the_call_for_the_callee`, `the_expiry_sweep_pages_through_and_tells_both_parties`, `updates_name_the_caller_of_a_request_only_and_an_end_tells_the_other_party` | covered, ported: an ENDED from a party, or its disconnect, reaches the other party only; missing addresses, an unknown status and logging n/a. Upstream `src/logic/updates.ts:199-245`; local `social/src/rpc/context.rs:576-604` |
| Community voice STARTED: every subscriber for a public listed community, members otherwise, never the starter | 10 | CV `moderators_start_and_end_and_members_cannot`; T `community_voice_start_join_and_ban_semantics` | covered, ported. Upstream `src/logic/updates.ts:330-433`; local `social/src/rpc/pubsub.rs:205-260` |
| Community voice ENDED reaches the audience its start had; a failed member read still announces a public listed start | 11 | CV `moderators_start_and_end_and_members_cannot`, `a_room_closed_out_of_band_reaches_its_audience_as_ended` | covered, ported: the audience is recorded at start and reused by the end and the closed-room report; an ENDED for everyone reads no membership (every `isMember` false); without a recorded start it reaches the members; a failed member read drops a member-only update and announces an everyone update without membership. Upstream `src/logic/updates.ts:330-433`; local `social/src/rpc/pubsub.rs:200-277`, `social/src/rpc/service/community_voice.rs:372-395` |
| Community voice: a missing community at start, logging | 3 | T `a_former_owner_cannot_start_or_end_voice_chat_in_a_deleted_community` | covered (the refusal); logging n/a |
| A second subscription of one stream on one connection ends with the stream-closed notice | 6 | `tests/subscription_lifecycle.rs` `duplicate_rpc_streams_send_the_pinned_final_notice` | covered (throughout); its metric and log cases n/a |
| One address on two connections: both receive | 2 | `pubsub.rs` `publish_is_shared_by_every_receiver` | covered |
| The initial snapshot comes before live updates: an update published after subscribing follows the replay; a failed snapshot continues live | 3 | T `two_explorers_exchange_a_friend_request_over_the_socket`; CN `a_live_update_published_after_subscribing_follows_the_snapshot`; `subscriptions.rs` `replay_tests` | covered, ported; differs: a failed snapshot ends the stream, as recorded under the friend-connectivity row |
| A stream without a connection id or emitter | 2 | — | n/a: an unauthenticated transport is refused before any stream (`social/src/rpc/service/mod.rs:56-61`) |
| The live loop yields the parsed updates its filter accepts, and skips one whose profile is not found | 10 | `service/helpers.rs` `pubsub_routing_tests::one_publish_reaches_only_the_matching_stream_in_order`; F `a_wallet_without_a_profile_is_left_out_with_a_profile_source` | covered, ported: with a profile source a friendship or friend connectivity update whose sender has no profile is not published (see Profiles, throughout); registration and error-path cases n/a. Upstream `src/logic/updates.ts:530-550`; local `social/src/rpc/context.rs:152-172`, `:480` |
| Deleting a community tells its online members it went OFFLINE | 3 | CW `membership_changes_and_deletion_reach_the_open_member_streams` | ported: each member on an open stream hears itself OFFLINE in it. Upstream `src/logic/community/communities.ts:164`, `src/logic/updates.ts:302-328`; local `social/src/rpc/pubsub.rs:176-198`, `social/src/communities/communities.rs:1031` |

### logic/ws-pool-component.spec.ts (16)

| behaviour | upstream cases | local test | status |
| --- | --- | --- | --- |
| Each open socket counts; closing one frees its slot | 4 | S `closing_an_authenticated_socket_releases_its_connection`, `each_socket_counts_and_closing_one_frees_its_slot`, `an_authenticated_socket_outlives_the_handshake_timeout` | covered, ported |
| A socket counts from the moment it opens | 1 | S `a_socket_counts_from_the_moment_it_opens` | ported; every socket of both protocols (upstream `src/controllers/handlers/uws/ws-handler.ts:363`; local `social/src/rpc/context.rs:193-211`, `social/src/rpc/ws.rs:70`, `social/src/rpc/stream.rs:244`) |
| A full pool refuses a new socket with 1013 "Unable to acquire connection"; a freed slot is taken again | 3 | S `a_full_pool_closes_a_new_socket_until_a_slot_frees` | ported; the check and the count happen together under one lock (upstream `src/logic/ws-pool/component.ts:26-29`, `src/controllers/handlers/uws/ws-handler.ts:407`; local `social/src/rpc/ws.rs:31`, `:70-72`, `social/src/rpc/context.rs:193-211`) |
| Shutdown ends each socket with 1001 "Server shutting down" | 3 | S `shutdown_ends_every_socket_with_1001` | ported; the server job waits up to 2s for them (upstream `src/logic/ws-pool/component.ts:59-72`; local `social/src/rpc/mod.rs:195-207`, `:229-231`, `social/src/rpc/context.rs:217-227`) |
| Duration metrics, unknown connections, the rejection metric, connection ids | 5 | — | n/a (the `ws_*_by_protocol` series are emitted, local `social/src/rpc/context.rs:202`, `comms/src/metrics.rs:6-18`) |

### logic/peers-stats.spec.ts (8)

| behaviour | upstream cases | local test | status |
| --- | --- | --- | --- |
| The online peers are the deduplicated union of every source; none from empty sources | 3 | comms `topology.rs` `complete_replacement_invalid_retention_and_quiet_expiry` | covered; differs: one source (the Pulse island frames), not archipelago and worlds stats, and every address is lowercased with anything that is not a wallet dropped. Upstream `src/logic/peers-stats/peers-stats.ts:12-14`; local `comms/src/topology.rs:56-62` |
| A failing source is skipped; all failing gives none | 4 | comms `topology.rs` `complete_replacement_invalid_retention_and_quiet_expiry` | covered; differs: a frame that does not decode keeps the previous set until its receipt expires, and an expired receipt empties it. Upstream `src/logic/peers-stats/peers-stats.ts:12-14`; local `comms/src/topology.rs:55`, `:89-97` |
| Every source is asked | 1 | — | n/a |

### RPC profiles and friendship notifications

RPC profiles use the first avatar's required `userId`, name (falling back to
`unclaimedName` even for an empty name) and required stored `face256`. The
response lowercases the identity and preserves the snapshot URL. Friend,
mutual-friend, request and blocked-user lists omit unavailable profiles while
retaining the domain row count. Block/unblock write responses retain an address
with empty display fields when enrichment is unavailable. Positive profiles are
cached for ten minutes; missing profiles are retried rather than negatively
cached. Resolver failure is kept distinct from a successful empty lookup.
`rpc/profiles.rs` tests `first_avatar_requires_identity_name_and_face_and_preserves_snapshot_url`
and `absent_profiles_are_omitted_from_lists_and_block_responses_keep_the_address`
cover those wire choices; the source golden replay covers absent-profile lists.

Requests and accepts create inbox/outbox notifications when both profiles exist;
cancel, reject and delete do not. Notification IDs use the committed action UUID
and metadata preserves sender, receiver, picture, name and optional message
(`friendship_notifications_use_action_ids_and_preserve_profile_metadata`). Local
notification persistence replaces this producer's SNS hop; it does not establish
compatibility for external SNS consumers or prove upstream retry behaviour.

## HTTP twins and `/v1/stream` (b6b2cef)

Upstream #504-#507 serve the procedures over HTTP on the social-api host, and #503 serves the six streams over a
plain JSON WebSocket. Locally each route calls the operation its procedure calls (`social/src/rpc/service/mod.rs`
`ops`: `friends.rs:36` `apply_friendship_action`, `:181` `friendship_status`, `blocks.rs:17` `block`, `:60`
`unblock`, `settings.rs:14`, `private_voice.rs:30-218`, `community_voice.rs:189-425`) and the same `Db` reads; the
routes are `social/src/rpc/http.rs` (`ENDPOINTS.md` lists them), the stream `social/src/rpc/stream.rs`.

| upstream | behaviour | local test | status |
| --- | --- | --- | --- |
| #504 friends | `/v2/friends`, `/v2/users/{address}/mutual-friends`, `/v1/users/{address}/friendship-status`, `/v2/friendship-requests`, `POST /v1/friendship-requests`, `PATCH /v1/friendship-requests/{address}`, `DELETE /v1/friends/{address}`: addresses only; 403 blocked, 409 invalid transition, 429 rate limited | `tests/social_api.rs` `a_friendship_runs_through_the_http_routes`, `friendship_routes_refuse_as_upstream_does`, `a_route_and_its_procedure_write_and_publish_alike` | ported; the procedure's invalid-transition message is now upstream's `InvalidFriendshipActionError` (local `social/src/rpc/service/friends.rs:22`) |
| #505 blocks | `POST /v1/blocks`, `DELETE /v1/blocks/{address}` (idempotent), `/v1/blocking-status`, `/v2/blocks` | `tests/social_api.rs` `blocks_run_through_the_http_routes` | Local regression coverage; block events follow the existence of a friendship, and paired writes are atomic. See the per-operation backlog for remaining verification. |
| #506 settings | `GET`/`PATCH /v1/social-settings` (the stored strings), `/v1/private-messages-settings` (at most 50, each validated, deduplicated) | `tests/social_api.rs` `settings_run_through_the_http_routes` | ported |
| #507 voice | private calls: start, incoming, accept/reject, end; community rooms: start, end, join, kick, promote, mute, demote, raise and lower or reject a hand | `tests/social_api.rs` `a_private_call_runs_through_the_http_routes`, `a_community_room_runs_through_the_http_routes` | ported; the community gates' messages are the procedures' (the rows above) |
| #503 stream handshake | Upgrade headers verified before the upgrade (401, 503) or the first frame (`auth_in_progress`, 3003, 3004), signed over `/v1/stream`, never `/` | `tests/stream.rs` `both_handshakes_are_over_the_stream_route_only`, `no_chain_within_the_window_closes_3004`; `tests/socket.rs` `mounted_socket_paths_and_proxy_headers_match_actual_source_verifiers` | ported; served at `/social-rpc/v1/stream` (the RPC socket's `/` is `/social-rpc`), local `social/src/rpc/mod.rs:283-286`, `social/src/rpc/stream.rs:162`, `:187`, `:213`, `:495` |
| #503 stream messages | `subscribe`/`subscribed`, `unsubscribe`/`end`, `ping`/`pong`, `event`; the error codes and messages of `messages.ts` | `tests/stream.rs` `a_subscription_receives_the_update_its_rpc_stream_would`; `stream.rs` `messages_parse_as_upstream_validates_them` | ported (local `social/src/rpc/stream.rs:614`, `:642`, `:771`) |
| #503 per-address cap | `WS_MAX_CONNECTIONS_PER_ADDRESS` 8 across both protocols; the ninth closes the oldest `3005` | `tests/stream.rs` `the_ninth_socket_of_an_address_ends_its_oldest`; `limits.rs` `the_ninth_socket_displaces_the_oldest` | ported, an RPC socket included (local `social/src/rpc/limits.rs:98`, `:184-208`); the cap is claimed after auth |
| #503 subscription bucket | 2/s, burst 24 per address, surviving reconnects, 10,000 idle buckets; `rate_limited` | `limits.rs` `the_subscription_bucket_survives_a_reconnect` | ported (local `social/src/rpc/limits.rs:103-128`, `:212`) |
| #503 limits | `STREAM_WS_*`, `WS_*` configuration | config `socket_limits_read_upstreams_variables`; `limits.rs` `configured_limits_apply_and_are_counted` | ported for `WS_MAX_BACKPRESSURE`, `WS_MAX_CONNECTIONS_PER_ADDRESS`, `WS_SUBSCRIPTIONS_PER_SECOND`, `WS_SUBSCRIPTION_BURST`, `WS_ADDRESS_BUDGET_CAPACITY` and the three `STREAM_WS_*`, with upstream's defaults; a value that is not positive is refused (local `config/src/social.rs:85-117`, `src/serve.rs:1986-1988`); differs: `WS_IDLE_TIMEOUT_IN_SECONDS` is not read, and one `AUTH_WINDOW_SECS` stands for `WS_AUTH_TIMEOUT_IN_MS` and `WS_AUTH_SIGNATURE_EXPIRATION_IN_MS` |
| #503 backpressure | Writes pause while uWS buffers; a dropped write closes 1008 `Slow consumer`; 1,000 events per subscription queue | `tests/stream.rs` `a_send_past_the_backpressure_limit_closes_slow_consumer`, `a_full_subscription_queue_closes_subscription_overflow` | ported (local `social/src/rpc/stream.rs:311-356`, `:421-437`, `:827-868`); differs: the waiting bytes are the writer task's queue, not uWS's buffered amount, so a very small `WS_MAX_BACKPRESSURE` can trip on scheduling delay; and a lagging address channel (256 events of any stream, `pubsub::CHANNEL_CAP`) also closes 1008 `Subscription overflow; reconnect and resync` (local `social/src/rpc/stream.rs:846-850`) |
| #503 metrics | `stream_ws_*`, `ws_*_by_protocol`, `ws_admission_*` | `limits.rs` `configured_limits_apply_and_are_counted` (the admission series) | ported; emitted through the `metrics` facade into serve's Prometheus exporter, the others without a test (local `social/src/rpc/context.rs:202`, `social/src/rpc/limits.rs:202`, `social/src/rpc/stream.rs:203`, `:257`, `:441`, `comms/src/metrics.rs:6-18`) |
| #503 friend connectivity | One `online` per friend online, then live updates under the RPC stream's delivery guard | — | ported without a dedicated test (local `social/src/rpc/stream.rs:870-872`, `:989-1014`) |
