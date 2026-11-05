// Local DM history on web, in IndexedDB. Stored plain: the origin is the boundary here, and the
// account address that native derives its key from already sits in this origin's storage.
//
// `messages` holds one record per message or reaction (autoincrement id = arrival order),
// indexed by [account, partner].

const DB_NAME = 'dcl-dm-history'
const DB_VERSION = 1
const MESSAGES = 'messages'
const BY_CONVERSATION = 'conversation'

function openDb() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION)
    req.onupgradeneeded = () => {
      const db = req.result
      if (!db.objectStoreNames.contains(MESSAGES)) {
        const store = db.createObjectStore(MESSAGES, { keyPath: 'id', autoIncrement: true })
        store.createIndex(BY_CONVERSATION, ['account', 'partner'])
      }
    }
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error)
  })
}

function request(req) {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error)
  })
}

function complete(tx) {
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve()
    tx.onerror = () => reject(tx.error)
    tx.onabort = () => reject(tx.error)
  })
}

async function withDb(fn) {
  const db = await openDb()
  try {
    return await fn(db)
  } finally {
    db.close()
  }
}

const conversationKey = (account, partner) => [account.toLowerCase(), partner.toLowerCase()]

export async function dmHistoryAppend(account, partner, entryJson) {
  const entry = JSON.parse(entryJson)
  const [a, p] = conversationKey(account, partner)
  await withDb(async (db) => {
    const tx = db.transaction(MESSAGES, 'readwrite')
    tx.objectStore(MESSAGES).add({ account: a, partner: p, ...entry })
    await complete(tx)
  })
}

export async function dmHistoryRead(account, partner) {
  const rows = await withDb((db) =>
    request(db.transaction(MESSAGES, 'readonly').objectStore(MESSAGES).index(BY_CONVERSATION).getAll(conversationKey(account, partner)))
  )
  return JSON.stringify(rows.map(({ id, account, partner, ...entry }) => entry))
}

export async function dmHistoryDelete(account, partner) {
  await withDb(async (db) => {
    const tx = db.transaction(MESSAGES, 'readwrite')
    const keys = await request(tx.objectStore(MESSAGES).index(BY_CONVERSATION).getAllKeys(conversationKey(account, partner)))
    for (const key of keys) tx.objectStore(MESSAGES).delete(key)
    await complete(tx)
  })
}

// keeps only the last `keep` messages of a conversation and every record after the oldest of
// them (the reactions to them), and never more than the last `keepRecords` records (reactions
// alone can grow a conversation); ids are arrival order
export async function dmHistoryTrim(account, partner, keep, keepRecords) {
  await withDb(async (db) => {
    const tx = db.transaction(MESSAGES, 'readwrite')
    const rows = await request(tx.objectStore(MESSAGES).index(BY_CONVERSATION).getAll(conversationKey(account, partner)))
    // the oldest message to keep is the `keep`-th from the end; fewer messages than that keeps all
    let seen = 0
    let firstKept = keep === 0 ? rows.length : 0
    for (let i = rows.length - 1; i >= 0 && keep > 0; i--) {
      if (rows[i].reaction == null && ++seen === keep) {
        firstKept = i
        break
      }
    }
    firstKept = Math.max(firstKept, rows.length - keepRecords)
    for (const row of rows.slice(0, firstKept)) tx.objectStore(MESSAGES).delete(row.id)
    await complete(tx)
  })
}
