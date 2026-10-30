# Security

## The model

Scene code is untrusted. The JS op surface is not a security boundary and cannot be made one:
JS is too leaky. An allowlist in the sandbox does not stop a scene reaching an op through the
deno object or by reloading the wasm bundle. All security is memory protection:

- Scene code never has access to engine memory.
- The signing key never lives in a process that runs scene code.
- Privileged operations exist only as messages on the `SystemApi` channel, and the engine
  decides who holds a sender for it.
- The engine stamps which scene sent each message; a scene's identity is never read from the
  message itself.

How this lands on each platform:

| | Native | Web |
|---|---|---|
| Scene isolation | V8 isolates inside the `dcl_deno_ipc` sidecar process, one process for all scenes | One sandbox worker per scene, each with its own wasm memory |
| Engine boundary | Process boundary; the sidecar talks to the engine over a pipe | The `dcl_wasm` relay runs in the engine's memory and copies messages wasm to wasm |
| Signing key | Engine process (`wallet`) | The shell page, a separate process from the app page; the engine asks it for each signature through `wallet::remote_signer` |
| SystemApi sender | Given to the scene the engine marks `is_super`; the engine forwards what the sidecar sends | Held by the engine-side relay for the one scene the engine marked super; any other scene that sends a `SystemApi` message is ended |

The native sidecar is one trust domain: the engine trusts it as a whole and relies on V8
isolation between scenes inside it.

## Rules

**Privilege is granted in Rust, never checked in JS.** Anything a scene must not be able to do
by default goes over `SystemApi`, not `RpcCalls` (every scene has that queue). Do not add
allowlists, op filters or "is this the system scene" checks in `sandbox_worker.js` or any other
JS; they are not a guard and future code may come to rely on them. On web the relay already
validates the caller (`crates/dcl_wasm/src/inner/relay.rs`, the `ToEngine::SystemApi` arm). If
caller validation ever needs extending, extend the relay. When reviewing a new scene capability
ask: what stops this in Rust if the scene reaches the op directly?

**The signing key stays in a process that runs no scene code, and signing stays in the
engine.** On native the wallet lives in the engine process and never goes to the sidecar. On web
it lives in the shell page, and the engine asks the shell for each signature. A scene that needs
a signed request supplies method, URL and body only; the engine builds the signed metadata
(realm, profile, scene) from its own state. Do not add a path that hands a scene, the sidecar, or
the app page anything derived from the key, and do not let scene-supplied metadata into a
signature.

**On web, the HUD never touches `localStorage`.** The app page and the sandbox workers share an
origin with the shell page that holds the key. The first `localStorage` read loads the origin's
whole storage area into the app process, where scene code runs. `app.html` throws on any access
in shell mode; HUD preferences go through `src/lib/prefs.ts` (IndexedDB). Only the shell
(`src/shell/main.ts` and the sign-in code it owns) may read it. The same reasoning applies to
anything secret: it must not be in the app process at all.

**On native, never `Path::join` or `PathBuf::push` with a component from outside.** Content
hashes, content-map keys, entity ids, scene file names, URLs, RPC arguments: anything that
arrived from a content server or a scene. `join` replaces the base when the right side is
absolute, no `..` needed, and `Path::is_absolute` is not a sufficient guard on Windows (`C:x`
and `\x` both discard the base). Use `JoinRelativeExt::join_relative` from `common::util`,
which returns `None` for prefix, root and parent components. Asset paths are contained by
`content_file_path` and `resolve_content_uri` in `crates/ipfs/src/ipfs_path.rs`; bevy's own
`UnapprovedPathMode` is set to `Allow` deliberately, because its check only admits paths under
the executable's assets directory and the ipfs cache lives elsewhere.

**Always ask whether new functionality should exist in headless mode.** The headless build is
the authoritative scene server. It runs scene code with no user present, and it is never a
system scene. A capability the server can never legitimately use (opening a URL, writing the
clipboard, anything that touches the host OS or a page) must be excluded from the headless build
at code level, not merely denied by the permissions table: a config-driven deny is one edit away
from being an allow. The scene promise still gets answered, with a denial.

## Reporting

Do not disclose a security problem in a public issue or PR. Use the private reporting
channel in Decentraland's [published security policy](https://github.com/decentraland/governance/blob/master/SECURITY.md),
including the affected revision, impact and reproduction details. Coordinate publication
of the fix and any reproduction with the maintainers; keep private findings out of public
PR descriptions until disclosure is agreed.
