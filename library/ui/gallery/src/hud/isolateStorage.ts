// The HUD canvas runs on its host's origin: on a deployment that is the origin /play serves the Explorer from, and the
// canvas is a same-origin iframe, so the HUD code takes it for the Explorer's web shell (explorer-core/lib/shell.ts)
// and would keep its preferences, recents and analytics queue in that origin's storage, where real players' HUD reads
// them. Before any HUD module loads, this document's storage is replaced with storage that lives in this page alone:
// Web Storage and cookies in memory, and IndexedDB and Cache Storage refused, which the HUD's prefs treat as values
// that last the session. That covers the HUD's preferences, Web Storage and cookies; BroadcastChannel and same-origin
// access through window.parent stay available to the canvas.

class MemoryStorage implements Storage {
  #items = new Map<string, string>()
  get length(): number {
    return this.#items.size
  }
  clear(): void {
    this.#items.clear()
  }
  getItem(key: string): string | null {
    return this.#items.get(String(key)) ?? null
  }
  key(index: number): string | null {
    return [...this.#items.keys()][index] ?? null
  }
  removeItem(key: string): void {
    this.#items.delete(String(key))
  }
  setItem(key: string, value: string): void {
    this.#items.set(String(key), String(value))
  }
}

const refused = (api: string) => () => {
  throw new DOMException(`The gallery's HUD canvas keeps no ${api}`, 'SecurityError')
}

export function isolateStorage(win: Window = window): void {
  const define = (target: object, name: string, descriptor: PropertyDescriptor) =>
    Object.defineProperty(target, name, { configurable: true, ...descriptor })
  define(win, 'localStorage', { value: new MemoryStorage() })
  define(win, 'sessionStorage', { value: new MemoryStorage() })
  define(win, 'indexedDB', {
    value: {
      open: refused('IndexedDB'),
      deleteDatabase: refused('IndexedDB'),
      databases: refused('IndexedDB')
    }
  })
  define(win, 'caches', {
    value: {
      open: refused('Cache Storage'),
      has: refused('Cache Storage'),
      keys: refused('Cache Storage')
    }
  })
  const cookies = new Map<string, string>()
  define(win.document, 'cookie', {
    get: () => [...cookies].map(([name, value]) => `${name}=${value}`).join('; '),
    set: (cookie: string) => {
      const pair = String(cookie).split(';', 1)[0]!
      const at = pair.indexOf('=')
      if (at > 0) cookies.set(pair.slice(0, at).trim(), pair.slice(at + 1).trim())
    }
  })
}

isolateStorage()
