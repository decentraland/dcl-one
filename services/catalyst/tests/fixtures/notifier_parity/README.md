# Events-notifier parity fixtures

What the events-notifier's Segment destination webhooks answer, per request,
derived from their pinned source `github.com/decentraland/events-notifier@1550a6e8`
(`src/controllers/handlers/forward-explorer-events.ts`,
`forward-web-events.ts`, `forward-onboarding-checkpoint.ts`,
`src/logic/event-parser.ts`, `src/utils.ts`, their `*.spec.ts`). The harness
`tests/notifier_parity.rs` signs every entry's body as Segment signs it
(`x-signature`: HMAC-SHA1 of the body under `fixture-signing-key`), sends it to
`social::segment_webhooks::router` nested at `/events-notifier` behind the
social writes switch, and compares the answer and the effect: the
`social.client_events` row and the onboarding checkpoint. The bodies under
`bodies/` are the notifier's own fixtures (`test/fixtures`, IPs masked as the
notifier masks them); nothing comes from a capture.

## Running

```sh
cd services/catalyst
CATALYST_TEST_DATABASE_URL=postgresql://user:pass@localhost:5432/postgres \
  cargo test -p catalyst --test notifier_parity -- --nocapture
```

Without `CATALYST_TEST_DATABASE_URL` the test skips. Every entry runs with the
table truncated and a fresh onboarding state (plus `harness.seed_checkpoint`),
so entries are independent. The summary line counts the verdicts; the test
prints one line per entry: `PASS`, `FAIL` (fails, not listed), `GAP` (listed and
failing as recorded), `CHANGED` (listed, failing differently), `FIXED` (listed
and passing), `BAD` (a defect in the fixture itself: unknown `harness` value, a
seed the store refuses, an identity without a context, a `source.kind` that is
neither `source` nor an accepted entry's `accepted`; never a gap).

`known_gaps.json` is the ratchet: the fixture ids the router does not answer
like the pinned source, each with the observed difference (the fresh wallet
masked as `<identity>`). The test fails on an entry that fails and is not
listed, on a listed entry that passes (remove it), and on a listed entry whose
observed difference changed (check it did not get worse, then refresh). Ids
are never added by hand:

- `NOTIFIER_PARITY_WRITE_GAPS=1` rewrites the list from the run: it drops the
  ids that pass and refreshes the observed differences; it never adds an id.
- `NOTIFIER_PARITY_WRITE_GAPS=1 NOTIFIER_PARITY_ALLOW_NEW_GAPS=1` also adds the
  unlisted failures and prints `added <id>: <observed>` for each; every added
  id is recorded under "Known gaps" with the pinned source lines it diverges
  from and the answer the router gives.

The flags compare to the value `1`.

### Known gaps

No entry is listed. A listed entry would carry the pinned source lines it
diverges from and the answer the router gives, as the module doc of
`social/src/segment_webhooks/mod.rs` carries the contract's differences.

### Accepted entries

Five entries record dcl-one's accepted answer rather than the notifier's
(`ACCEPTED_ENTRIES` in the harness, `source.kind: "accepted"`). They are
checked like every other entry but are excluded from the ratchet: they can
never appear in `known_gaps.json`, so a failure there is a regression, not a
gap. Every other entry has `source.kind: "source"`.

- `forward.expired_chain`: the notifier's `validateAuthChain`
  (`forward-explorer-events.ts:24-31`) checks the chain's shape and owner
  only, so the fixture body's own chain, whose ephemeral delegation expired on
  2025-05-26, is accepted and published. dcl-one verifies the signatures and
  the expiry (`dcl_auth::verify_auth_chain`, `explorer.rs`
  `auth_chain_is_valid`) and answers 401 `Invalid AuthChain`, no row: an
  unverified chain proves no owner. Every other explorer entry signs a fresh
  chain (`harness.identity: fresh`).
- `forward.no_received_at` and `forward.no_message_id`: the parser reads
  `receivedAt`/`timestamp` with `new Date(...).getTime()` and `messageId` as
  is (`event-parser.ts:308-309,352`), so an event without them is published
  with `NaN` timestamps or an undefined key and answered 200, the delay
  metrics skipped (`forward-explorer-events.ts:226-231`). dcl-one stores the
  event as a row keyed by `messageId` with both times
  (`social.client_events`) and answers 400 `Invalid event`.
- `forward-web.no_traits`: `event.traits.ethAddress` throws outside any try
  (`event-parser.ts:462`), the framework's 500; the unit spec
  (`forward-web-events.spec.ts:149-172`) mocks the parser and expects 200
  `validationPassed: 0`, which dcl-one answers, no row.
- `forward-web.no_page`: `event.context.page.path` throws the same way
  (`event-parser.ts:316-330`), a 500; dcl-one answers 400
  `Invalid web event`.

## Files

| File | Content |
|---|---|
| `forwarders.fixture.json` | the three routes: signature refusals, the explorer pipeline (checkpoint side effects, parse, context validation, denylist, auth chain, duplicate delivery), the web identify pipeline, the checkpoint lift, the writes switch and the unset key |
| `bodies/*.json` | the notifier's fixture bodies: `logged_in`, `logged_in_cached`, `move_to_parcel`, `web_event`, `web_event_invalid`, `checkpoint` (the checkpoint spec's valid body) |
| `known_gaps.json` | the ratchet |

