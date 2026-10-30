# ADR: Explorer end-to-end tests drive it through its console, without Playwright

- Status: Accepted
- Date: 2026-10-29

## Implementation scope

The public checkout includes the web driver, HUD automation helper and native
automation modules described below. Check command registration at the target
revision and current runtime evidence before claiming platform acceptance.
The optional react-web Playwright gate/visual suites and scene-kill harness are
not included in this checkout; their procedures require an authorized test workspace.

## Context

The Bevy Explorer runs in two shells. On the web, `app.html` holds the engine and the HUD (React
DOM) in an out-of-process iframe of the `/play` shell page. On the standalone, the HUD is a CEF
page rendered offscreen and composited by the engine, and nothing outside the process sees its
DOM. Both expose the same console: `engine_console_command` and `window.engine` in the app frame,
the console window natively.

The end-to-end suite moving into dcl-one, `decentraland/explorer-automation`, drives the Unity
client: a C# suite through AltTester, and a web suite in Playwright. A Playwright suite for the
Bevy Explorer would reach the web shell only, find controls by DOM selectors that the standalone
cannot answer, and need a second driver for the standalone, so every scenario would be written
twice or tested on one shell. Playwright's one capability a console lacks, real (trusted) input,
is the Chrome DevTools Protocol (CDP): `Runtime.addBinding`, `Input.dispatchMouseEvent`,
`Input.dispatchKeyEvent`, `Input.insertText`, a few hundred lines to speak directly.

## Decision

End-to-end tests of the Explorer drive it only through its console, the engine API. They use no
Playwright, nor another browser-automation framework (Puppeteer, Selenium or WebDriver,
Synpress): what they need from a browser, the in-repo web driver does natively, with no
dependency. This covers the console's test commands, the web driver, and the suite moving in from
explorer-automation. Existing Playwright procedures for optional react-web tiers and the
scene-kill harness remain outside this decision; their availability is stated above.

- **One interface on both shells.** On the web a test runs console commands in the app frame
  through the web driver (`rendering/bevy-explorer/source/local-harness/web-driver`); on the
  standalone, through `dcl/console` and `dcl/console_result` on loopback, which every native build
  carries and starts only with `--automation-token`. One spec runs on both.
- **What a test needs becomes a console command**, not test-framework code: state reads
  (`/world_state`, `/avatar_state`, `/chat_history`, `/voice_state`, `/entity_screen_position`),
  the HUD (`/hud_list`, `/hud_read`, `/hud_click`, `/hud_fill`, `/hud_wait`, controls found by role
  and accessible name in the page, `react-web/src/lib/hudAutomation.ts`) and input (`/press_key`,
  `/click_at`). Each is listed in `rendering/bevy-explorer/docs/guidelines/running.md`, "Test
  commands".
- **Input is real on both.** The standalone clicks and types through CEF. On the web the page
  finds the control and asks the web driver for the input, which sends it through Chrome's input
  pipeline, so the page sees `isTrusted` events as from a user. Without the driver the commands
  fall back to DOM events and say so (`"input": "synthetic"`).
- **The web driver is ours and dependency-free:** Node built-ins and CDP over
  `--remote-debugging-pipe`. It never opens a debugging port (the benchmark runner refuses one
  too), and it answers input requests only from the default contexts of the origin it opened. It
  is one way to drive the page, not the gate on it: the page's `__dclTrustedInput` request and its
  `__dclTrustedInputDone` answer are a stable contract (`running.md`, "Test commands"), so a
  Playwright or Puppeteer user installs the hook with `exposeBinding` and their own input.
- **The surface ships in every build.** The console, the HUD and input commands natively, and the
  loopback console behind a launch option are the client's agent surface, on purpose: the deployed
  clients are a home for people's own agents, rather than everyone rebuilding the plumbing. The
  preview-only commands stay preview-only; that is a separate decision.
- **What lies outside the Explorer is driven natively too.** Sign-in runs on pages the console
  cannot reach (the auth site on the web, a browser pairing for the standalone); the web driver
  acts on those pages itself, stands in for the wallet with a provider it injects, and the email
  code comes from a mailbox. Page screenshots and their comparison are the driver's as well.
- **explorer-automation's Playwright web suite is rewritten on the driver, not ported.**

Not chosen:

- **Playwright on the web and another driver natively:** two drivers and two specs, and DOM
  selectors the standalone cannot answer.
- **Playwright only for trusted input:** a dependency, its browser downloads and launch flags,
  for the CDP calls the web driver makes directly.
- **DOM events only:** they prove the HUD's handlers, not the browser's input path, and pointer
  events dispatched by the page may not reach the engine canvas's handlers.
- **OS input** (SendInput, CGEvent, xdotool): window focus, screen coordinates, macOS
  accessibility permission and per-OS code; flaky in CI.
- **A Chrome extension calling `chrome.debugger`:** the same CDP, packaged as an extension.
- **Porting explorer-automation's web suite as it is** (Playwright, Synpress wallets): two ways to
  drive a browser in one suite, and a dependency for what the driver does natively.

## Consequences

- A failing step reproduces by typing the same command into the DevTools console or the
  standalone's console. Tests depend on command names and replies, not on HUD markup, apart
  from the accessible names the HUD gives its controls.
- The console grows with what tests need, and what it grows by ships to everyone. The commands
  read state, drive the HUD and input as a user would, or change what `/teleport` and
  `/changerealm` already change; the loopback console is off unless a token is given at launch.
- The web driver and its CDP use are maintained here. Chrome's protocol changes and per-platform
  behaviour land on us (macOS reads `nativeVirtualKeyCode` as a Mac key code, so the driver
  leaves it out). Its tests run against the runner's Chrome on Windows, Ubuntu and macOS in the
  Test Suite.
- Real input on the web needs Chrome, which is the only browser the web client supports.
- Everything Playwright would have given these tests is built and kept here: page actions outside
  the Explorer, a wallet stand-in, screenshot comparison and a test report. That is more code to
  own, in exchange for one way to drive the browser in them and no dependency to follow.
- Optional react-web Playwright tiers and the scene-kill harness are outside this decision;
  this public checkout does not supply their specs and configs.

## Follow-ups

1. The suite itself: explorer-automation's Explorer scenarios on the web driver and `dcl/console`,
   with references recorded in dcl-one.
2. Screenshots of what the user sees on the web: the engine's `/screenshot` captures the canvas
   without the HUD's DOM, and a page capture needs the web driver (`Page.captureScreenshot`).
3. Sign-in on the web driver: actions on the auth pages, an injected EIP-1193 wallet provider, the
   email code from a mailbox, and the standalone's pairing opened in the driver's Chrome. Then
   camera-mode commands.
