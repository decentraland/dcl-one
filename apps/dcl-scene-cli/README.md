# dcl-scene-cli

One Rust binary that scaffolds, builds, previews and publishes Decentraland SDK7 scenes, replacing
`@dcl/sdk-commands` 7.27.0 ([js-sdk-toolchain](https://github.com/decentraland/js-sdk-toolchain)) with
the same commands and flags. rolldown, the TypeScript toolchain, abgen and `livekit-server` are in the
binary; nothing to `npm install`. The vendored runtime pins `@dcl/sdk` 7.29.1-34986384248.commit-bb45080
(src/vendor/README.md).

## Start a scene

```
dcl-scene-cli init [--dir D] [--project scene|smart-wearable] [-y|--yes] [--node-modules-only]
dcl-scene-cli get-context-files [--dir D] [--offline]
```

`init` writes embedded templates and `node_modules` offline. The `/inspector/*` UI needs
`npm install --save-dev @dcl/inspector` or `DCL_ONE_INSPECTOR_DIR`. `get-context-files` writes the
`migrate-smart-items-to-code` skill and downloads `ai-sdk-context` into `dclcontext/`.

## Build it

```
dcl-scene-cli build [--dir D] [-p|--production] [-w|--watch] [--ignoreComposite]
                    [--customEntryPoint] [--skip-install] [--skip-type-check] [--built-in]
```

### Custom build scripts

A `package.json` `scripts.build` is run by `build`, `start`, `deploy` and `pack`: once, from the scene
root, via `sh -c` (Windows `cmd.exe /d /s /c`) with `node_modules/.bin` on `PATH`; no npm hooks,
install or SDK fallback. It must terminate, exit zero and leave a nonempty bundle at
`scene.json.main`. `DCL_SCENE_CLI_PRODUCTION` is `1` for deploy, `0` for preview;
`DCL_SCENE_CLI_MAIN` names the bundle. Keep output in its own directory (normally `bin/`) and
serialize build and deploy of one scene. `build --built-in` forces the SDK pipeline (also from inside
a script); `--skip-build` skips the initial run.

The built-in pipeline bundles to `bin/index.js`, type-checks beside the build with incremental tsc
(`.dcl-cache/`), and encodes `main.crdt` in Rust (`DCL_ONE_CRDT_VERIFY=1` compares with node).

## Preview it

```
dcl-scene-cli start [--dir D] [-p|--port N] [--skip-build] [--no-watch] [-m|--mobile]
                    [--data-layer] [--offline-comms]
                    [--no-livekit | --livekit-url WS_URL --livekit-api-key KEY
                     --livekit-api-secret SECRET|--livekit-api-secret-file F] [--livekit-room NAME]
                    [--no-asset-bundles] [--no-mcp] [--mcp-port N] [--error-source-lines-context N]
                    [--no-host] [--tunnel WSS_URL] [--talk [--player URL]] [-- EXPLORER_PARAMS...]
```

Serves a local realm on 8000 and reloads the client on save. `http://127.0.0.1:8000` is the launch
page (deep-link knobs, web **Player** via `?where=web&player=<url>`). abgen on 5147 serves
`{realm}/optimized-assets`. Scene errors print via the client's MCP port (default 8123, range
1024-65535). `/content/contents/{hash}` resolves by path: a stale digest gets the current bytes.
`--data-layer` serves the Creator Hub protocol at `/data-layer` and `/inspector/`.

### Play

A build with `PLAY_EMBED_DIR` serves the web explorer at `http://127.0.0.1:8000/play/`
(cross-origin isolated, CSP from `engine/web/csp-policy.mts`; secure context only); otherwise `/play/`
is 404. `--talk` opens it (else `DCL_SCENE_CLI_SITE`, `--player` or `DCL_SCENE_CLI_WEB_EXPLORER`) with
the Scene assistant and prints a `Talk:` URL.

- Cargo: `scripts/compose-play-embed.sh`, then
  `PLAY_EMBED_DIR=$ONE/engine/deploy/web cargo build -p dcl-scene-cli`.
- Nix: `dcl-scene-cli.override { playEmbed = self.packages.x86_64-linux.engine-web-embedded; }`.

### Token

Each `start` mints a capability token. Writes (`/api/project/{file,asset}`, assistant, debug
commands, `/scene-json`, `/scene-thumbnail`, `/data-layer?token=`) need `Authorization: Bearer
<token>`, else 401. Pages get it only by `<meta>` on a top-level loopback navigation or a `#token=`
fragment.

On **Scene**, use Left/Right, Home or End to switch editor tabs. In **Shape**, arrow keys
move between parcels; Space or Enter toggles a parcel. **Add spawn area** opens numeric
coordinates as an alternative to drawing. Save or Cancel finishes the spawn draft.
Errors stay visible until dismissed; successful saves are announced briefly.

### Voice and multiplayer

The embedded LiveKit uses 7880-7882; `--no-livekit` and `--tunnel` use the ws-room (no audio). An
external server takes the `--livekit-*` flags or `LIVEKIT_URL`, `LIVEKIT_API_KEY`,
`LIVEKIT_API_SECRET`. Unity needs `--accept-untrusted-realm` and
`--gatekeeper-url http://127.0.0.1:8000/get-scene-adapter`. macOS cargo builds embed no server.

```
dcl-scene-cli host [--dir D] [--preview http://127.0.0.1:8000] [--room room-1]
```

With `"authoritativeMultiplayer": true`, `start` also runs the bundle headless as the server (voice
off); `--no-host` skips it; `host` runs only the server.

### Storage

```
dcl-scene-cli storage scene|player|env get|set|delete|list|clear [KEY]
                    [--value V] [--address 0x…] [--prefix P] [--limit N] [--offset N]
                    [--confirm] [--json] [--dir D] [--target local|preview|org|zone|URL]
                    [--preview http://127.0.0.1:8000] [--sign-key FILE]
dcl-scene-cli storage target [local|org|zone|URL] [--dir D]
dcl-scene-cli storage export [--out FILE] | import FILE [--merge]
```

The preview serves the storage service's routes from `.dcl-one/storage.sqlite` or forwards them,
signed, upstream; a bare command acts where the preview's **Storage** tab points.

Browser imports merge by default. Unchecking **Merge with existing values** requires
confirmation before replacing every local scope. JSON values are parsed; quote a string
such as `"true"` to keep it as text. Environment values are always text. Use Previous/Next
to browse larger listings.

## Share it

```
dcl-preview-tunnel --listen 0.0.0.0:9000 --token SECRET                         # public host
dcl-scene-cli start --tunnel wss://tunnel.example:9000 --tunnel-token SECRET    # your machine
dcl-scene-cli sync [--dir D] [--node URL] [--project ID] [--sign-key FILE] [--prefer local|remote] [--dry-run]
```

Prefer `--tunnel-token-file` or `DCL_SCENE_CLI_TUNNEL_TOKEN`. `sync` is a two-way content-hash sync
with a catalyrst builder project (`.dcl-one/sync.json`).

## Publish it

```
dcl-scene-cli deploy [--dir D] [-t|--target-server CATALYST-DOMAIN-OR-CONTENT-URL]
                     [--dry-run] [--timestamp MS]
dcl-scene-cli world access show|edit <name>
dcl-scene-cli world access set    <name> --public | --password S | --user U... --community C...
dcl-scene-cli world access add|remove <name> --user U | --community C
```

`deploy` signs in a browser on the preview server, or with `DCL_PRIVATE_KEY`. Worlds go to the worlds
server, parcels to the first healthy public catalyst (`--target-server` /
`DCL_SCENE_CLI_TARGET_SERVER` override); only content missing from `/available-content` is uploaded
(`DCL_SCENE_CLI_UPLOAD_ALL=1` sends all). Also `unpublish` and `pack`.

## Reference

Ports: 8000 preview, 5147 abgen, 7880-7882 LiveKit. Scene files (never deployed): `bin/`,
`.dcl-one/`, `.dcl-cache/`, `.dcl-optimized-assets/`.

Environment: `DCL_SCENE_CLI_CATALYST` (default `https://interconnected.online`),
`DCL_SCENE_CLI_CATALYST_ROTATION`, `DCL_SCENE_CLI_WORLD_BASE`, `DCL_SCENE_CLI_SOCIAL_API`,
`DCL_SCENE_CLI_FEATURE_FLAGS`, `DCL_SCENE_CLI_CONTENT_CACHE_MAX`, `DCL_SCENE_CLI_WEB_EXPLORER`,
`DCL_SCENE_CLI_ALLOWED_ORIGINS`, `DCL_SCENE_CLI_LINKER_HOST`, `DCL_PRIVATE_KEY`, `ABGEN_BIN`; build
time `PLAY_EMBED_DIR` and `DCL_SCENE_CLI_SITE` (the operator's site origin, no default).

Tests: `cargo test -p dcl-scene-cli` needs a toolchain and `node`; gated suites are in
docs/testing.md.

### Creator Hub integration

```sh
DCL_SCENE_CLI_ALLOWED_ORIGINS=https://example.org dcl-scene-cli start --dir ./my-scene
```

**Edit in Creator Hub** opens `<site>/create?projectUrl=…#token=…` (`DCL_SCENE_CLI_CREATOR_HUB_URL`
overrides). Loopback routes: `/api/project`, `/api/project/{files,file,assets,asset}`
(revision-checked writes), `/api/project/assistant/{providers,turn,conversations,bridge}`,
`/api/project/debug[/sessions|/events|/command]`, `/scene-inspector`.

The Scene assistant runs a local `codex`, `claude`, `cursor-agent`, `gemini`, `opencode` or `hermes`;
extra setups `{id, label, engine, command, model?, args?}` go in
`$XDG_CONFIG_HOME/dcl-scene-cli/assistants.json` (or `DCL_SCENE_CLI_ASSISTANTS`). Scene tools need a
separately started `dcl-scene-mcp --http` and `DCL_SCENE_CLI_SCENE_MCP_URL`, `DCL_SCENE_MCP_BEARER`,
`DCL_SCENE_MCP_TOKEN`.

UI Designer: install `@dcl/inspector` (verified 7.46.1), run
`bash scripts/install-ui-designer-runtime.sh /path/to/@dcl/inspector`, set `DCL_ONE_INSPECTOR_DIR` and
start with `--data-layer`.
