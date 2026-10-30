// A dependency-free runner that drives Chrome for the web client's tests, so they reach the web
// client as they reach the standalone through `dcl/console`:
//
//   - Console commands. `console(line)` runs `await window.engine_console_command(line)` in the
//     app frame.
//   - Trusted input. The page (react-web/src/lib/hudAutomation.ts) calls
//     `window.__dclTrustedInput(json)` for a click, a key or text to insert instead of
//     dispatching DOM events. The runner installs that function in every frame with
//     `Runtime.addBinding` and answers each request with `Input.dispatchMouseEvent`,
//     `dispatchKeyEvent` or `insertText` on the top-level page, so the page sees `isTrusted`
//     events, as it does from the standalone's CEF input. It then tells the page it is done by
//     evaluating `window.__dclTrustedInputDone(id, error)` in the frame that asked.
//
// Chrome is started with --remote-debugging-pipe and never with a port: the benchmark runner
// (local-harness/benchmark) refuses --remote-debugging-port, and nothing else on the machine can
// reach a pipe. Only Node's built-ins are used.
//
// The app frame is mounted by a shell page and served with Document-Isolation-Policy, so it is
// a same-origin but out-of-process iframe: a target of its own, found with Target.setAutoAttach.
// Input goes to the top-level page in the main frame's viewport pixels, so a frame-local click
// position is offset by where the frame's content box sits in that viewport.
//
//   node driver.mjs --url <url> [--chrome <path>] [--headed] [--arg <flag>]... [command ...]

import { spawn, spawnSync } from 'node:child_process';
import { realpathSync, statSync } from 'node:fs';
import { mkdtemp, rm } from 'node:fs/promises';
import { homedir, tmpdir } from 'node:os';
import path from 'node:path';
import readline from 'node:readline';
import { fileURLToPath } from 'node:url';

const BINDING = '__dclTrustedInput';
const DONE = '__dclTrustedInputDone';
const DEFAULT_TIMEOUT_MS = 120_000;
const CLOSE_TIMEOUT_MS = 5_000;
const POLL_MS = 100;
const PRESENT_TIMEOUT_MS = 5_000;

// The bits of the `modifiers` field of CDP's input events.
const MODIFIER_BITS = { Alt: 1, Control: 2, Meta: 4, Shift: 8 };
// `button` and the `buttons` bit of the mouse button a request names.
const MOUSE_BUTTONS = { left: 1, right: 2, middle: 4 };
// The targets that hold frames of the page: the page itself and an out-of-process iframe. Others
// (workers) are let go.
const FRAME_TARGETS = new Set(['page', 'iframe']);

// Chrome as the benchmark runner finds it (local-harness/benchmark/src/chrome.rs `find`).
function chromeCandidates(env) {
  if (process.platform === 'win32') {
    return ['ProgramFiles', 'ProgramFiles(x86)', 'LocalAppData']
      .filter((name) => env[name])
      .map((name) => path.join(env[name], 'Google', 'Chrome', 'Application', 'chrome.exe'));
  }
  if (process.platform === 'darwin') {
    const inside = 'Google Chrome.app/Contents/MacOS/Google Chrome';
    return [path.join('/Applications', inside), path.join(homedir(), 'Applications', inside)];
  }
  const dirs = (env.PATH ?? '').split(path.delimiter).filter(Boolean);
  return ['google-chrome-stable', 'google-chrome', 'chromium', 'chromium-browser'].flatMap((name) =>
    dirs.map((dir) => path.join(dir, name))
  );
}

const isFile = (file) => statSync(file, { throwIfNoEntry: false })?.isFile() === true;

/** The Chrome to run: `CHROME_PATH` when set (null when nothing is there, as the benchmark
 *  runner refuses it rather than choosing another), else the first install it knows of. */
export function findChrome(env = process.env) {
  if (env.CHROME_PATH) return isFile(env.CHROME_PATH) ? env.CHROME_PATH : null;
  return chromeCandidates(env).find(isFile) ?? null;
}

