# Running this crate's tests

`cargo test -p dcl-scene-cli` must pass with only this checkout, a toolchain and `node` (not
ignorable: `golden` fails without it). Other dependencies use
`#[ignore = "needs <VAR>: <why>; see docs/testing.md"]`; `tests/testgate_contract.rs` rejects a bare
`#[ignore]` and gating variables missing from this page.

## Gated tests

```
DCL_SCENE_CLI_TEST_NODE_MODULES=/path/to/scene/node_modules \
DCL_SCENE_CLI_TEST_SCENE=/path/to/scene \
  cargo test -p dcl-scene-cli -- --include-ignored
```

Under `--include-ignored` a missing variable fails naming it (`tests/common/testgate.rs`).

| Variable | Unlocks | Point it at |
| --- | --- | --- |
| `DCL_SCENE_CLI_TEST_NODE_MODULES` | `data_layer_rpc`, `initial_build_recovery`, `workspace_e2e` | a scene `node_modules` with `@dcl/sdk`, `@dcl/rpc`, `@dcl/inspector`, `ws`, on the same filesystem as `$TMPDIR` |
| `DCL_SCENE_CLI_TEST_SCENE` | `init_scaffold` provisioned build, `error_contract` `t2`/`t6` | an installed scene checkout (`node_modules/@dcl/sdk` exists) |
| `DCL1_TUNNEL_PUBLIC_URL` | `tunnel_live` | a deployed preview tunnel's public URL |
| `DCL1_TUNNEL_LOCAL_URL`, `DCL1_TUNNEL_TOUCH_FILE` | optional for `tunnel_live` | the local preview behind it; a served file to touch |
| `LIVEKIT_URL`, `LIVEKIT_API_KEY`, `LIVEKIT_API_SECRET`, `LIVEKIT_HTTP_URL` (optional) | `livekit_live` | a LiveKit server (`livekit-server --dev`: `ws://127.0.0.1:7880`, `devkey`, `secret`) |

`livekit_embedded` fails without a `livekit-server` unless `ALLOW_SKIPPED_INTEGRATION=1`.

## Escape hatch

| Variable | Effect |
| --- | --- |
| `ALLOW_SKIPPED_INTEGRATION=1` | a missing dependency returns `ok` and prints `SKIPPED <test>: ...` to fd 2; read those lines |
| `CATALYRST_TESTGATE_SKIPLOG` | file that receives one `test<TAB>requirement<TAB>detail` line per skip |

## Other knobs

| Variable | Effect |
| --- | --- |
| `UPDATE_GOLDEN=1` | rewrite `testdata/golden/*.golden`: `UPDATE_GOLDEN=1 cargo test -p dcl-scene-cli --test golden` |
| `DCL_ONE_SCHEMA_FUZZ_SEED` | replay one `schema_parity` fuzz seed |

Spawn the CLI through `error_contract.rs` `run()` (`CHILD_TIMEOUT`); never wait on a product timeout
such as `DCL_SCENE_CLI_LINKER_TIMEOUT_SECS` (default 600).

## Browser journeys in CI

`Scene CLI journeys` builds the PR base and proposed CLI and runs the same 45
stages against both: launch targets, scene editor, storage scopes and transfers,
publishing targets, wallet handoff, missing integrations, offline feedback and
tablet layouts (768 × 1024). Local HTTP fixtures replace upstream services. Runtime credentials
are removed from child processes; no wallet signatures or live uploads occur.

```sh
node tooling/scene-cli-journeys.mjs --binary services/catalyst/target/debug/dcl-scene-cli \
  --chrome chromium --label head --revision HEAD --output .cache/scene-cli-journeys/head
python3 tooling/scene-cli-journey-report.py render .cache/scene-cli-journeys
```

Run from the repository root with Node 24+ and Chromium/Chrome. Add a second run
with `--label base` and `--output .cache/scene-cli-journeys/base` for comparisons.
Ports 18765–18766 must be free. Each run cleans its temporary processes and scene.
Artifacts retain partial screenshots and errors when a stage fails. Changed
screenshots request review; they do not fail a pixel baseline. Current visual
defects are observable without tests requiring them to persist.

The 30-day `scene-cli-journeys` artifact contains PNGs, manifests, logs and
`report.html`. A trusted publisher updates one bot PR comment with changed journeys
in collapsed `<details>` sections and direct GitHub image attachments. It creates
no releases or tags. With no changes or failures it posts no initial comment;
an existing comment is updated to clear stale results. Comparisons mask carets,
clock labels, request counts and minted entity IDs; uploaded screenshots are
unmodified. Live wallets, remote deployment,
World mutations, voice and external clients/editors remain unverified boundaries.
Capture and publishing are separate jobs in `.github/workflows/scene-cli-journeys.yml`.
Automatic comments apply to same-repository PRs after the publisher script is on
the default branch. Fork and Dependabot PRs retain artifacts; a maintainer can
dispatch the same workflow with its `run_id` input to publish a completed run.
The first PR can bootstrap publication locally with:

```sh
python3 tooling/scene-cli-journey-report.py publish --repo OWNER/REPO --run-id RUN_ID
```

This needs `GH_TOKEN` with Actions read, contents read and PR write permissions,
and a token accepted by GitHub's attachment endpoint for inline images.
If the endpoint refuses the token, the comment links to the artifact instead;
no extra secret or storage service is required.
See the [journey CI ADR](../../../docs/adr/2026-10-30-scene-cli-journey-ci.md) for the
publication boundary.
