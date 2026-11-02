// Tests for the web driver (README.md): `node --test local-harness/web-driver/driver.test.mjs`
// from the engine's source directory (Node 24 reads a directory argument as a file, so name it).
//
// The end-to-end cases open fixture/index.html (a shell with the app, fixture/app.html, in an
// iframe placed away from (0, 0), as the web client's shell does) in a real Chrome, twice: with
// app.html served with Document-Isolation-Policy, which makes it an out-of-process iframe, and
// without, when it shares the page's process. The driver offsets a click by where the frame is
// in both, through different CDP calls, so both are proven.
//
// They are skipped when no Chrome is installed, unless DCL_WEB_DRIVER_REQUIRE_CHROME=1 (CI sets
// it), and the ones that load the real HUD commands (react-web/src/lib/hudAutomation.ts, with
// its types stripped as each request is served) need a Node with module.stripTypeScriptTypes.

import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';
import http from 'node:http';
import * as nodeModule from 'node:module';
import path from 'node:path';
import { after, before, beforeEach, describe, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { acceptsBinding, findChrome, launch } from './driver.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const FIXTURE = path.join(HERE, 'fixture');
const HUD_SOURCE = path.resolve(HERE, '../../react-web/src/lib/hudAutomation.ts');

const chrome = findChrome();
const noChrome =
  chrome == null && process.env.DCL_WEB_DRIVER_REQUIRE_CHROME !== '1' ? 'Chrome is not installed (set CHROME_PATH)' : false;
const noStrip = typeof nodeModule.stripTypeScriptTypes === 'function' ? false : 'this Node has no module.stripTypeScriptTypes';

// Where fixture/app.html puts its canvas, in the frame's own pixels.
const CANVAS = { left: 61, top: 47 };

// --- the unit cases ---

describe('acceptsBinding', () => {
  const origin = 'http://127.0.0.1:8080';

  test('accepts a default context of the origin the page was opened at', () => {
    assert.equal(acceptsBinding({ isDefault: true, origin }, origin), true);
  });

  test('refuses another origin', () => {
    assert.equal(acceptsBinding({ isDefault: true, origin: 'http://127.0.0.1:8081' }, origin), false);
    assert.equal(acceptsBinding({ isDefault: true, origin: 'https://example.com' }, origin), false);
  });

  test('refuses an isolated world, even of the right origin', () => {
    assert.equal(acceptsBinding({ isDefault: false, origin }, origin), false);
  });

  test('refuses a context it does not know', () => {
    assert.equal(acceptsBinding(undefined, origin), false);
  });
});

describe('findChrome', () => {
  test('takes CHROME_PATH when a file is there, and nothing when it is not', () => {
    assert.equal(findChrome({ CHROME_PATH: process.execPath }), process.execPath);
    assert.equal(findChrome({ CHROME_PATH: path.join(HERE, 'no-such-chrome') }), null);
  });
});

// --- the page and what it is served ---

// A server for the fixture: the shell, app.html (isolated or not) and the HUD commands, which
// are hudAutomation.ts with its types stripped, as the page loads it.
async function serve({ isolated }) {
  const server = http.createServer((request, response) => {
    const { pathname } = new URL(request.url, 'http://localhost');
    const reply = (status, type, body, headers = {}) =>
      response.writeHead(status, { 'Content-Type': type, 'Cache-Control': 'no-store', ...headers }).end(body);
    try {
      if (pathname === '/' || pathname === '/index.html') {
        return reply(200, 'text/html', readFileSync(path.join(FIXTURE, 'index.html')));
      }
      if (pathname === '/app.html') {
        const headers = isolated ? { 'Document-Isolation-Policy': 'isolate-and-require-corp' } : {};
        return reply(200, 'text/html', readFileSync(path.join(FIXTURE, 'app.html')), headers);
      }
      if (pathname === '/hudAutomation.js') {
        return reply(200, 'text/javascript', nodeModule.stripTypeScriptTypes(readFileSync(HUD_SOURCE, 'utf8')));
      }
      reply(404, 'text/plain', 'not found');
    } catch (error) {
      reply(500, 'text/plain', String(error));
    }
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  return {
    url: `http://127.0.0.1:${server.address().port}`,
    close: () =>
      new Promise((resolve) => {
        server.close(resolve);
        server.closeAllConnections();
      })
  };
}

// Registers the hooks that serve the fixture and open it in Chrome; the result's `driver` is
// there once they have run.
function openPage({ isolated, mode, query = '' }) {
  const page = {};
  before(
    async () => {
      page.server = await serve({ isolated });
      page.driver = await launch({ url: `${page.server.url}/?mode=${mode}${query}`, chrome });
      await page.driver.ready({ timeout: 60_000 });
    },
    { timeout: 120_000 }
  );
  after(async () => {
    await page.driver?.close();
    await page.server?.close();
  });
  return page;
}

const json = async (driver, line) => JSON.parse(await driver.console(line));
const events = (driver) => json(driver, '/fixture_events');
const reset = (driver, value) => driver.console(value === undefined ? '/fixture_reset' : `/fixture_reset ${JSON.stringify({ value })}`);
// a request of the wire protocol, sent by the fixture's own bare client (?mode=protocol)
const send = (driver, request) => driver.console(`/fixture_input ${JSON.stringify(request)}`);
const KEY = {
  b: { code: 'KeyB', key: 'b', keyCode: 66, text: 'b', location: 0 },
  escape: { code: 'Escape', key: 'Escape', keyCode: 27, location: 0 },
  enter: { code: 'Enter', key: 'Enter', keyCode: 13, text: '\r', location: 0 },
  control: { code: 'ControlLeft', key: 'Control', keyCode: 17, location: 1 },
  shift: { code: 'ShiftLeft', key: 'Shift', keyCode: 16, location: 1 }
};

// The events an action causes.
async function during(driver, action) {
  const before = (await events(driver)).length;
  await action();
  return (await events(driver)).slice(before);
}

const trusted = (list) => list.every((event) => event.trusted === true);
const summary = (list) => list.map((event) => `${event.type}:${event.code ?? event.target}`);

for (const isolated of [true, false]) {
  const kind = isolated ? 'app frame out of process (Document-Isolation-Policy)' : 'app frame in process';

  // The wire protocol, with a client written in the fixture that does not use hudAutomation.ts.
  describe(`trusted input, ${kind}`, { skip: noChrome }, () => {
    const page = openPage({ isolated, mode: 'protocol' });
    beforeEach(() => reset(page.driver));

    test(isolated ? 'the app frame is an iframe target of its own' : 'the app frame shares the page', async () => {
      const state = await json(page.driver, '/fixture_state');
      assert.equal(state.crossOriginIsolated, isolated, 'Document-Isolation-Policy isolates the frame, and only when sent');
      const iframes = page.driver.frameTargets().filter((target) => target.type === 'iframe');
      assert.equal(iframes.length, isolated ? 1 : 0);
    });

    test('a click lands on its target, offset by where the frame is', async () => {
      const point = JSON.parse(await send(page.driver, { op: 'click', at: '#save', button: 'left' }));
      const seen = await events(page.driver);
      assert.deepEqual(summary(seen), ['pointerdown:save', 'mousedown:save', 'mouseup:save', 'click:save']);
      assert.ok(trusted(seen));
      assert.ok(Math.abs(seen[3].clientX - point.x) <= 1 && Math.abs(seen[3].clientY - point.y) <= 1);
    });

    test('a click at a position in the frame is exactly there, past the frame\'s border', async () => {
      const seen = await during(page.driver, () =>
        send(page.driver, { op: 'click', x: CANVAS.left + 100, y: CANVAS.top + 80, button: 'left' })
      );
      const down = seen.find((event) => event.type === 'pointerdown');
      assert.ok(trusted(seen));
      assert.equal(down.target, 'canvas');
      assert.deepEqual([down.offsetX, down.offsetY], [100, 80]);
      assert.deepEqual([down.clientX, down.clientY], [CANVAS.left + 100, CANVAS.top + 80]);
      assert.equal(down.buttons, 1);
    });

    test('a right click and a middle click press their own buttons', async () => {
      const right = await during(page.driver, () => send(page.driver, { op: 'click', at: '#save', button: 'right' }));
      assert.ok(trusted(right));
      assert.equal(right.find((event) => event.type === 'mousedown').button, 2);
      assert.ok(right.some((event) => event.type === 'contextmenu'));
      assert.ok(!right.some((event) => event.type === 'click'));

      const middle = await during(page.driver, () => send(page.driver, { op: 'click', at: '#save', button: 'middle' }));
      assert.ok(trusted(middle));
      assert.equal(middle.find((event) => event.type === 'mousedown').button, 1);
      assert.equal(middle.find((event) => event.type === 'auxclick').button, 1);
      assert.ok(!middle.some((event) => event.type === 'click'));
    });

    test('a key with text types it, and one without does not', async () => {
      await reset(page.driver, '');
      await send(page.driver, { op: 'click', at: '#search', button: 'left' });

      const typed = await during(page.driver, () => send(page.driver, { op: 'key', ...KEY.b }));
      assert.deepEqual(summary(typed), ['keydown:KeyB', 'keypress:KeyB', 'beforeinput:search', 'input:search', 'keyup:KeyB']);
      assert.ok(trusted(typed));
      assert.equal(typed[0].key, 'b');
      assert.equal(typed.find((event) => event.type === 'input').value, 'b');

      const silent = await during(page.driver, () => send(page.driver, { op: 'key', ...KEY.escape }));
      assert.deepEqual(summary(silent), ['keydown:Escape', 'keyup:Escape']);
      assert.ok(trusted(silent));
      assert.equal((await json(page.driver, '/fixture_state')).value, 'b');

      const enter = await during(page.driver, () => send(page.driver, { op: 'key', ...KEY.enter }));
      assert.deepEqual(summary(enter).slice(0, 2), ['keydown:Enter', 'keypress:Enter']);
    });

    test('a key sent as down is not released until it is sent as up', async () => {
      const down = await during(page.driver, () => send(page.driver, { op: 'key', ...KEY.b, state: 'down' }));
      assert.deepEqual(down.map((event) => event.type).filter((type) => type !== 'beforeinput' && type !== 'input'), ['keydown', 'keypress']);
      const up = await during(page.driver, () => send(page.driver, { op: 'key', ...KEY.b, state: 'up' }));
      assert.deepEqual(up.map((event) => event.type), ['keyup']);
      assert.ok(trusted(down) && trusted(up));
    });

    test('a held modifier is on every event after it, and a shortcut types nothing', async () => {
      await reset(page.driver, '');
      await send(page.driver, { op: 'click', at: '#search', button: 'left' });

      const seen = await during(page.driver, async () => {
        await send(page.driver, { op: 'key', ...KEY.control, state: 'down' });
        await send(page.driver, { op: 'key', ...KEY.b });
        await send(page.driver, { op: 'key', ...KEY.control, state: 'up' });
        await send(page.driver, { op: 'key', ...KEY.b });
      });
      const keys = seen.filter((event) => event.type === 'keydown' || event.type === 'keyup');
      assert.deepEqual(
        keys.map((event) => `${event.type}:${event.code}:${event.ctrlKey}`),
        [
          'keydown:ControlLeft:true',
          'keydown:KeyB:true',
          'keyup:KeyB:true',
          'keyup:ControlLeft:false',
          'keydown:KeyB:false',
          'keyup:KeyB:false'
        ]
      );
      const inputs = seen.filter((event) => event.type === 'input');
      assert.equal(inputs.length, 1, 'only the key pressed without Control types');
      assert.equal((await json(page.driver, '/fixture_state')).value, 'b');

      const shift = await during(page.driver, async () => {
        await send(page.driver, { op: 'key', ...KEY.shift, state: 'down' });
        await send(page.driver, { op: 'click', at: '#save', button: 'left' });
        await send(page.driver, { op: 'key', ...KEY.shift, state: 'up' });
        await send(page.driver, { op: 'click', at: '#save', button: 'left' });
      });
      assert.deepEqual(shift.filter((event) => event.type === 'click').map((event) => event.shiftKey), [true, false]);
    });

    test('inserted text goes into the focused field', async () => {
      await reset(page.driver, '');
      await send(page.driver, { op: 'click', at: '#search', button: 'left' });
      const seen = await during(page.driver, () => send(page.driver, { op: 'insertText', text: 'héllo ✓' }));
      const input = seen.find((event) => event.type === 'input');
      assert.ok(trusted(seen));
      assert.equal(input.inputType, 'insertText');
      assert.equal((await json(page.driver, '/fixture_state')).value, 'héllo ✓');
    });

    test('a request the driver cannot serve is answered with why', async () => {
      await assert.rejects(send(page.driver, { op: 'nope' }), /unknown trusted input op `nope`/);
      await assert.rejects(send(page.driver, { op: 'click', x: 10, y: 10, button: 'back' }), /unknown button `back`: use left, right or middle/);
      await assert.rejects(send(page.driver, { op: 'click', x: 'ten', y: 10, button: 'left' }), /needs numeric "x" and "y"/);
      await assert.rejects(send(page.driver, { op: 'insertText' }), /needs "text"/);
      assert.deepEqual(await events(page.driver), [], 'nothing reached the page');
    });

    test('the console rejects with the page\'s message', async () => {
      await assert.rejects(page.driver.console('/no_such_command'), /^Error: unknown command \/no_such_command$/);
    });
  });

  // The page's own commands (react-web/src/lib/hudAutomation.ts) with the driver answering them.
  describe(`HUD commands, ${kind}`, { skip: noChrome || noStrip }, () => {
    const page = openPage({ isolated, mode: 'hud' });
    beforeEach(() => reset(page.driver));

    test('/hud_click clicks the control with real input', async () => {
      const result = JSON.parse(await page.driver.console('/hud_click Save'));
      assert.equal(result.input, 'trusted');
      assert.equal(result.name, 'Save');
      const seen = await events(page.driver);
      assert.ok(trusted(seen));
      assert.deepEqual(seen.filter((event) => event.type === 'click').map((event) => event.target), ['save']);
    });

    test('/hud_fill replaces the text that is there', async () => {
      assert.equal((await json(page.driver, '/fixture_state')).value, 'old');
      const result = JSON.parse(await page.driver.console('/hud_fill {"name":"Search","text":"Genesis"}'));
      assert.equal(result.input, 'trusted');
      assert.equal(result.value, 'Genesis');
      assert.equal((await json(page.driver, '/fixture_state')).value, 'Genesis');
      const inputs = (await events(page.driver)).filter((event) => event.type === 'input');
      assert.ok(inputs.length > 0 && trusted(inputs));
      assert.equal(inputs.at(-1).value, 'Genesis');
    });

    test('/hud_fill with empty text clears the field', async () => {
      const result = JSON.parse(await page.driver.console('/hud_fill {"name":"Search","text":""}'));
      assert.equal(result.input, 'trusted');
      assert.equal(result.value, '');
      assert.equal((await json(page.driver, '/fixture_state')).value, '');
      assert.ok(trusted((await events(page.driver)).filter((event) => event.type === 'input')));
    });

    test('/press_key presses a key the page gets as a user\'s', async () => {
      await page.driver.console('/press_key KeyB');
      const keys = (await events(page.driver)).filter((event) => event.type.startsWith('key'));
      assert.deepEqual(keys.map((event) => `${event.type}:${event.key}`), ['keydown:b', 'keypress:b', 'keyup:b']);
      assert.ok(trusted(keys));
    });

    test('/press_key --state keeps a modifier down for the commands after it', async () => {
      await page.driver.console('/press_key ShiftLeft --state down');
      try {
        await page.driver.console('/hud_click Save');
      } finally {
        await page.driver.console('/press_key ShiftLeft --state up');
      }
      const click = (await events(page.driver)).find((event) => event.type === 'click');
      assert.equal(click.shiftKey, true);
    });

    test('/click_at clicks the world where it is told', async () => {
      await page.driver.console(`/click_at ${CANVAS.left + 100} ${CANVAS.top + 80}`);
      const down = (await events(page.driver)).find((event) => event.type === 'pointerdown');
      assert.equal(down.trusted, true);
      assert.equal(down.target, 'canvas');
      assert.deepEqual([down.offsetX, down.offsetY], [100, 80]);
    });

    test('/click_at --button right opens the context menu', async () => {
      await page.driver.console(`/click_at ${CANVAS.left + 100} ${CANVAS.top + 80} --button right`);
      const menu = (await events(page.driver)).find((event) => event.type === 'contextmenu');
      assert.equal(menu.trusted, true);
    });
  });
}

// A frame of another origin has the binding too, and must get nothing from it.
describe('a frame of another origin', { skip: noChrome }, () => {
  const page = {};
  before(
    async () => {
      page.other = await serve({ isolated: false });
      page.host = await serve({ isolated: false });
      const probe = encodeURIComponent(`${page.other.url}/app.html?mode=probe`);
      page.driver = await launch({ url: `${page.host.url}/?probe=${probe}`, chrome });
      await page.driver.ready({ timeout: 60_000 });
    },
    { timeout: 120_000 }
  );
  after(async () => {
    await page.driver?.close();
    await page.host?.close();
    await page.other?.close();
  });

  test('its call is ignored: no input, and no answer', async () => {
    let messages = [];
    for (let i = 0; i < 100 && messages.length === 0; i++) {
      messages = await json(page.driver, '/fixture_messages');
      if (messages.length === 0) await new Promise((resolve) => setTimeout(resolve, 100));
    }
    assert.equal(messages.length, 1, 'the frame reported back');
    assert.equal(messages[0].bound, true, 'the binding is there, as in every frame');
    assert.equal(messages[0].answered, false);
    assert.deepEqual(messages[0].events, []);
  });
});

describe('the command line', () => {
  function run(args, input = '') {
    return new Promise((resolve) => {
      const child = spawn(process.execPath, [path.join(HERE, 'driver.mjs'), ...args], { stdio: ['pipe', 'pipe', 'pipe'] });
      let stdout = '';
      let stderr = '';
      child.stdout.on('data', (chunk) => (stdout += chunk));
      child.stderr.on('data', (chunk) => (stderr += chunk));
      child.on('close', (code) => resolve({ code, stdout, stderr }));
      child.stdin.end(input);
    });
  }
  const lines = (stdout) => stdout.split('\n').filter(Boolean).map((line) => JSON.parse(line));

  test('needs a url', async () => {
    const { code, stderr } = await run(['/hud_list']);
    assert.equal(code, 2);
    assert.match(stderr, /--url is required[\s\S]*usage:/);
  });

  test('says when there is no Chrome to start', async () => {
    const { code, stderr } = await run(['--url', 'http://127.0.0.1:9/', '--chrome', path.join(HERE, 'no-such-chrome')]);
    assert.equal(code, 1);
    assert.match(stderr, /web-driver: could not start Chrome/);
  });

  describe('with Chrome', { skip: noChrome }, () => {
    const page = {};
    before(
      async () => {
        page.server = await serve({ isolated: true });
      },
      { timeout: 60_000 }
    );
    after(() => page.server?.close());

    test('runs the commands it is given and prints a line for each; exits 1 when one failed', { timeout: 120_000 }, async () => {
      const { code, stdout } = await run(['--url', `${page.server.url}/?mode=protocol`, '/fixture_state', '/no_such_command']);
      const [state, missing] = lines(stdout);
      assert.equal(state.command, '/fixture_state');
      assert.equal(state.ok, true);
      assert.equal(JSON.parse(state.output).crossOriginIsolated, true);
      assert.deepEqual(missing, { command: '/no_such_command', ok: false, error: 'unknown command /no_such_command' });
      assert.equal(code, 1);
    });

    test('reads the commands from stdin when it is given none; exits 0 when all passed', { timeout: 120_000 }, async () => {
      const { code, stdout } = await run(['--url', `${page.server.url}/?mode=protocol`], '/fixture_reset\n\n/fixture_events\n');
      assert.deepEqual(lines(stdout), [
        { command: '/fixture_reset', ok: true, output: 'reset' },
        { command: '/fixture_events', ok: true, output: '[]' }
      ]);
      assert.equal(code, 0);
    });
  });
});
