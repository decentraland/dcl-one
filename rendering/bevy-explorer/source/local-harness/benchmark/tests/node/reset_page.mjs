// Runs the reset page's script, as the server serves it, against a stand-in browser and prints what
// it did as JSON: where it sent the browser on, and what it left in storage.
//
//   node reset_page.mjs <page.html> <origin> <token> <case json>
//
// <case json> is { search, cookieStore }: the page's query string, and whether the browser has the
// Cookie Store API.
import { readFileSync } from 'node:fs'
import vm from 'node:vm'

const [pageFile, origin, token, caseJson] = process.argv.slice(2)
const { search, cookieStore } = JSON.parse(caseJson)

const html = readFileSync(pageFile, 'utf8')
const script = html.match(/<script>([\s\S]*)<\/script>/)[1]

const seen = { replaced: null, posted: null, unregistered: 0, cookiesDeleted: [], errors: [] }

const stored = { 'dcl-local-guest-root-key': 'a key', other: 'x', 'a-build-cache': 'y' }
const localStorage = { ...stored }
for (const [name, value] of Object.entries({
  getItem: (key) => (key in localStorage ? localStorage[key] : null),
  removeItem: (key) => { delete localStorage[key] }
})) Object.defineProperty(localStorage, name, { value, enumerable: false })

let sessionCount = 3
const sandbox = {
  console: { error: (...args) => seen.errors.push(args.map(String).join(' ')) },
  URL,
  URLSearchParams,
  location: {
    search,
    origin,
    pathname: `/__perf/${token}/reset`,
    replace: (url) => { seen.replaced = url }
  },
  navigator: {
    serviceWorker: {
      getRegistrations: async () => [1, 2].map(() => ({ unregister: async () => { seen.unregistered++ } }))
    }
  },
  localStorage,
  sessionStorage: { get length() { return sessionCount }, clear: () => { sessionCount = 0 } },
  document: { cookie: 'one=1; two=2' },
  fetch: async (url, init) => { seen.posted = { url, method: init.method, body: init.body } }
}
if (cookieStore) {
  sandbox.cookieStore = {
    getAll: async () => [{ name: 'one', domain: null, path: '/' }, { name: 'two', domain: null, path: '/' }],
    delete: async ({ name }) => { seen.cookiesDeleted.push(name) }
  }
}

await vm.runInNewContext(script, sandbox)
console.log(JSON.stringify({ ...seen, localStorage: Object.keys(localStorage), sessionStorage: sessionCount }))
