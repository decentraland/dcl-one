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
| Scene isolation | V8 isolates inside `dcl_deno_ipc` sidecar processes: one process for all scenes by default; one process per scene with the headless server's `--scene-hosts per-scene`. A sidecar's stdin is empty and its output goes to the engine's log line by line, never to the engine's stdout, which carries the headless server's control channel | One sandbox worker per scene, each with its own wasm memory |
| Engine boundary | Process boundary; the sidecar talks to the engine over a pipe | The `dcl_wasm` relay runs in the engine's memory and copies messages wasm to wasm |
| Signing key | Engine process (`wallet`) | The shell page, a separate process from the app page; the engine asks it for each signature through `wallet::remote_signer` |
| SystemApi sender | Given to the scene the engine marks `is_super`; the engine forwards `SystemApi` only from the sidecar running that scene, while it runs. On the desktop every scene shares the sidecar that runs the system scene, so this separates nothing there; it does where scenes get their own sidecar | Held by the engine-side relay for the one scene the engine marked super; any other scene that sends a `SystemApi` message is ended |

The engine validates every frame the native sidecar sends: a frame it cannot read ends that
sidecar, and a sidecar speaks only for the scenes the engine gave it. Scenes that share a sidecar
rely on V8 isolation between them. With `--scene-hosts per-scene` no two scenes share one, and
losing a sidecar ends only its scene (`crates/dcl_deno_ipc/src/pool.rs`).

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

**The engine never panics or acts for another scene on what a scene host sends.** A frame over
the size limit, cut short, nested too deep, undecodable, or that does not fit the engine's budget
for scene host frames (`dcl_deno_ipc::HOST_FRAME_BUDGET_BYTES`, shared by every host), ends that
host's connection, which the engine handles as a lost sidecar. So does opening more reply channels
than a host may hold: a channel counts until the engine drops its sender, whatever the host says,
so engine code that keeps senders drops the closed ones. The host itself ends a scene that holds
more than its own share (`dcl_deno_ipc::MAX_SCENE_IPC_CHANNELS`), so an honest host loses one
scene, not its connection; only the per-host limit is the engine's. A response for a scene the
host was not given, a `SystemApi` message from a host running no system scene, or IPC data from a
host is dropped and counted (`dcl_deno_ipc::rejected_host_messages`), with one warning per
connection. The engine takes a host's connection only from the process it started; every host's
responses share one bounded queue, which a host waits on rather than shedding into, so a flood
holds back only the flooding host; a host that reads nothing for 10 s is ended. `crates/dcl_deno_ipc/src/engine_ipc_tests.rs`
plays a scene host against the engine side of the pipe; cover every new `SceneToEngine` message
there.

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
