// The engine owns this queue. No signing keys or authentication chains are stored here.
function openDb() {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open('dcl-client-logins', 1)
    request.onupgradeneeded = () => request.result.createObjectStore('queue')
    request.onsuccess = () => resolve(request.result)
    request.onerror = () => reject(request.error)
  })
}

export async function clientLoginsRead() {
  const db = await openDb()
  try {
    return await new Promise((resolve, reject) => {
      const request = db.transaction('queue', 'readonly').objectStore('queue').get('pending')
      request.onsuccess = () => resolve(request.result ?? '[]')
      request.onerror = () => reject(request.error)
    })
  } finally { db.close() }
}

export async function clientLoginsWrite(json) {
  const db = await openDb()
  try {
    await new Promise((resolve, reject) => {
      const transaction = db.transaction('queue', 'readwrite')
      transaction.objectStore('queue').put(json, 'pending')
      transaction.oncomplete = () => resolve()
      transaction.onerror = () => reject(transaction.error)
      transaction.onabort = () => reject(transaction.error ?? new Error('login queue save aborted'))
    })
  } finally { db.close() }
}