function noChromeMessage(env) {
  if (env.CHROME_PATH) return `no Chrome at '${env.CHROME_PATH}' (CHROME_PATH)`;
  const looked = chromeCandidates(env).slice(0, 3).join(', ');
  return `Google Chrome is not installed (looked in ${looked}); install it, or point CHROME_PATH at it`;
}

/** Does a call of the binding from this execution context count? Only from a default (not an
 *  isolated-world, as a browser extension's) context of the origin the runner opened: another
 *  origin's frame must not be able to make input. */
export function acceptsBinding(context, origin) {
  return context != null && context.isDefault === true && context.origin === origin;
}

const debug = process.env.DCL_WEB_DRIVER_DEBUG
  ? (...parts) => console.error('[web-driver]', ...parts)
  : () => {};

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// `promise`, or a rejection with `message` after `ms`.
async function within(promise, ms, message) {
  let timer;
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error(message())), ms);
      })
    ]);
  } finally {
    clearTimeout(timer);
  }
}

// The DevTools protocol over Chrome's pipe: JSON messages, each ended by a NUL byte. Commands go
// out on `output` (Chrome's fd 3), replies and events come in on `input` (fd 4).
class Pipe {
  #next = 1;
  #pending = new Map();
  #failure = null;

  constructor(output, input, onEvent) {
    this.output = output;
    let buffered = Buffer.alloc(0);
    input.on('data', (chunk) => {
      buffered = Buffer.concat([buffered, chunk]);
      for (let end = buffered.indexOf(0); end !== -1; end = buffered.indexOf(0)) {
        const text = buffered.subarray(0, end).toString('utf8');
        buffered = buffered.subarray(end + 1);
        if (text !== '') this.#receive(text, onEvent);
      }
    });
    // a dead Chrome is reported through its exit; a broken stream must not crash the runner
    input.on('error', () => {});
    output.on('error', () => {});
  }

