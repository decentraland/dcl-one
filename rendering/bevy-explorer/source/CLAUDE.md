# bevy-explorer

- `react-web/`: the web explorer's HUD (React). Read `react-web/AGENTS.md` before changing it.
- `deploy/web/`: what the web build ships: the page, the engine's JS, the service worker.
- `crates/`, `src/`: the engine (Rust). It runs natively and, built to wasm, in the browser. A
  local wasm build is `make web` in `apps/bevy-explorer` (`python3 build-web.py`).
- For the hosted scene editor, start at `react-web/src/features/editorHost/`; its checked-in pin and host contract define the integration. Resolve any external editor handbook through authorized task context.
