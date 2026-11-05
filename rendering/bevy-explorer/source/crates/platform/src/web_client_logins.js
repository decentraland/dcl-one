// Mutations merge under one IndexedDB transaction, including across Explorer tabs.
const MAX_AGE_MS = 86_400_000
const TOMBSTONE_MS = MAX_AGE_MS + 30_000
const MAX_PENDING = 32

function openDb() {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open('dcl-client-logins', 1)
    request.onupgradeneeded = () => request.result.createObjectStore('queue')
    request.onsuccess = () => resolve(request.result)
    request.onerror = () => reject(request.error)
  })
}

function prune(store, now) {
  for (const [id, at] of Object.entries(store.removed)) {
    if (now - at >= TOMBSTONE_MS) delete store.removed[id]
  }
  for (const [id, event] of Object.entries(store.events)) {
    if (!Number.isSafeInteger(event.occurred_at) || now - event.occurred_at >= MAX_AGE_MS) {
      delete store.events[id]
      store.removed[id] = now
    }
  }
  const oldest = Object.entries(store.events).sort(([a, x], [b, y]) =>
    x.occurred_at - y.occurred_at || a.localeCompare(b))
  for (const [id] of oldest.slice(0, Math.max(0, oldest.length - MAX_PENDING))) {
    delete store.events[id]
    store.removed[id] = now
  }
}

async function mutate(changes, readEvents) {
  const db = await openDb()
  try {
    return await new Promise((resolve, reject) => {
      const transaction = db.transaction('queue', 'readwrite')
      let result
      let failure
      transaction.oncomplete = () => resolve(result)
      transaction.onerror = () => reject(failure ?? transaction.error)
      transaction.onabort = () => reject(failure ?? transaction.error ?? new Error('login queue save aborted'))
      const records = transaction.objectStore('queue')
      const request = records.get('pending')
      request.onsuccess = () => {
        try {
          const old = JSON.parse(request.result ?? '[]')
          const store = Array.isArray(old)
            ? { events: Object.fromEntries(old.map(event => [event.login_id, event])), removed: {} }
            : old
          const now = Date.now()
          prune(store, now)
          for (const event of changes.additions) {
            if (!Object.hasOwn(store.removed, event.login_id) && !Object.hasOwn(store.events, event.login_id)) {
              store.events[event.login_id] = event
            }
          }
          for (const id of changes.removals) {
            delete store.events[id]
            store.removed[id] = now
          }
          prune(store, now)
          records.put(JSON.stringify(store), 'pending')
          result = JSON.stringify(readEvents ? Object.values(store.events) : Object.keys(store.events))
        } catch (error) {
          failure = error
          transaction.abort()
        }
      }
    })
  } finally { db.close() }
}

export async function clientLoginsRead() {
  return mutate({ additions: [], removals: [] }, true)
}

export async function clientLoginsWrite(json) {
  return mutate(JSON.parse(json), false)
}