  #receive(text, onEvent) {
    let message;
    try {
      message = JSON.parse(text);
    } catch {
      return debug('unreadable message', text.slice(0, 200));
    }
    if (message.id != null) {
      const call = this.#pending.get(message.id);
      this.#pending.delete(message.id);
      if (!call) return;
      if (message.error) call.reject(new Error(`${call.method}: ${message.error.message}`));
      else call.resolve(message.result ?? {});
    } else if (message.method) {
      onEvent(message);
    }
  }

  send(method, params = {}, sessionId) {
    if (this.#failure) return Promise.reject(this.#failure);
    const id = this.#next++;
    const message = { id, method, params };
    if (sessionId) message.sessionId = sessionId;
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { method, resolve, reject });
      this.output.write(`${JSON.stringify(message)}\0`, (error) => {
        if (error && this.#pending.delete(id)) reject(error);
      });
    });
  }

  /** Chrome is gone: reject what waits for it, and everything after. */
  fail(error) {
    if (this.#failure) return;
    this.#failure = error;
    for (const call of this.#pending.values()) call.reject(error);
    this.#pending.clear();
  }
}

class Driver {
  #proc;
  #profile;
  #origin;
  #pipe;
  #exit;
  #stderr = [];
  #sessions = new Map();
  #contexts = new Map();
  #page = null;
  #app = null;
  #held = new Set();
  #shown = new WeakSet();
  #queue = Promise.resolve();
  #closing = null;
  /** Chrome's own name and version, once launched, e.g. `Chrome/156.0.8078.48`. */
  product = '';

  constructor(proc, profile, url) {
    this.#proc = proc;
    this.#profile = profile;
    this.#origin = new URL(url).origin;
    this.url = url;
    this.#pipe = new Pipe(proc.stdio[3], proc.stdio[4], (event) => this.#onEvent(event));

    // the last lines Chrome said, for when it cannot start
    let partial = '';
    proc.stdio[2]?.on('data', (chunk) => {
      const lines = (partial + chunk.toString('utf8')).split(/\r?\n/);
      partial = lines.pop() ?? '';
      this.#stderr.push(...lines.filter(Boolean));
      this.#stderr.splice(0, Math.max(0, this.#stderr.length - 20));
    });
    proc.stdio[2]?.on('error', () => {});

    // 'exit' comes before the last of stderr is read: give it a moment, as 'close' would
    this.#exit = new Promise((resolve) => {
      const done = (info) => {
        this.#pipe.fail(new Error(this.#goneMessage(info)));
        resolve(info);
      };
      proc.on('error', (error) => done({ error }));
      proc.once('exit', (code, signal) => setTimeout(() => done({ code, signal }), 250));
    });
  }

  #goneMessage({ error, code, signal } = {}) {
    if (error) return `could not start Chrome: ${error.message}`;
    const tail = this.#stderr.length ? `\n${this.#stderr.slice(-8).join('\n')}` : '';
    return `Chrome exited (${signal ?? `code ${code}`}) before it was done${tail}`;
  }

  // --- the browser ---

  #send(method, params, session) {
    return this.#pipe.send(method, params, session?.id);
  }

  /** Open the page: a target of its own, set up before it loads so no frame escapes. */
  async open() {
    const version = await this.#pipe.send('Browser.getVersion');
    this.product = version.product;
    const { targetId } = await this.#pipe.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await this.#pipe.send('Target.attachToTarget', { targetId, flatten: true });
    this.#page = { id: sessionId, type: 'page', targetId, parent: null };
    this.#sessions.set(sessionId, this.#page);
    await this.#setUp(this.#page);
    const { errorText } = await this.#send('Page.navigate', { url: this.url }, this.#page);
    if (errorText) throw new Error(`could not open ${this.url}: ${errorText}`);
  }

  // What every frame target gets: its execution contexts tracked, the binding installed, the
  // frames it opens attached (and held until they are set up the same way).
  async #setUp(session) {
    const send = (method, params) => this.#send(method, params, session);
    await Promise.all([
      send('Runtime.enable'),
      send('Runtime.addBinding', { name: BINDING }),
      send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true }),
      send('Page.enable'),
      // without it a headless or background window has no focused frame, and keys go nowhere
      send('Emulation.setFocusEmulationEnabled', { enabled: true })
    ]);
  }

  #onEvent({ method, params, sessionId }) {
    switch (method) {
      case 'Target.attachedToTarget':
        // only the frames' sessions report what they attach; the page's own attach is explicit
        if (sessionId) this.#attached(sessionId, params);
        break;
      case 'Target.detachedFromTarget':
        this.#sessions.delete(params.sessionId);
        for (const [key, context] of this.#contexts) {
          if (context.session.id === params.sessionId) this.#contexts.delete(key);
        }
        break;
      case 'Runtime.executionContextCreated': {
        const session = this.#sessions.get(sessionId);
        const { id, origin, auxData = {} } = params.context;
        if (session) {
          this.#contexts.set(`${sessionId}:${id}`, {
            session,
            id,
            origin,
            isDefault: auxData.isDefault === true,
            frameId: auxData.frameId
          });
        }
        break;
      }
      case 'Runtime.executionContextDestroyed':
        this.#contexts.delete(`${sessionId}:${params.executionContextId}`);
        break;
      case 'Runtime.executionContextsCleared':
        for (const [key, context] of this.#contexts) {
          if (context.session.id === sessionId) this.#contexts.delete(key);
        }
        break;
      case 'Runtime.bindingCalled': {
        if (params.name !== BINDING) break;
        const session = this.#sessions.get(sessionId);
        // one request at a time, in the order asked: a click and the keys after it must not mix
        if (session) {
          this.#queue = this.#queue.then(() => this.#serve(session, params)).catch((error) => debug('request failed', error));
        }
        break;
      }
    }
  }

  #attached(parentId, { sessionId, targetInfo, waitingForDebugger }) {
    if (this.#sessions.has(sessionId)) return;
    const session = {
      id: sessionId,
      type: targetInfo.type,
      targetId: targetInfo.targetId,
      parent: this.#sessions.get(parentId) ?? null
    };
    this.#sessions.set(sessionId, session);
    void (async () => {
      const frames = FRAME_TARGETS.has(session.type);
      try {
        if (frames) await this.#setUp(session);
      } catch (error) {
        debug('could not set up', session.type, error.message);
      }
      // a target held at start runs only now, whether or not it was set up
      if (waitingForDebugger) await this.#send('Runtime.runIfWaitingForDebugger', {}, session).catch(() => {});
      if (!frames) {
        await this.#pipe.send('Target.detachFromTarget', { sessionId }).catch(() => {});
        this.#sessions.delete(sessionId);
      }
    })();
  }

  /** The frame targets that are up: the page, and an out-of-process iframe for each frame that
   *  has its own process. For checking what a test runs against. */
  frameTargets() {
    return [...this.#sessions.values()]
      .filter((session) => FRAME_TARGETS.has(session.type))
      .map(({ type, targetId }) => ({ type, targetId }));
  }

  // --- the app frame: ready and console commands ---

  // The context that has the console and the binding: the app frame's, not the shell's.
  async #findApp() {
    if (this.#app && this.#contexts.has(`${this.#app.session.id}:${this.#app.id}`)) return this.#app;
    this.#app = null;
    for (const context of [...this.#contexts.values()]) {
      if (!acceptsBinding(context, this.#origin)) continue;
      const answer = await this.#send(
        'Runtime.evaluate',
        {
          expression: `typeof window.engine_console_command === 'function' && typeof window.${BINDING} === 'function'`,
          contextId: context.id,
          returnByValue: true
        },
        context.session
      ).catch(() => null); // a context that went away while asking
      if (answer?.result?.value === true) return (this.#app = context);
    }
    return null;
  }

  /** Resolves once a context of the page's origin has both `window.engine_console_command` and
   *  `window.__dclTrustedInput`. */
  async ready({ timeout = DEFAULT_TIMEOUT_MS } = {}) {
    const deadline = Date.now() + timeout;
    const exited = this.#exit.then((info) => {
      throw new Error(this.#goneMessage(info));
    });
    exited.catch(() => {});
    for (;;) {
      const app = await Promise.race([this.#findApp(), exited]);
      if (app) return;
      if (Date.now() >= deadline) {
        const seen = [...this.#contexts.values()]
          .filter((context) => context.isDefault)
          .map((context) => context.origin);
        throw new Error(
          `${this.url} did not get window.engine_console_command and window.${BINDING} in a ${this.#origin} frame within ${timeout} ms (frames: ${seen.join(', ') || 'none'})`
        );
      }
      await Promise.race([sleep(POLL_MS), exited]);
    }
  }

  /** `await window.engine_console_command(line)` in the app frame: what it resolves with, or a
   *  rejection with the message the page failed with. */
  async console(line, { timeout = DEFAULT_TIMEOUT_MS } = {}) {
    const started = Date.now();
    await this.ready({ timeout });
    const context = await this.#findApp();
    if (!context) throw new Error('the app frame went away');
    // a rejection arrives as a value, so the page's message is not wrapped in a protocol error
    const expression = `(async (line) => {
      try {
        return { ok: true, value: await window.engine_console_command(line) };
      } catch (e) {
        const error = e instanceof Error ? e.message : typeof e === 'string' ? e : JSON.stringify(e);
        return { ok: false, error: error ?? String(e) };
      }
    })(${JSON.stringify(String(line))})`;
    const exited = this.#exit.then((info) => {
      throw new Error(this.#goneMessage(info));
    });
    exited.catch(() => {});
    const answer = await within(
      Promise.race([
        this.#send('Runtime.evaluate', { expression, contextId: context.id, awaitPromise: true, returnByValue: true }, context.session),
        exited
      ]),
      Math.max(1, timeout - (Date.now() - started)),
      () => `\`${line}\` did not finish within ${timeout} ms`
    );
    if (answer.exceptionDetails) {
      throw new Error(answer.exceptionDetails.exception?.description ?? answer.exceptionDetails.text);
    }
    const { ok, value, error } = answer.result.value;
    if (!ok) throw new Error(error);
    return value;
  }

  // --- trusted input ---

  async #serve(session, { executionContextId, payload }) {
    const context = this.#contexts.get(`${session.id}:${executionContextId}`);
    if (!acceptsBinding(context, this.#origin)) return debug('ignored a call from', context?.origin ?? 'an unknown context');
    let request;
    try {
      request = JSON.parse(payload);
    } catch {
      return debug('unreadable request');
    }
    if (request == null || !Number.isSafeInteger(request.id)) return debug('a request without an id');
    let error = null;
    try {
      await this.#perform(context, request);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
    // after the input call has returned: the page then sees its effects
    const answer = `window.${DONE}(${request.id}, ${error === null ? 'null' : JSON.stringify(error)})`;
    const { exceptionDetails } = await this.#send('Runtime.evaluate', { expression: answer, contextId: context.id }, context.session)
      .catch((e) => ({ exceptionDetails: { text: e.message } }));
    if (exceptionDetails) debug('could not answer request', request.id, exceptionDetails.text);
  }

  async #perform(context, request) {
    switch (request.op) {
      case 'click':
        return this.#click(context, request);
      case 'key':
        return this.#key(request);
      case 'insertText':
        if (typeof request.text !== 'string') throw new Error('a trusted insertText needs "text"');
        return this.#send('Input.insertText', { text: request.text }, this.#page);
      default:
        throw new Error(`unknown trusted input op \`${request.op}\``);
    }
  }

  get #modifiers() {
    let mask = 0;
    for (const code of this.#held) mask |= MODIFIER_BITS[code.replace(/(Left|Right)$/, '')];
    return mask;
  }

  // A frame that has only just appeared is not hit-testable until its first frame is composited:
  // a click sent before then goes to the frame under it and is lost (about one first click in five
  // with several browsers starting at once). A screenshot of the page waits for that frame, so one
  // is taken before the first click on each document; the engine's own start-up takes longer than
  // that, but a fixture's does not.
  async #presented(context) {
    if (this.#shown.has(context)) return;
    this.#shown.add(context);
    const shot = this.#send('Page.captureScreenshot', { format: 'jpeg', quality: 1, optimizeForSpeed: true }, this.#page);
    // best effort: a window that cannot be drawn must not hold up the input
    await within(shot, PRESENT_TIMEOUT_MS, () => 'no first frame').catch((error) => debug(error.message));
  }

  async #click(context, { x, y, button }) {
    if (!Number.isFinite(x) || !Number.isFinite(y)) throw new Error('a trusted click needs numeric "x" and "y"');
    const bit = MOUSE_BUTTONS[button];
    if (bit == null) throw new Error(`unknown button \`${button}\`: use left, right or middle`);
    await this.#presented(context);
    const offset = await this.#frameOffset(context);
    const at = { x: offset.x + x, y: offset.y + y, modifiers: this.#modifiers };
    const mouse = (type, extra) => this.#send('Input.dispatchMouseEvent', { type, ...at, ...extra }, this.#page);
    await mouse('mouseMoved', { buttons: 0 });
    await mouse('mousePressed', { button, buttons: bit, clickCount: 1 });
    await mouse('mouseReleased', { button, buttons: 0, clickCount: 1 });
  }

  async #key({ code, key, keyCode, text, location = 0, state }) {
    if (typeof code !== 'string' || typeof key !== 'string' || !Number.isInteger(keyCode)) {
      throw new Error('a trusted key needs "code", "key" and "keyCode"');
    }
    if (state != null && state !== 'down' && state !== 'up') throw new Error(`unknown state \`${state}\`: use down or up`);
    const isModifier = /^(Alt|Control|Meta|Shift)(Left|Right)$/.test(code);
    const dispatch = (type, modifiers, withText) =>
      this.#send(
        'Input.dispatchKeyEvent',
        {
          type,
          key,
          code,
          // no nativeVirtualKeyCode: it is the OS's own code, a Mac one on macOS (where 27, Escape's
          // Windows code, is Minus), and a mismatched one makes Chrome repeat the key
          windowsVirtualKeyCode: keyCode,
          location,
          modifiers,
          ...(withText ? { text, unmodifiedText: text } : {})
        },
        this.#page
      );
    if (state !== 'up') {
      // the key's own modifier counts in its keydown, as a browser reports it
      if (isModifier && state === 'down') this.#held.add(code);
      const modifiers = this.#modifiers | (isModifier ? MODIFIER_BITS[code.replace(/(Left|Right)$/, '')] : 0);
      // a key held with Alt, Control or Meta is a shortcut, not typing
      const types = typeof text === 'string' && text !== '' && (modifiers & ~MODIFIER_BITS.Shift) === 0;
      await dispatch(types ? 'keyDown' : 'rawKeyDown', modifiers, types);
    }
    if (state !== 'down') {
      if (isModifier && state === 'up') this.#held.delete(code);
      await dispatch('keyUp', this.#modifiers, false);
    }
  }

  // --- where a frame is ---

  async #rootFrameId(session) {
    const { frameTree } = await this.#send('Page.getFrameTree', {}, session);
    return frameTree.frame.id;
  }

  // Where the content box of a frame's owner element sits in the viewport of the session that
  // holds the element (a frame of the page, or an out-of-process iframe's own viewport).
  async #contentOrigin(session, frameId) {
    const { backendNodeId } = await this.#send('DOM.getFrameOwner', { frameId }, session);
    const { model } = await this.#send('DOM.getBoxModel', { backendNodeId }, session);
    return { x: model.content[0], y: model.content[1] };
  }

  // Where a session's root frame begins in the top-level page's viewport: (0, 0) for the page,
  // for an out-of-process iframe its owner's content box in the parent's session, and so on up.
  async #rootOffset(session) {
    if (!session.parent) return { x: 0, y: 0 };
    const own = await this.#contentOrigin(session.parent, await this.#rootFrameId(session));
    const parent = await this.#rootOffset(session.parent);
    return { x: own.x + parent.x, y: own.y + parent.y };
  }

  // Where the frame an execution context belongs to begins in the top-level page's viewport.
  async #frameOffset({ session, frameId }) {
    if (!frameId) throw new Error('the calling frame is unknown');
    const base = await this.#rootOffset(session);
    if (frameId === (await this.#rootFrameId(session))) return base;
    // a frame inside this session's page (the app frame, when it shares the page's process)
    const own = await this.#contentOrigin(session, frameId);
    return { x: own.x + base.x, y: own.y + base.y };
  }

  // --- closing ---

  /** `Browser.close`, a kill after 5 s, and the temporary profile removed. */
  close() {
    this.#closing ??= (async () => {
      this.#pipe.send('Browser.close').catch(() => {});
      // a timer left running would hold the process open after the close
      const exitedWithin = (ms) => within(this.#exit, ms, () => 'Chrome is still running').then(() => true, () => false);
      if (!(await exitedWithin(CLOSE_TIMEOUT_MS))) {
        this.#kill();
        await exitedWithin(CLOSE_TIMEOUT_MS);
      }
      this.#pipe.fail(new Error('the driver is closed'));
      for (const stream of this.#proc.stdio) stream?.destroy();
      try {
        await rm(this.#profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
      } catch (error) {
        // Chrome's helpers may hold a file a moment longer; a temporary folder is not worth a failure
        console.error(`web-driver: could not remove ${this.#profile}: ${error.message}`);
      }
    })();
    return this.#closing;
  }

  #kill() {
    const pid = this.#proc.pid;
    if (pid != null && process.platform === 'win32') {
      // the whole tree: the browser's helpers outlive a plain kill
      spawnSync('taskkill', ['/pid', String(pid), '/T', '/F'], { stdio: 'ignore' });
    }
    this.#proc.kill('SIGKILL');
  }
}

/** Start Chrome on `url` and drive it. `args` are extra Chrome flags, after the driver's own. */
export async function launch({ url, chrome, headless = true, args = [], windowSize = [1600, 900] } = {}) {
  if (typeof url !== 'string' || url === '') throw new TypeError('launch needs the url to open');
  const executable = chrome ?? findChrome();
  if (executable == null) throw new Error(noChromeMessage(process.env));
  const profile = await mkdtemp(path.join(tmpdir(), 'dcl-web-driver-'));
  const flags = [
    '--remote-debugging-pipe',
    `--user-data-dir=${profile}`,
    '--no-first-run',
    '--no-default-browser-check',
    '--no-startup-window',
    `--window-size=${windowSize[0]},${windowSize[1]}`,
    // a hidden or throttled page pauses the engine (deploy/web/engine/boot.js), and a window that
    // another covers counts as hidden
    '--disable-renderer-backgrounding',
    '--disable-backgrounding-occluded-windows',
    '--disable-background-timer-throttling',
    ...(headless ? ['--headless=new'] : []),
    ...args
  ];
  const proc = spawn(executable, flags, { stdio: ['ignore', 'ignore', 'pipe', 'pipe', 'pipe'] });
  const driver = new Driver(proc, profile, url);
  try {
    await driver.open();
  } catch (error) {
    await driver.close();
    throw error;
  }
  return driver;
}

// --- the command line ---

const USAGE = `usage: node driver.mjs --url <url> [--chrome <path>] [--headed] [--arg <flag>]... [command ...]

Opens <url> in Chrome, waits for the page's console, and runs each command, printing one JSON
line {"command", "ok", "output" | "error"} per command. Without commands it reads them from
stdin, one per line. Exits 1 if any command failed.`;

function parseArguments(argv) {
  const options = { url: null, chrome: undefined, headless: true, args: [], commands: [] };
  for (let i = 0; i < argv.length; i++) {
    // `--flag value` or `--flag=value`; anything else is a command
    const [flag, inline] = argv[i].startsWith('--') ? argv[i].split(/=(.*)/s, 2) : [argv[i]];
    const value = () => {
      if (inline !== undefined) return inline;
      if (i + 1 >= argv.length) throw new Error(`${flag} needs a value`);
      return argv[++i];
    };
    switch (flag) {
      case '--url':
        options.url = value();
        break;
      case '--chrome':
        options.chrome = value();
        break;
      case '--arg':
        options.args.push(value());
        break;
      case '--headed':
        options.headless = false;
        break;
      case '--help':
      case '-h':
        options.help = true;
        break;
      default:
        if (argv[i].startsWith('--')) throw new Error(`unknown option ${flag}`);
        options.commands.push(argv[i]);
    }
  }
  return options;
}

async function main(argv) {
  let options;
  try {
    options = parseArguments(argv);
    if (options.help) return console.log(USAGE);
    if (!options.url) throw new Error('--url is required');
  } catch (error) {
    console.error(`${error.message}\n${USAGE}`);
    process.exitCode = 2;
    return;
  }

  let driver;
  let failed = false;
  // Ctrl+C, a kill or a reader gone (`| head`): close Chrome and remove its profile first, as a
  // finished run does, or each such run leaves a profile in the temporary folder
  let stopping = false;
  const stop = (why, code) => {
    if (stopping) return;
    stopping = true;
    console.error(`web-driver: ${why}; closing Chrome`);
    void Promise.resolve(driver?.close()).finally(() => process.exit(code));
  };
  process.once('SIGINT', () => stop('interrupted', 130));
  process.once('SIGTERM', () => stop('terminated', 143));
  // every later write fails too, so this stays subscribed
  process.stdout.on('error', () => stop('its output was closed', 1));
  try {
    driver = await launch({ url: options.url, chrome: options.chrome, headless: options.headless, args: options.args });
    await driver.ready();
    const run = async (command) => {
      try {
        const output = await driver.console(command);
        console.log(JSON.stringify({ command, ok: true, output }));
      } catch (error) {
        failed = true;
        console.log(JSON.stringify({ command, ok: false, error: error.message }));
      }
    };
    if (options.commands.length > 0) {
      for (const command of options.commands) await run(command);
    } else {
      for await (const line of readline.createInterface({ input: process.stdin })) {
        if (line.trim() !== '') await run(line.trim());
      }
    }
  } catch (error) {
    console.error(`web-driver: ${error.message}`);
    failed = true;
  } finally {
    await driver?.close();
  }
  process.exitCode = failed ? 1 : 0;
}

const here = (file) => {
  try {
    return realpathSync(file);
  } catch {
    return path.resolve(file);
  }
};
if (process.argv[1] && here(process.argv[1]) === here(fileURLToPath(import.meta.url))) {
  await main(process.argv.slice(2));
}
