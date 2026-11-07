#!/usr/bin/env node
// lfs-prefetch: downloads the Git LFS objects a key scan will read (key-guard.mjs --lfs-objects) from one fixed
// LFS endpoint into the repository's local object store, so the scan itself runs with no credentials and no
// network. It asks for exactly the listed objects through the LFS batch API: no ref, tree or path is
// resolved here, and nothing in the repository (.lfsconfig, .gitattributes, Git config) chooses the endpoint.
// Every object is checked against its oid and size before it lands in the store. Any object it cannot
// supply is a failure, never a skip: the scan would otherwise report it as an unmaterialized pointer anyway.
// The authorization value comes from LFS_AUTHORIZATION and is never printed. Dependency-free.
import { Buffer } from 'node:buffer'
import { createHash } from 'node:crypto'
import { mkdirSync, readFileSync, renameSync, statSync, unlinkSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { lfsStore } from './key-guard.mjs'

const BATCH_SIZE = 100
const CONCURRENCY = 4
const LFS_JSON = 'application/vnd.git-lfs+json'

/** An https URL, or plain http on the loopback interface only (the tests' fake server). */
export function allowedUrl(value) {
  let url
  try {
    url = new URL(value)
  } catch {
    return false
  }
  if (url.username || url.password) return false
  return url.protocol === 'https:' || (url.protocol === 'http:' && url.hostname === '127.0.0.1')
}

/** Parses the JSON lines key-guard --lfs-objects prints into distinct, well-formed { oid, size }. */
export function parseObjects(text) {
  const objects = new Map()
  for (const line of text.split('\n')) {
    if (!line.trim()) continue
    const value = JSON.parse(line)
    if (
      typeof value?.oid !== 'string' ||
      !/^[0-9a-f]{64}$/.test(value.oid) ||
      !Number.isSafeInteger(value.size) ||
      value.size < 0
    )
      throw new Error('malformed LFS object line')
    objects.set(value.oid, { oid: value.oid, size: value.size })
  }
  return [...objects.values()]
}

const objectPath = (store, oid) => join(store, oid.slice(0, 2), oid.slice(2, 4), oid)

function verified(file, { oid, size }) {
  try {
    if (statSync(file).size !== size) return false
    return createHash('sha256').update(readFileSync(file)).digest('hex') === oid
  } catch {
    return false
  }
}

async function batch(endpoint, authorization, objects, fetchFn) {
  const response = await fetchFn(`${endpoint}/objects/batch`, {
    method: 'POST',
    redirect: 'error',
    headers: { Accept: LFS_JSON, 'Content-Type': LFS_JSON, Authorization: authorization },
    body: JSON.stringify({ operation: 'download', transfers: ['basic'], objects })
  })
  if (!response.ok) throw new Error(`the LFS batch request answered ${response.status}`)
  const body = await response.json()
  return new Map((body.objects ?? []).map((item) => [item.oid, item]))
}

async function download(store, object, answer, fetchFn) {
  if (!answer) return 'not in the batch response'
  if (answer.error) return `the server refused it (${answer.error.code ?? 'error'})`
  const action = answer.actions?.download
  if (!action?.href || !allowedUrl(action.href)) return 'no usable download action'
  const response = await fetchFn(action.href, { headers: action.header ?? {} })
  if (!response.ok) return `the download answered ${response.status}`
  const content = Buffer.from(await response.arrayBuffer())
  if (content.length !== object.size) return 'the payload size does not match the pointer'
  if (createHash('sha256').update(content).digest('hex') !== object.oid)
    return 'the payload hash does not match the pointer'
  const file = objectPath(store, object.oid)
  mkdirSync(dirname(file), { recursive: true })
  const temporary = `${file}.${process.pid}.tmp`
  writeFileSync(temporary, content)
  try {
    renameSync(temporary, file)
  } catch (error) {
    unlinkSync(temporary)
    throw error
  }
  return null
}

/**
 * Ensures every object is in `store`, verified. Returns { local, downloaded, bytes, failures }, where each
 * failure is { oid, reason }. Objects already present and verified are not requested.
 */
export async function prefetch({
  store,
  endpoint,
  authorization,
  objects,
  fetchFn = globalThis.fetch
}) {
  if (!allowedUrl(endpoint))
    throw new Error('the LFS endpoint must be an https URL without credentials')
  const missing = objects.filter((object) => !verified(objectPath(store, object.oid), object))
  const failures = []
  let bytes = 0
  for (let start = 0; start < missing.length; start += BATCH_SIZE) {
    const chunk = missing.slice(start, start + BATCH_SIZE)
    const answers = await batch(endpoint, authorization, chunk, fetchFn)
    for (let next = 0; next < chunk.length; next += CONCURRENCY)
      await Promise.all(
        chunk.slice(next, next + CONCURRENCY).map(async (object) => {
          const reason = await download(store, object, answers.get(object.oid), fetchFn).catch(
            (error) => `the download failed (${error instanceof Error ? error.name : 'error'})`
          )
          if (reason) failures.push({ oid: object.oid, reason })
          else bytes += object.size
        })
      )
  }
  return {
    local: objects.length - missing.length,
    downloaded: missing.length - failures.length,
    bytes,
    failures
  }
}

/** Command-line entry point. Returns the exit code. */
export async function main(argv, env = process.env) {
  const options = { root: process.cwd(), endpoint: undefined, objects: undefined }
  for (let i = 0; i < argv.length; i += 1) {
    if (argv[i] === '--root') options.root = resolve(argv[++i])
    else if (argv[i] === '--endpoint') options.endpoint = argv[++i]
    else if (argv[i] === '--objects') options.objects = argv[++i]
    else {
      process.stderr.write('lfs-prefetch: unknown argument\n')
      return 2
    }
  }
  if (!options.endpoint || !options.objects || !env.LFS_AUTHORIZATION) {
    process.stderr.write('lfs-prefetch: --endpoint, --objects and LFS_AUTHORIZATION are required\n')
    return 2
  }
  try {
    const objects = parseObjects(readFileSync(options.objects, 'utf8'))
    const result = await prefetch({
      store: lfsStore(options.root),
      endpoint: options.endpoint.replace(/\/+$/, ''),
      authorization: env.LFS_AUTHORIZATION,
      objects
    })
    for (const { oid, reason } of result.failures)
      process.stdout.write(`MISSING ${oid}: ${reason}\n`)
    process.stdout.write(
      `lfs-prefetch: ${objects.length} objects, ${result.local} already local, ${result.downloaded} downloaded (${result.bytes} bytes), ${result.failures.length} missing\n`
    )
    return result.failures.length ? 1 : 0
  } catch (error) {
    // Only the message's first line: a response body never reaches the log.
    process.stderr.write(
      `lfs-prefetch: ${String(error instanceof Error ? error.message : error).split('\n')[0]}\n`
    )
    return 1
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = await main(process.argv.slice(2))
}
