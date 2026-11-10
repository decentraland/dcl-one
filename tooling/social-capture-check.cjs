#!/usr/bin/env node
'use strict'

// Structural checks only: never import captures, upstream source or its dependencies.
const assert = require('node:assert/strict')
const { spawnSync } = require('node:child_process')
const { createHash } = require('node:crypto')
const fs = require('node:fs')
const path = require('node:path')

const ROOT = path.resolve(__dirname, '..')
const SOCIAL = 'services/catalyst/tests/fixtures/conformance/social'
const CONTRACTS = `${SOCIAL}/source-contracts`
const ANALYTICS = 'library/dcl-http/tests/fixtures/social-analytics-source-cases.json'
const SOURCES = {
  social: { repository: 'decentraland/social-service-ea', revision: 'b6b2cefefdf727b2ed47ddb887e77d226a41eea6' },
  gatekeeper: { repository: 'decentraland/comms-gatekeeper', revision: 'be6a4b97a2189157884503485892ff46d0ac68da' }
}
// Helpers belong in the syntax check, but are not capture entry points.
const HELPERS = new Set(['social-capture-common.cjs', 'social-voice-fixtures.cjs', 'social-capture-check.cjs'])
const TOKENS = new Set(['social', 'gatekeeper', 'fixture', 'fixture-directory', 'postgres-url', 'redis-server?'])

function requireNode26(version = process.versions.node) {
  assert.match(version, /^26\./, `Social capture checks require Node 26; got ${version}`)
}

function containedFile(root, relative) {
  assert.equal(typeof relative, 'string', 'Artifact paths must be strings')
  assert.ok(relative.length > 0 && !relative.includes('\\') && !relative.includes('\0') &&
    !path.posix.isAbsolute(relative) && !/^[A-Za-z]:/.test(relative) &&
    !relative.split('/').some(part => part === '.' || part === '..' || part === ''),
  `Artifact path must be relative and contained: ${relative}`)
  const resolved = path.join(root, relative)
  assert.ok(fs.existsSync(resolved), `Missing artifact: ${relative}`)
  const fromRoot = path.relative(fs.realpathSync(root), fs.realpathSync(resolved))
  assert.ok(fromRoot && !fromRoot.startsWith(`..${path.sep}`) && fromRoot !== '..' && !path.isAbsolute(fromRoot),
    `Artifact resolves outside the repository: ${relative}`)
  assert.ok(fs.statSync(resolved).isFile(), `Artifact must be a file: ${relative}`)
  return resolved
}

function list(root, directory, pattern) {
  return fs.readdirSync(path.join(root, directory)).filter(name => pattern.test(name))
    .map(name => `${directory}/${name}`).sort()
}

function inventory(root) {
  const tooling = list(root, 'tooling', /^social-.*\.cjs$/)
  const contracts = list(root, CONTRACTS, /\.cjs$/)
  return {
    scripts: [...tooling.filter(file => !HELPERS.has(path.posix.basename(file))), ...contracts],
    fixtures: [...list(root, SOCIAL, /-cases\.json$/), ...list(root, CONTRACTS, /\.json$/), ANALYTICS],
    syntax: [...tooling, ...contracts, ...list(root, 'tooling/tests', /^social-.*\.test\.cjs$/)]
  }
}

function sameInventory(actual, expected, label) {
  const missing = expected.filter(item => !actual.includes(item))
  const unknown = actual.filter(item => !expected.includes(item))
  assert.ok(missing.length === 0 && unknown.length === 0,
    `${label} coverage mismatch; uncatalogued: ${missing.join(', ') || '(none)'}; undiscovered: ${unknown.join(', ') || '(none)'}`)
}

function unique(values, label) {
  assert.equal(new Set(values).size, values.length, `Duplicate capture ${label}`)
}

