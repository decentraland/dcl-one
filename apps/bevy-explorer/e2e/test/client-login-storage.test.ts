import assert from 'node:assert/strict'
import { test } from 'node:test'
import { readFile } from 'node:fs/promises'
const source = await readFile(new URL('../../../../rendering/bevy-explorer/source/crates/platform/src/web_client_logins.js', import.meta.url), 'utf8')
const { clientLoginsRead, clientLoginsWrite } = await import('data:text/javascript;base64,' + Buffer.from(source).toString('base64'))
const browserGlobals = globalThis as unknown as { indexedDB?: unknown }
const tick = () => new Promise(resolve => setImmediate(resolve))

// Readwrite transactions serialize across connections and only commit pending writes on complete.
function database() {
  let durable: string | undefined
  let quarantine: string | undefined
  const transactions: any[] = []
  const start = () => queueMicrotask(() => transactions[0]?.request?.onsuccess())
  const db = {
    close() {},
    transaction() {
      const tx: any = {
        objectStore: () => ({
          get() {
            tx.request = { get result() { return durable } }
            if (transactions[0] === tx) start()
            return tx.request
          },
          put(value: string, key: string) { if (key === 'corrupt') tx.quarantine = value; else tx.pending = value },
        }),
        abort() {
          assert.equal(transactions[0], tx)
          transactions.shift()
          tx.onabort()
          start()
        },
      }
      transactions.push(tx)
      return tx
    },
  }
  return {
    seed(value: string) { durable = value },
    quarantined() { return quarantine },
    api: { open() {
      const request: any = { result: db }
      queueMicrotask(() => request.onsuccess())
      return request
    } },
    commit() {
      const tx = transactions.shift()
      assert.ok(tx)
      if (tx.pending !== undefined) durable = tx.pending
      if (tx.quarantine !== undefined) quarantine = tx.quarantine
      tx.oncomplete()
      start()
    },
    abort() { transactions[0].abort() },
  }
}
const event = (id: string, at = Date.now()) => ({ login_id: id, occurred_at: at, wallet: 'alice', endpoint: 'https://example.org/v1/client-login' })
const delta = (additions: ReturnType<typeof event>[] = [], removals: string[] = []) => JSON.stringify({ additions, removals })

async function withDatabase(run: (db: ReturnType<typeof database>, finish: (promise: Promise<any>) => Promise<any>) => Promise<void>) {
  const previous = browserGlobals.indexedDB
  const db = database()
  browserGlobals.indexedDB = db.api
  const finish = async (promise: Promise<any>) => { await tick(); db.commit(); return promise }
  try { await run(db, finish) } finally { browserGlobals.indexedDB = previous }
}

test('login queue saves await commit and aborted transactions preserve previous records', async () => {
  await withDatabase(async (db, finish) => {
    let committed = false
    const saving = clientLoginsWrite(delta([event('a')])).then(() => { committed = true })
    await tick()
    assert.equal(committed, false)
    db.commit()
    await saving
    assert.equal(committed, true)
    const failed = assert.rejects(clientLoginsWrite(delta([event('b')], ['a'])), /aborted/)
    await tick()
    db.abort()
    await failed
    const restarted = JSON.parse(await finish(clientLoginsRead()))
    assert.deepEqual(restarted.map((item: any) => item.login_id), ['a'])
  })
})

test('two runtimes retain both logins after restart and ack cannot erase or resurrect another event', async () => {
  await withDatabase(async (_db, finish) => {
    assert.deepEqual(JSON.parse(await finish(clientLoginsRead())), []) // runtime A
    assert.deepEqual(JSON.parse(await finish(clientLoginsRead())), []) // runtime B
    const a = event('a'), b = event('b')
    await finish(clientLoginsWrite(delta([a])))
    // B has a stale empty view: startup cannot overwrite A's newly persisted occurrence.
    await finish(clientLoginsWrite(delta()))
    await finish(clientLoginsWrite(delta([b])))
    assert.deepEqual(JSON.parse(await finish(clientLoginsRead())).map((item: any) => item.login_id).sort(), ['a', 'b'])
    await finish(clientLoginsWrite(delta([], ['a'])))
    // Retrying an uncertain save must not reinsert an event acknowledged by another runtime.
    await finish(clientLoginsWrite(delta([a])))
    assert.deepEqual(JSON.parse(await finish(clientLoginsRead())).map((item: any) => item.login_id), ['b'])
  })
})

test('concurrent writers merge and the shared store caps pending events across runtimes', async () => {
  await withDatabase(async (db, finish) => {
    const now = Date.now()
    const writes = [clientLoginsWrite(delta([event('a', now)])), clientLoginsWrite(delta([event('b', now)]))]
    await tick(); db.commit(); await tick(); db.commit()
    await Promise.all(writes)
    for (let i = 0; i < 40; i++) await finish(clientLoginsWrite(delta([event('new-' + i, now + i + 1)])))
    const rows = JSON.parse(await finish(clientLoginsRead()))
    assert.equal(rows.length, 32)
    assert.equal(rows.some((item: any) => item.login_id === 'a'), false)
    await finish(clientLoginsWrite(delta([event('a', now)])))
    assert.equal(JSON.parse(await finish(clientLoginsRead())).some((item: any) => item.login_id === 'a'), false)
  })
})


test('empty, corrupt and mixed-version records recover without discarding valid occurrences', async () => {
  await withDatabase(async (db, finish) => {
    for (const raw of ['', '{', 'null', '{"events":null,"removed":{}}']) {
      db.seed(raw)
      assert.deepEqual(JSON.parse(await finish(clientLoginsRead())), [])
      assert.equal(db.quarantined(), raw)
    }
    const valid = event('good')
    const mixed = JSON.stringify([valid, null, { login_id: 'bad', occurred_at: Date.now() }])
    db.seed(mixed)
    assert.deepEqual(JSON.parse(await finish(clientLoginsRead())), [valid])
    assert.equal(db.quarantined(), mixed)
    await finish(clientLoginsWrite(delta([event('new')])))
    assert.deepEqual(JSON.parse(await finish(clientLoginsRead())).map((row: any) => row.login_id).sort(), ['good', 'new'])
  })
})

test('browser storage policy stays aligned with the shared Rust limits', async () => {
  const rust = await readFile(new URL('../../../../rendering/bevy-explorer/source/crates/platform/src/client_login_store.rs', import.meta.url), 'utf8')
  assert.match(rust, /MAX_AGE_MS: u64 = 24 \* 60 \* 60 \* 1000/)
  assert.match(rust, /FUTURE_TOLERANCE_MS: u64 = 5 \* 60 \* 1000/)
  assert.match(rust, /MAX_PENDING: usize = 32/)
  assert.match(source, /MAX_AGE_MS = 86_400_000/)
  assert.match(source, /TOMBSTONE_MS = MAX_AGE_MS \+ 300_000/)
  assert.match(source, /MAX_PENDING = 32/)
})
