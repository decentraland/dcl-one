// Mutations merge under one IndexedDB transaction, including across Explorer tabs.
const MAX_AGE_MS = 86_400_000
const TOMBSTONE_MS = MAX_AGE_MS + 300_000
const MAX_PENDING = 32

function validEvent(event) {
  return event !== null && typeof event === 'object' &&
    ['login_id', 'wallet', 'endpoint'].every(key => typeof event[key] === 'string' && event[key].length > 0) &&
    Number.isSafeInteger(event.occurred_at) && event.occurred_at >= 0
}

function recover(raw) {
  let old
  let damaged = false
  try { old = JSON.parse(raw ?? '[]') } catch { damaged = true }
  const store = { events: Object.create(null), removed: Object.create(null) }
  let entries = []
  if (Array.isArray(old)) entries = old.map(event => [event?.login_id, event])
  else if (old && typeof old === 'object') {
    if (old.events && typeof old.events === 'object' && !Array.isArray(old.events)) entries = Object.entries(old.events)
    else damaged = true
    if (old.removed && typeof old.removed === 'object' && !Array.isArray(old.removed)) {
      for (const [id, at] of Object.entries(old.removed)) {
        if (Number.isSafeInteger(at) && at >= 0) store.removed[id] = at
        else damaged = true
      }
    } else damaged = true
  } else damaged = true
  for (const [id, event] of entries) {
    if (validEvent(event) && id === event.login_id) store.events[id] = event
    else damaged = true
  }
  return { store, damaged }
}

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
    x.occurred_at - y.occurred_at || (a < b ? -1 : a > b ? 1 : 0))
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
          const { store, damaged } = recover(request.result)
          if (damaged) {
            // One recovery snapshot, committed atomically with repaired valid records.
            records.put(request.result, 'corrupt')
            console.warn('recovered damaged login event storage; invalid records quarantined')
          }
          const now = Date.now()
          prune(store, now)
          for (const event of changes.additions) {
            if (!validEvent(event)) {
              console.warn('discarded invalid login event addition')
              continue
            }
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
