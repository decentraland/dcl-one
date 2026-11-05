import assert from 'node:assert/strict'
import { test } from 'node:test'
import { readFile } from 'node:fs/promises'
const source = await readFile(new URL('../../../../rendering/bevy-explorer/source/crates/platform/src/web_client_logins.js', import.meta.url), 'utf8')
const { clientLoginsRead, clientLoginsWrite } = await import('data:text/javascript;base64,' + Buffer.from(source).toString('base64'))
const browserGlobals = globalThis as unknown as { indexedDB?: unknown }

// Resolve each browser transaction explicitly: a successful put request alone is not a commit.
test('login queue saves await commit and surface transaction aborts', async () => {
  const previous = browserGlobals.indexedDB
  let transaction: any
  let saved: string | undefined
  let closed = 0
  const db = {
    close() { closed++ },
    transaction() {
      transaction = {
        objectStore: () => ({ put(value: string) { saved = value } }),
      }
      return transaction
    },
  }
  browserGlobals.indexedDB = { open() {
    const request: any = { result: db }
    queueMicrotask(() => request.onsuccess())
    return request
  } } as any
  try {
    let committed = false
    const saving = clientLoginsWrite('[{"login_id":"same-event"}]').then(() => { committed = true })
    await new Promise(resolve => setImmediate(resolve))
    assert.equal(committed, false)
    assert.equal(saved, '[{"login_id":"same-event"}]')
    transaction.oncomplete()
    await saving
    assert.equal(committed, true)
    assert.equal(closed, 1)
    const failed = clientLoginsWrite('[]')
    const rejected = assert.rejects(failed, /aborted/)
    await new Promise(resolve => setImmediate(resolve))
    transaction.onabort()
    await rejected
    assert.equal(closed, 2)
  } finally { browserGlobals.indexedDB = previous }
})

test('login queue reads resume persisted JSON and default only a missing record', async () => {
  const previous = browserGlobals.indexedDB
  let value: string | undefined = '[{"login_id":"pending"}]'
  const db = {
    close() {},
    transaction: () => ({ objectStore: () => ({ get() {
      const request: any = { result: value }
      queueMicrotask(() => request.onsuccess())
      return request
    } }) }),
  }
  browserGlobals.indexedDB = { open() {
    const request: any = { result: db }
    queueMicrotask(() => request.onsuccess())
    return request
  } } as any
  try {
    assert.deepEqual(JSON.parse(await clientLoginsRead()), [{ login_id: 'pending' }])
    value = undefined
    assert.deepEqual(JSON.parse(await clientLoginsRead()), [])
  } finally { browserGlobals.indexedDB = previous }
})
