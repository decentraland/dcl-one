# Web driver

A runner for the web client's tests that needs nothing installed but Node 24 and Chrome. It
launches Chrome over the DevTools protocol and gives a test what the standalone's `dcl/console`
and CEF input give it:

- **Console commands.** `console(line)` runs `await window.engine_console_command(line)` in the
  app frame and returns what it resolves with, or rejects with the page's message.
- **Trusted input.** `/hud_click`, `/hud_fill`, `/press_key` and `/click_at` (page side:
  `react-web/src/lib/hudAutomation.ts`) ask the runner for real input instead of dispatching DOM
  events, so the page sees `isTrusted` events.

Chrome is started with `--remote-debugging-pipe`, never a port (the benchmark runner forbids
`--remote-debugging-port`).

## Use

```sh
node local-harness/web-driver/driver.mjs --url http://localhost:5173/ '/hud_click Save'
```

```
node driver.mjs --url <url> [--chrome <path>] [--headed] [--arg <flag>]... [command ...]
```

One JSON line per command, `{"command", "ok", "output"}` or `{"command", "ok": false, "error"}`;
without commands it reads them from stdin, one per line. Exit code 1 if a command failed, 2 for
a bad command line. On Git Bash for Windows, pass commands on stdin: it rewrites an argument that
starts with `/`.

From a script:

```js
import { launch } from './driver.mjs'
const driver = await launch({ url, args: ['--enable-unsafe-webgpu'] })
await driver.ready()
await driver.console('/hud_click Save')
await driver.close()
```

`findChrome()` takes `CHROME_PATH`, else the places `local-harness/benchmark` looks. `launch`
takes `headless` (default true), `args` (more Chrome flags) and `windowSize`. `ready()` waits for
a frame of the page's origin with both `window.engine_console_command` and
`window.__dclTrustedInput`. `frameTargets()` lists the page and the out-of-process iframes.
`DCL_WEB_DRIVER_DEBUG=1` logs what the driver ignores or cannot answer.

## Wire protocol

The page's side of this is a stable contract, documented in `docs/guidelines/running.md` ("Test
commands"): any DevTools client (a Playwright or Puppeteer script through `exposeBinding` and its
own mouse and keyboard) can install the hook and give the HUD commands real input. This runner is
one implementation of it.

The page calls `window.__dclTrustedInput(json)`, a binding the runner installs in every frame
with `Runtime.addBinding`. The JSON is `{"id", "op", ...}`:

| `op` | fields |
| --- | --- |
| `click` | `x`, `y` (CSS pixels in the calling frame's viewport), `button`: `left`, `right` or `middle` |
| `key` | `code`, `key`, `keyCode` (Windows virtual key code), `text` (optional), `location` (0, 1 left, 2 right), `state` (optional, `down` or `up`; absent presses and releases) |
| `insertText` | `text` |

The runner does the input on the top-level page and then evaluates
`window.__dclTrustedInputDone(id, error)` in the frame that asked, `error` a string or `null`.
It serves a call only from a default (not isolated-world) context of the origin it opened.
`Alt*`, `Control*`, `Meta*` and `Shift*` keys sent with `state` `down` stay held (Alt 1,
Control 2, Meta 4, Shift 8 in CDP's `modifiers`) until sent with `up`, and are on every event
between. A key held with Alt, Control or Meta sends no text.

Before the first click on a document the runner takes a screenshot of the page. A frame that has
only just appeared cannot be hit-tested until its first frame is composited, and a click sent
earlier is lost (it was, one time in five, with several browsers starting at once).

## Why the frame is offset

The app frame is served with `Document-Isolation-Policy`, so it is a same-origin iframe in its own
process, and its events come from the top-level page in that page's viewport pixels. A click's
`x` and `y` are in the frame, so the runner adds where the frame's content box is: the frame
owner's box (`DOM.getFrameOwner`, `DOM.getBoxModel`) in the session that holds it, and so up to
the page. A frame in the page's own process is found the same way in the page's session.

## Tests

```sh
node --test local-harness/web-driver/driver.test.mjs
```

From `rendering/bevy-explorer/source`. Node 24 reads a directory argument as a file, so name the
file, or use the glob `local-harness/web-driver/*.test.mjs`.

The tests serve `fixture/` (a shell with the app in an iframe away from the corner, with a
border) and run Chrome headless against it, once with the app frame out of process and once in
process. Without Chrome they skip, unless `DCL_WEB_DRIVER_REQUIRE_CHROME=1`; the cases that load
`hudAutomation.ts` need `module.stripTypeScriptTypes`. Each browser keeps its profile in a
temporary folder that `close()` removes.