function repository(value) {
  return typeof value === 'string' ? value.replace(/^https:\/\/github\.com\//, '') : value
}

// Preserve the frozen formats: baseline, source, provenance, and the two root-level variants.
function fixtureSources(fixture) {
  const found = {}
  function record(repo, revision) {
    const kind = Object.keys(SOURCES).find(key => SOURCES[key].repository === repository(repo))
    assert.ok(kind, `Unknown fixture source repository: ${repo}`)
    assert.equal(revision, SOURCES[kind].revision, `Wrong fixture ${kind} revision`)
    if (found[kind]) assert.equal(found[kind], revision, `Conflicting fixture ${kind} revisions`)
    found[kind] = revision
  }
  function pair(value) {
    if (value && typeof value === 'object') record(value.repository || value.source, value.revision)
  }
  if (fixture.baseline) pair(fixture.baseline)
  if (fixture.provenance) pair(fixture.provenance)
  if (typeof fixture.source === 'string') record(fixture.source, fixture.revision)
  else if (fixture.source) pair(fixture.source)
  if (fixture.revision && !fixture.source) {
    const repos = new Set((fixture.sources || []).map(source => source.repository))
    assert.equal(repos.size, 1, 'Root revision must identify one fixture repository')
    record([...repos][0], fixture.revision)
  }
  if (fixture.social) {
    assert.ok(fixture.sources.some(source => repository(source.repository) === SOURCES.social.repository),
      'Root social pin must cite the social repository')
    record(SOURCES.social.repository, fixture.social)
  }
  if (typeof fixture.companion === 'string') {
    assert.ok(fixture.sources.some(source => repository(source.repository) === SOURCES.gatekeeper.repository),
      'Root companion pin must cite the gatekeeper repository')
    record(SOURCES.gatekeeper.repository, fixture.companion)
  } else if (fixture.companion) pair(fixture.companion)
  return Object.keys(found).sort()
}

function validateCatalog(root = ROOT, catalog = JSON.parse(fs.readFileSync(path.join(root, 'tooling/social-captures.json'), 'utf8'))) {
  requireNode26()
  assert.equal(catalog.version, 1, 'Unsupported social capture catalog version')
  assert.equal(catalog.nodeMajor, 26, 'The capture catalog must require Node 26')
  assert.deepEqual(catalog.sources, SOURCES, 'The capture catalog must use the frozen source repositories and revision pins')
  assert.ok(Array.isArray(catalog.captures), 'Catalog captures must be an array')
  for (const key of ['id', 'script', 'fixture']) unique(catalog.captures.map(capture => capture[key]), key)
  const discovered = inventory(root)
  sameInventory(catalog.captures.map(capture => capture.script), discovered.scripts, 'Capture script')
  sameInventory(catalog.captures.map(capture => capture.fixture), discovered.fixtures, 'Fixture')
  assert.equal(catalog.captures.length, 48, 'The frozen capture catalog must contain exactly 48 entries')
  for (const capture of catalog.captures) {
    try {
      assert.match(capture.id, /^[a-z0-9]+(?:-[a-z0-9]+)*$/, 'Invalid capture id')
      containedFile(root, capture.script)
      const file = containedFile(root, capture.fixture)
      assert.ok(Array.isArray(capture.args) && capture.args.length >= 2, 'Capture args must be an ordered token array')
      unique(capture.args, 'argument token')
      assert.ok(capture.args.every(token => TOKENS.has(token)), 'Unknown capture argument token')
      assert.equal(capture.args[0], 'social', 'The first capture argument must be social')
      assert.equal(capture.args.filter(token => token === 'fixture' || token === 'fixture-directory').length, 1,
        'Capture args must name exactly one fixture output')
      assert.ok(Array.isArray(capture.sources), 'Capture sources must be an array')
      unique(capture.sources, 'source')
      assert.deepEqual([...capture.sources].sort(), capture.args.filter(token => Object.hasOwn(SOURCES, token)).sort(),
        'Capture source requirements must match its argument tokens')
      assert.ok(Array.isArray(capture.environment) && capture.environment.every(name => name === 'CATALYST_TEST_DATABASE_URL'),
        'Unknown capture environment requirement')
      assert.ok(Array.isArray(capture.commands), 'Capture commands must be an array')
      assert.deepEqual(capture.commands, capture.args.includes('redis-server?') ? ['redis-server'] : [],
        'Capture command requirements must match its argument tokens')
      unique(capture.environment, 'environment requirement')
      assert.equal(typeof capture.notes, 'string', 'Capture notes must be a string')
      const bytes = fs.readFileSync(file)
      let fixture
      try { fixture = JSON.parse(bytes.toString('utf8')) } catch (error) {
        throw new Error(`Invalid fixture JSON: ${error.message}`)
      }
      assert.ok(fixture && typeof fixture === 'object' && !Array.isArray(fixture), 'Fixture must be a JSON object')
      assert.deepEqual(fixtureSources(fixture), [...capture.sources].sort(), 'Fixture provenance must cite every required source')
      const node = fixture.node || fixture.provenance?.runtime ||
        (typeof fixture.runtime === 'string' ? fixture.runtime : fixture.runtime?.node)
      if (node !== undefined) assert.match(node, /^v26\./, 'Fixture runtime must use Node 26')
      assert.match(capture.sha256, /^[a-f0-9]{64}$/, 'Fixture SHA-256 must be a full lowercase digest')
      assert.equal(createHash('sha256').update(bytes).digest('hex'), capture.sha256,
        'Frozen fixture SHA-256 changed; review any recapture explicitly')
    } catch (error) {
      throw new Error(`${capture.id} (${capture.fixture}): ${error.message}`, { cause: error })
    }
  }
  return { captures: catalog.captures.length, fixtures: discovered.fixtures.length }
}

function checkSyntax(root = ROOT) {
  requireNode26()
  const files = inventory(root).syntax
  for (const file of files) {
    const result = spawnSync(process.execPath, ['--check', containedFile(root, file)], { encoding: 'utf8' })
    assert.equal(result.status, 0, `Syntax check failed: ${file}\n${result.stderr || result.error || ''}`)
  }
  return files.length
}

if (require.main === module) {
  try {
    const result = validateCatalog()
    const syntax = checkSyntax()
    console.log(`Social capture catalog: ${result.captures} captures, ${result.fixtures} frozen JSON fixtures; syntax checked ${syntax} capture, helper and test files.`)
  } catch (error) {
    console.error(error.message)
    process.exitCode = 1
  }
}

module.exports = { validateCatalog, checkSyntax, containedFile, fixtureSources, requireNode26 }
