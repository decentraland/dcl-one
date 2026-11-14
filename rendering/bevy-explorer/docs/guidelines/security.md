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
in shell mode; HUD preferences go through `library/explorer-core/lib/prefs.ts` (IndexedDB). Only the shell
(each host's shell, such as `react-web/src/shell/main.ts`, and the sign-in code it owns) may read it. The same reasoning applies to
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

**An op never panics or exits on scene input.** On desktop, scenes can call every registered op
directly with any arguments, and a panic inside an op aborts the native sidecar with every scene
in it. Validate resource ids, buffer lengths and handles, and return an error. Take structured
arguments as `#[from_v8] Plain<T>` rather than `#[serde]`: serde_v8 panics when a getter or proxy
throws while it reads a value. Such ops take plain data only: own enumerable properties and
arrays, within a depth and total-size budget. Class instances whose fields are prototype
getters, boxed primitives, proxies, functions and resizable buffers are rejected. Replace an upstream op that
panics through the middleware in `crates/dcl_deno/src/js/mod.rs`, keeping its async-ness and
argument count; `every_override_replaces_an_op_of_the_same_shape` checks that each override
replaces an op of the same async-ness. The
`every_*_op_survives_junk_arguments` tests call every registered op with malformed arguments.

**Always ask whether new functionality should exist in headless mode.** The headless build is
the authoritative scene server. It runs scene code with no user present, and it is never a
system scene. A capability the server can never legitimately use (opening a URL, writing the
clipboard, anything that touches the host OS or a page) must be excluded from the headless build
at code level, not merely denied by the permissions table: a config-driven deny is one edit away
from being an allow. The scene promise still gets answered, with a denial.

**On the scene server every hosted scene is a portable.** `reconcile_scene_rooms` (in
`src/bin/headless.rs`) re-inserts any wanted scene missing from `PortableScenes`, so a
scene-callable path that adds or removes portables (`spawn`, `kill`, `exit`) refuses in
`server_mode()` and answers the promise with a denial; otherwise a tenant scene restarts itself
in a loop on the shared engine.

**Every scene asks through one permission path: the stored rules, then the per-use prompt.**
A new kind of scene or portable (a smart wearable, say) resolves there too, with no gate that
answers first from a manifest or an up-front grant; the per-use prompt shows the player what is
asked each time. Fetch and WebSocket go through that path on native, where `dcl_deno` asks
`Fetch` and `Websocket` per use (`crates/dcl_deno/src/js/fetch/mod.rs`,
`crates/dcl_deno/src/js/websocket.rs`); on web they are deliberately ungated, because scene code
there can always reach the page's raw `fetch`, so a check on them would be bypassable.

## Network capabilities

Use the existing engine-assigned scene identity and the transport's Rust address policy when
granting scene network access. Verify that the dialled addresses are checked, including
addresses selected after a redirect where the transport follows redirects. Keep link-local
and metadata addresses blocked. State the scope for each transport and platform in the
[running guide](running.md).

For signed service routing, verify the path the destination actually checks against the real
proxy rewrite. Reject paths whose interpretation can shift the request to another service
during URL normalisation or proxy decoding. Validate untrusted path parameters as single
segments before building the URI. Backends may trust a forwarded path only on a connection
from a trusted proxy that overwrites the client-supplied header.

## Scene signed fetch

For `signedFetch` and `getHeaders`, scene-runtime Rust hashes the UTF-8 bytes of the
snapshotted body before queuing the signing request. An empty string produces the SHA-256
digest of zero bytes. An absent or null body omits the digest. The engine accepts a supplied
digest only if it is exactly 64 lowercase hexadecimal characters. It places the digest in
`hashPayload` and builds the scene, realm and delegation claims from trusted state. The
launch domains `decentraland.org`, `decentraland.zone` and `decentraland.today` select
`org`, `zone` and `today` respectively. Other domains use `org`.

Body-bearing calls require a signer that returns a valid body hash in its signed metadata. If a
new scene runtime is paired with an older engine that does not supply one, the call is rejected
before `signedFetch` sends a request. Bodyless calls retain their existing compatibility. The
signature binds the normalized request path. Callers using `getHeaders` must send the same method,
URL and body they supplied for signing.

A service that relies on a signed body must require `hashPayload` and compare it
with the SHA-256 of the bytes it receives before it processes the request.

## Reporting

Do not disclose a security problem in a public issue or PR. Use the private reporting
channel in Decentraland's [published security policy](https://github.com/decentraland/governance/blob/master/SECURITY.md),
including the affected revision, impact and reproduction details. Coordinate publication
of the fix and any reproduction with the maintainers; keep private findings out of public
PR descriptions until disclosure is agreed.
