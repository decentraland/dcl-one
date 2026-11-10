'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { execFileSync, spawnSync } = require('node:child_process')
const { after, test } = require('node:test')
const { SOCIAL_REVISION, requireNode26 } = require('../social-capture-common.cjs')
requireNode26()

const tooling = path.resolve(__dirname, '..')
const scripts = fs.readdirSync(tooling).filter(name => /^social-.*-cases\.cjs$/.test(name) || name === 'social-service-handler-errors.cjs').sort()
const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'social-tooling-guards-'))
after(() => fs.rmSync(directory, { recursive: true, force: true }))
const git = (root, ...args) => execFileSync('git', ['-C', root, ...args], { stdio: 'pipe', encoding: 'utf8' }).trim()
function checkout(name) {
  const root = path.join(directory, name)
  fs.mkdirSync(root)
  git(root, 'init', '--quiet')
  fs.writeFileSync(path.join(root, '.gitignore'), 'node_modules/\n')
  fs.writeFileSync(path.join(root, 'package.json'), '{}\n')
  fs.writeFileSync(path.join(root, 'tracked.txt'), 'original\n')
  git(root, 'add', '.')
  git(root, '-c', 'user.name=Capture test', '-c', 'user.email=capture@example.invalid', 'commit', '--quiet', '-m', 'fixture')
  const dependency = path.join(root, 'node_modules/ts-node')
  fs.mkdirSync(dependency, { recursive: true })
  fs.writeFileSync(path.join(dependency, 'index.js'), "require('node:fs').writeFileSync(process.env.CAPTURE_LOAD_MARKER, 'loaded'); throw Error('dependency sentinel reached')\n")
  return fs.realpathSync(root)
}
const clean = checkout('clean')
const preload = path.join(directory, 'preload.cjs')
fs.writeFileSync(preload, `
if (process.env.CAPTURE_NODE_MAJOR) Object.defineProperty(process.versions, 'node', { value: process.env.CAPTURE_NODE_MAJOR + '.0.0' })
if (process.env.CAPTURE_PIN_ROOT) {
  const childProcess = require('node:child_process')
  const original = childProcess.execFileSync
  childProcess.execFileSync = function (file, args, ...rest) {
    if (file === 'git' && args[1] === process.env.CAPTURE_PIN_ROOT && args.includes('--verify')) return ${JSON.stringify(SOCIAL_REVISION)}
    return original.call(this, file, args, ...rest)
  }
}
`)
function run(script, root, options = {}) {
  const marker = path.join(directory, 'dependency-loaded')
  const output = path.join(directory, 'capture.json')
  fs.rmSync(marker, { force: true })
  fs.rmSync(output, { force: true })
  const result = spawnSync(process.execPath, ['--require', preload, path.join(tooling, script), root, output, options.companion || clean], {
    env: { ...process.env, CATALYST_TEST_DATABASE_URL: 'postgresql://capture.invalid/disposable', CAPTURE_PIN_ROOT: root, CAPTURE_LOAD_MARKER: marker, ...options.env },
    encoding: 'utf8', timeout: 5000
  })
  assert.ifError(result.error)
  assert.notEqual(result.status, 0, script)
  assert.equal(fs.existsSync(output), false, `${script} wrote a capture before preflight completed`)
  return { stderr: result.stderr, loaded: fs.existsSync(marker) }
}

test('all 29 tooling entrypoints are independent of capture entrypoint imports', () => {
  assert.equal(scripts.length, 29)
  for (const script of scripts) {
    const source = fs.readFileSync(path.join(tooling, script), 'utf8')
    assert.doesNotMatch(source, /require\(['"]\.\/social-.*(?:cases|errors)\.cjs['"]\)/, script)
    assert.match(source, /^\/\/ Output fixture: .+\.json$/m, script)
  }
})

test('every tooling capture rejects another Node major before loading upstream dependencies', () => {
  for (const script of scripts) {
    const result = run(script, clean, { env: { CAPTURE_NODE_MAJOR: '24' } })
    assert.match(result.stderr, /require Node 26/, script)
    assert.equal(result.loaded, false, script)
  }
})

test('every tooling capture rejects a wrong HEAD before loading upstream dependencies', () => {
  for (const script of scripts) {
    const result = run(script, clean, { env: { CAPTURE_PIN_ROOT: '' } })
    assert.match(result.stderr, /pinned revision/, script)
    assert.equal(result.loaded, false, script)
  }
})

for (const state of ['tracked', 'staged', 'untracked']) {
  test(`every tooling capture rejects ${state} changes before loading upstream dependencies`, () => {
    const root = checkout(state)
    fs.writeFileSync(path.join(root, state === 'untracked' ? 'extra.ts' : 'tracked.txt'), 'changed\n')
    if (state === 'staged') git(root, 'add', 'tracked.txt')
    for (const script of scripts) {
      const result = run(script, root)
      assert.match(result.stderr, /Expected clean .* including untracked files/, script)
      assert.equal(result.loaded, false, script)
    }
  })
}

test('the companion checkout is validated before the social TypeScript registrar loads', () => {
  const result = run('social-service-private-lifecycle-cases.cjs', clean, { companion: checkout('companion') })
  assert.match(result.stderr, /comms-gatekeeper at pinned revision/)
  assert.equal(result.loaded, false)
})

test('database captures reject missing, empty and whitespace-only explicit URLs before loading dependencies', () => {
  const databaseScripts = scripts.filter(script => fs.readFileSync(path.join(tooling, script), 'utf8').includes('requireDatabaseUrl('))
  assert.equal(databaseScripts.length, 8)
  for (const script of databaseScripts) for (const value of [undefined, '', ' \t ']) {
    const result = run(script, clean, { env: { CATALYST_TEST_DATABASE_URL: value } })
    assert.match(result.stderr, /explicit disposable PostgreSQL URL/, script)
    assert.equal(result.loaded, false, script)
  }
})

test('a clean pinned checkout reaches dependency registration after the guards', () => {
  const result = run('social-service-source-cases.cjs', clean)
  assert.match(result.stderr, /dependency sentinel reached/)
  assert.equal(result.loaded, true)
})

test('creating shared voice fixtures loads no source until a scenario runs', () => {
  const { createCommunityVoiceFixtures } = require('../social-voice-fixtures.cjs')
  let loads = 0
  const components = createCommunityVoiceFixtures({ load() { loads++; throw Error('unexpected source load') }, logs: {} })
  assert.equal(typeof components, 'function')
  assert.equal(loads, 0)
})
