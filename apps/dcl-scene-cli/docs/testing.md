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

`livekit_embedded` fails without a `livekit-server` unless `ALLOW_SKIPPED_INTEGRATION=1`. A new test
that runs a script under `scripts/` must add it to the export assembler's `KEEP_RE`.

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
