#!/usr/bin/env node
// Actual source adapter/cache-key operations. The injected registry response
// normalizes known profile lookups; this is not a deployed registry capture.
const fs = require('node:fs')
const path = require('node:path')
const crypto = require('node:crypto')
const { load, logs, revision, upstream } = require('./social-service-voice-source-cases.cjs')
const pointer = '0xaabb111111111111111111111111111111111111'
const upper = '0xAABB111111111111111111111111111111111111'
const missing = '0xCaFe222222222222222222222222222222222222'
const document = { avatars: [{ userId: pointer, name: 'Case profile', hasClaimedName: true,
  avatar: { snapshots: { face256: 'https://images.example.org/case.png' } } }] }
async function main() {
  const cache = new Map(), fetches = [], steps = []
  const registry = await load('adapters/registry.ts').createRegistryComponent({ logs,
    config: { requireString: async () => 'https://registry.example.org' },
    redis: { get: async key => cache.get(key), mGet: async keys => keys.map(key => cache.get(key)), put: async (key, value) => cache.set(key, value) },
    fetcher: { fetch: async (_, options) => {
      const ids = JSON.parse(options.body).ids
      fetches.push(ids)
      return { ok: true, json: async () => [...new Set(ids.map(id => id.toLowerCase()))].filter(id => id === pointer).map(() => document) }
    } }
  })
  const operations = [ ['single', [upper]], ['single', [pointer]], ['batch', [upper, pointer, upper]],
    ['expire', [upper]], ['batch', [upper]], ['batch', [upper]], ['batch', [upper, pointer]], ['single', [missing]] ]
  for (const [operation, addresses] of operations) {
    let profiles, error
    if (operation === 'expire') cache.delete(`catalyst:minimal:profile:${addresses[0]}`)
    else try {
      const values = operation === 'single' ? [await registry.getProfile(addresses[0])] : await registry.getProfiles(addresses)
      profiles = values.map(load('logic/profiles.ts').getProfileInfo)
    } catch (failure) { error = failure.message }
    steps.push({ operation, addresses, profiles, error, fetches: [...fetches], cache_keys: [...cache.keys()].sort() })
  }
  const files = ['src/adapters/registry.ts', 'src/logic/profiles.ts', 'src/utils/address.ts']
  fs.writeFileSync(process.argv[3], JSON.stringify({ baseline: { repository: 'decentraland/social-service-ea', revision,
    sources: files.map(file => ({ path: file, sha256: crypto.createHash('sha256').update(fs.readFileSync(path.join(upstream, file))).digest('hex') })) },
    runtime: { node: process.version }, scope: 'Actual source registry with exact requested cache keys and case-sensitive Set deduplication. The profile-fetch dependency is injected with one normalized document per known pointer; no claim about deployed registry case handling or response ordering.', pointer, document, steps }, null, 2) + '\n')
  process.stdout.write(`Captured ${steps.length} mixed-case profile key steps\n`)
}
main().catch(error => { console.error(error); process.exitCode = 1 })
