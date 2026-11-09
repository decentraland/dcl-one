'use strict'

// Capture tooling only: importing this module never loads an upstream dependency.
const fs = require('node:fs')
const path = require('node:path')
const crypto = require('node:crypto')
const { execFileSync } = require('node:child_process')
const { createRequire } = require('node:module')

const SOCIAL_REVISION = 'b6b2cefefdf727b2ed47ddb887e77d226a41eea6'
const GATEKEEPER_REVISION = 'be6a4b97a2189157884503485892ff46d0ac68da'
const SOURCES = Object.freeze({
  social: Object.freeze({ repository: 'decentraland/social-service-ea', revision: SOCIAL_REVISION }),
  gatekeeper: Object.freeze({ repository: 'decentraland/comms-gatekeeper', revision: GATEKEEPER_REVISION })
})

function requireNode26() {
  if (process.versions.node.split('.')[0] !== '26') throw Error('Social captures require Node 26')
}

function assertPinnedSource(root, kind = 'social') {
  requireNode26()
  const source = SOURCES[kind]
  if (!source) throw Error('Unknown capture source kind')
  if (typeof root !== 'string' || !root.trim()) throw Error('A source checkout root is required')
  let upstream
  try { upstream = fs.realpathSync(root) } catch { throw Error('Source checkout root does not exist') }
  const git = (...args) => execFileSync('git', ['-C', upstream, ...args], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim()
  let top, revision, status
  try {
    top = fs.realpathSync(git('rev-parse', '--show-toplevel'))
    revision = git('rev-parse', '--verify', 'HEAD')
    status = git('status', '--porcelain=v1', '--untracked-files=all')
  } catch { throw Error('Source checkout must be a Git working tree') }
  if (top !== upstream) throw Error('Source checkout root must be the Git working tree root')
  if (revision !== source.revision) throw Error(`Expected ${source.repository} at pinned revision ${source.revision}`)
  if (status) throw Error(`Expected clean ${source.repository} source, including untracked files`)
  return { upstream, ...source }
}

function sourceCapture(root, { companions = [] } = {}) {
  const source = assertPinnedSource(root)
  // Validate every companion before even loading the TypeScript registrar.
  for (const companion of companions) assertPinnedSource(companion.root, companion.kind)
  const req = createRequire(path.join(source.upstream, 'package.json'))
  req('ts-node').register({
    transpileOnly: true,
    project: path.join(source.upstream, 'tsconfig.json'),
    compilerOptions: { module: 'CommonJS', moduleResolution: 'node' }
  })
  return { ...source, req, load: file => req(path.join(source.upstream, 'src', file)) }
}

function sha256(root, file) {
  return crypto.createHash('sha256').update(fs.readFileSync(path.join(root, file))).digest('hex')
}

// Callers retain their existing metadata layout; use sha256 directly for variants.
function provenance(root, files, extra = {}) {
  return { ...SOURCES.social, sha256: Object.fromEntries(files.map(file => [file, sha256(root, file)])), ...extra }
}

function requireDatabaseUrl(value) {
  if (typeof value !== 'string' || !value.trim()) throw Error('An explicit disposable PostgreSQL URL is required')
  return value
}

module.exports = { SOCIAL_REVISION, GATEKEEPER_REVISION, requireNode26, assertPinnedSource, sourceCapture, sha256, provenance, requireDatabaseUrl }
