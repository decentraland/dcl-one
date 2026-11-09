// Saved-outfit pictures on web, in IndexedDB: one record per [account, slot].

const DB_NAME = 'dcl-outfit-thumbnails'
const DB_VERSION = 1
const THUMBNAILS = 'thumbnails'
const BY_ACCOUNT = 'account'

function openDb() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION)
    req.onupgradeneeded = () => {
      const db = req.result
      if (!db.objectStoreNames.contains(THUMBNAILS)) {
        const store = db.createObjectStore(THUMBNAILS, { keyPath: ['account', 'slot'] })
        store.createIndex(BY_ACCOUNT, 'account')
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

// => JSON [{ slot, thumbnail }]
export async function outfitThumbnailsRead(account) {
  const db = await openDb()
  try {
    const tx = db.transaction(THUMBNAILS, 'readonly')
    const records = await request(tx.objectStore(THUMBNAILS).index(BY_ACCOUNT).getAll(account.toLowerCase()))
    return JSON.stringify(records.map(({ slot, thumbnail }) => ({ slot, thumbnail })))
  } finally {
    db.close()
  }
}

// thumbnail: a data URL, or null to remove the slot's
export async function outfitThumbnailWrite(account, slot, thumbnail) {
  const db = await openDb()
  try {
    const tx = db.transaction(THUMBNAILS, 'readwrite')
    const store = tx.objectStore(THUMBNAILS)
    const key = [account.toLowerCase(), slot]
    if (thumbnail == null) store.delete(key)
    else store.put({ account: key[0], slot, thumbnail })
    await complete(tx)
  } finally {
    db.close()
  }
}
