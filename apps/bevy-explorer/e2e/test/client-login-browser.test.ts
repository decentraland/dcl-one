import assert from 'node:assert/strict'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { test } from 'node:test'
import { serveWebApp } from '../driver/serve.ts'
import { WebConsole, findChrome } from '../driver/web.ts'

const chrome = await findChrome()
if (!chrome && process.env.DCL_E2E_REQUIRE_CHROME === '1') throw new Error('DCL_E2E_REQUIRE_CHROME=1 but no Chrome was found')

test('real IndexedDB preserves independent page occurrences, acknowledgment and restart recovery',
  { skip: chrome ? false : 'no Chrome installed' }, async () => {
    const root = await mkdtemp(path.join(tmpdir(), 'dcl-login-browser-'))
    const source = await readFile(new URL('../../../../rendering/bevy-explorer/source/crates/platform/src/web_client_logins.js', import.meta.url), 'utf8')
    await writeFile(path.join(root, 'storage.js'), source)
    await writeFile(path.join(root, 'client.html'), `<script type="module">
      import { clientLoginsRead, clientLoginsWrite } from './storage.js';
      window.queue = { read: clientLoginsRead, write: clientLoginsWrite };
    </script>`)
    await writeFile(path.join(root, 'index.html'), `<script>
      const frame = () => new Promise(resolve => {
        const element = document.createElement('iframe');
        element.src = 'client.html';
        element.onload = () => resolve(element);
        document.body.append(element);
      });
      window.engine_console_command = async () => {
        const a = await frame(), b = await frame();
        const first = await Promise.all([a.contentWindow.queue.read(), b.contentWindow.queue.read()]);
        const eventA = { login_id: 'a', occurred_at: Date.now(), wallet: 'alice', endpoint: 'https://example.org/v1/client-login' };
        const eventB = { login_id: 'b', occurred_at: Date.now(), wallet: 'alice', endpoint: 'https://example.org/v1/client-login' };
        const delta = (additions = [], removals = []) => JSON.stringify({ additions, removals });
        await Promise.all([
          a.contentWindow.queue.write(delta([eventA])),
          b.contentWindow.queue.write(delta([eventB]))
        ]);
        // A stale startup has no additions, so it cannot erase the other context's occurrence.
        await b.contentWindow.queue.write(delta());
        a.remove(); b.remove();
        const restarted = await frame(), observer = await frame();
        const afterRestart = JSON.parse(await restarted.contentWindow.queue.read());
        await restarted.contentWindow.queue.write(delta([], ['a']));
        await observer.contentWindow.queue.write(delta([eventA]));
        const afterAck = JSON.parse(await observer.contentWindow.queue.read());
        // Abort an actual IndexedDB transaction, before its mutation can commit.
        const proto = observer.contentWindow.IDBDatabase.prototype;
        const original = proto.transaction;
        proto.transaction = function(...args) {
          const transaction = original.apply(this, args);
          queueMicrotask(() => transaction.abort());
          return transaction;
        };
        let aborted = false;
        try { await observer.contentWindow.queue.write(delta([], ['b'])); }
        catch { aborted = true; }
        proto.transaction = original;
        observer.remove(); restarted.remove();
        const final = await frame();
        const afterAbortRestart = JSON.parse(await final.contentWindow.queue.read());
        const seed = raw => new Promise((resolve, reject) => {
          const open = indexedDB.open('dcl-client-logins', 1);
          open.onsuccess = () => {
            const db = open.result, tx = db.transaction('queue', 'readwrite');
            tx.objectStore('queue').put(raw, 'pending');
            tx.oncomplete = () => { db.close(); resolve(); };
            tx.onerror = () => reject(tx.error);
          };
        });
        await seed('');
        const emptyRecovered = JSON.parse(await final.contentWindow.queue.read());
        await seed(JSON.stringify([eventB, null, { login_id: 'bad', occurred_at: Date.now() }]));
        const mixedRecovered = JSON.parse(await final.contentWindow.queue.read());
        await final.contentWindow.queue.write(JSON.stringify({ additions: [null, { login_id: 'bad' }, eventA], removals: ['b'] }));
        const afterInvalidAddition = JSON.parse(await final.contentWindow.queue.read());
        final.remove();
        return JSON.stringify({ first: first.map(JSON.parse), afterRestart, afterAck, aborted, afterAbortRestart, emptyRecovered, mixedRecovered, afterInvalidAddition });
      };
    </script>`)
    const server = await serveWebApp(root, 0)
    let web: WebConsole | undefined
    try {
      web = await WebConsole.launch({ url: server.url, chrome, headless: true, readyTimeoutMs: 60_000, commandTimeoutMs: 20_000 })
      const result = JSON.parse(await web.run('/login_storage')) as {
        first: unknown[][]; afterRestart: { login_id: string }[]; afterAck: { login_id: string }[];
        aborted: boolean; afterAbortRestart: { login_id: string }[]; emptyRecovered: unknown[]; mixedRecovered: { login_id: string }[]; afterInvalidAddition: { login_id: string }[]
      }
      assert.deepEqual(result.first, [[], []])
      assert.deepEqual(result.afterRestart.map(event => event.login_id).sort(), ['a', 'b'])
      assert.deepEqual(result.afterAck.map(event => event.login_id), ['b'])
      assert.equal(result.aborted, true)
      assert.deepEqual(result.afterAbortRestart.map(event => event.login_id), ['b'])
      assert.deepEqual(result.emptyRecovered, [])
      assert.deepEqual(result.mixedRecovered.map(event => event.login_id), ['b'])
      assert.deepEqual(result.afterInvalidAddition.map(event => event.login_id), ['a'])
    } finally {
      await web?.close()
      await server.close()
      await rm(root, { recursive: true, force: true })
    }
  })