## Entry schema

```json
{
  "id": "forward.move_to_parcel",
  "request": {"path": "/forward", "body": "$move_to_parcel", "patch": {"context.realm": "worlds", "properties": "$delete"}},
  "harness": {"sign": "valid", "identity": "fresh", "body_form": "compact", "force_context_validation": false,
              "denylist": [], "expected_realm": null, "expected_realm_url": null, "writes_enabled": true,
              "signing_key": true, "repeat": 1, "seed_checkpoint": null},
  "expected": {"status": 200, "body": {"ok": true}},
  "effect": {"rows": 1, "row": {"sub_type": "move-to-parcel", "address": "$identity"},
             "checkpoint": {"user": "$identity", "checkpoint": 7, "recorded": false}},
  "source": {"kind": "source", "refs": ["event-parser.ts:348-364"], "note": "why"}
}
```

- `request.body`: a JSON value, or `$name` for `bodies/<name>.json`;
  `request.patch` sets dotted paths before signing (`$delete` removes one).
- `harness.sign`: `valid` (default), `wrong` (signed under another key) or
  `none` (no header). `body_form: pretty` sends the body re-spaced and signs
  its compact form, as Segment signs `JSON.stringify(body)` of what it parsed.
- `harness.identity`: `fresh` signs a new owner's `SIGNER, ECDSA_EPHEMERAL`
  chain now and writes the owner into `context.dcl_eth_address`,
  `context.auth_chain` and `userId`; `foreign` sends that chain under another
  wallet's address. `$identity` in any expected string is the owner.
- `harness.seed_checkpoint`: a checkpoint body recorded before the request
  (`POST /onboarding/checkpoint`'s shape); `completed` on a checkpoint never
  reached is a no-op in the store, so a `completed` expectation seeds `reached`.
- `expected.body` compares whole; `expected.body_subset` compares the listed
  members. `effect.rows` is the table's count after the request; `effect.row`
  is a subset of the stored row (`key`, `source`, `sub_type`, `address`,
  `session_id`, `anonymous_id`, `realm`, `ip`, `metadata`, the metadata as a
  subset too); `effect.checkpoint` names `user` and `checkpoint` and either
  `recorded: false` or a subset of the state (`completed`, `email`, `wallet`,
  `metadata`).
- `source.kind`: `source` (the expectation is the pinned notifier's answer)
  or `accepted` (dcl-one's answer, one of the accepted entries above);
  `source.refs`: the pinned notifier lines (file:lines) the expectation reads
  from; `source.note` says why when the lines alone do not.
