'use strict'

const assert = require('node:assert/strict')
const { test } = require('node:test')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { execFileSync, spawnSync } = require('node:child_process')
const { SOCIAL_REVISION, GATEKEEPER_REVISION, sha256, provenance, requireDatabaseUrl } = require('../social-capture-common.cjs')

const repository = path.resolve(__dirname, '../..')
const helper = path.join(repository, 'tooling/social-capture-common.cjs')
const captures = path.join(repository, 'services/catalyst/tests/fixtures/conformance/social/source-contracts')
const catalog = JSON.parse(fs.readFileSync(path.join(repository, 'tooling/social-captures.json'), 'utf8'))

function fixture(t) {
  const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'social-capture-test-')))
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }))
  const source = path.join(directory, 'source')
  fs.mkdirSync(source)
  const git = (...args) => execFileSync('git', ['-C', source, ...args], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim()
  git('init', '--quiet')
  fs.writeFileSync(path.join(source, '.gitignore'), 'node_modules/\nignored\n')
  fs.writeFileSync(path.join(source, 'tracked'), 'clean\n')
  git('add', '.')
  git('-c', 'user.name=Capture Test', '-c', 'user.email=capture@example.invalid', '-c', 'commit.gpgsign=false', 'commit', '--quiet', '-m', 'capture test')
  const marker = path.join(directory, 'dependency-imports')
  const output = path.join(directory, 'output.json')
  const events = path.join(directory, 'events')
  const preload = path.join(directory, 'preload.cjs')
  // Only child tests substitute a temporary commit's expected identity. The actual
  // Git root, status, staged changes and untracked files still come from Git.
  fs.writeFileSync(preload, `
const fs = require('node:fs'), path = require('node:path'), cp = require('node:child_process'), Module = require('node:module');
const exec = cp.execFileSync, originalLoad = Module._load;
const pins = JSON.parse(process.env.CAPTURE_TEST_PINS || '{}');
cp.execFileSync = function(command, args, ...rest) {
  const result = exec.call(this, command, args, ...rest);
  if (command === 'git' && args.includes('--verify') && args.at(-1) === 'HEAD' && pins[args[1]]) return pins[args[1]] + '\\n';
  return result;
};
if (process.env.CAPTURE_TEST_NODE) Object.defineProperty(process.versions, 'node', { value: process.env.CAPTURE_TEST_NODE });
Module._load = function(name, parent, ...rest) {
  const imported = name === 'ts-node' || Object.keys(pins).some(root => name.startsWith(root + '/src/') || name.startsWith(root + '/node_modules/'));
  if (imported) {
    fs.appendFileSync(process.env.CAPTURE_TEST_MARKER, name + '\\n');
    if (!process.env.CAPTURE_TEST_SERVER) {
      if (name === 'ts-node' && process.env.CAPTURE_TEST_ACCEPT) return { register() {} };
      throw Error('Upstream dependency imported before rejection');
    }
    if (name === 'ts-node') return { register() {} };
    if (name.endsWith('/server.js')) return { async createServerComponent() {
      const server = require('node:http').createServer((_request, response) => response.end());
      return { use() {}, setContext() {},
        async start() { await new Promise(resolve => server.listen(0, '127.0.0.1', resolve)); fs.appendFileSync(process.env.CAPTURE_TEST_EVENTS, 'start\\n'); },
        async stop() { await new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve())); fs.appendFileSync(process.env.CAPTURE_TEST_EVENTS, 'stop\\n'); }
      };
    } };
    if (name.endsWith('/injectors.js')) return { getUnderlyingServer() { throw Error('injected failure after listener start'); } };
    if (name.endsWith('/http.routes.ts')) return { async setupHttpRoutes() { return { middleware() {}, allowedMethods() {} }; } };
    if (name.endsWith('/env-config-provider')) return { createRecordConfigComponent(values) { return { async getNumber(key) { return Number(values[key]); } }; } };
    if (name.endsWith('/requestBodyLimit.ts')) return { resolveMaxRequestBodyBytes(value) { return value; } };
    if (name.endsWith('/schema-validator-component')) return { createSchemaValidatorComponent() { return {}; } };
    if (name.endsWith('/pg')) return { Client: class { async connect() {} async end() {} async query() { throw Object.assign(Error('injected SQL failure'), { code: 'P0001' }); } } };
    return {};
  }
  return originalLoad.call(this, name, parent, ...rest);
};
`)
  function run(args, options = {}) {
    return spawnSync(process.execPath, args, {
      encoding: 'utf8', timeout: 15000,
      env: { ...process.env, NODE_OPTIONS: `--require=${JSON.stringify(preload)}`, CAPTURE_TEST_PINS: JSON.stringify({ [source]: SOCIAL_REVISION }), CAPTURE_TEST_MARKER: marker, CAPTURE_TEST_EVENTS: events, ...options }
    })
  }
  return { directory, source, marker, output, events, git, run }
}

function check(f, expression, options) {
  return f.run(['-e', `const common = require(${JSON.stringify(helper)}); ${expression}`, f.source], options)
}

function rejected(result, pattern) {
  assert.equal(result.error, undefined, result.error?.message)
  assert.equal(result.status, 1, result.stderr)
  assert.match(result.stderr, pattern)
}

test('requires Node 26 before imports', t => {
  const f = fixture(t)
  rejected(check(f, 'common.sourceCapture(process.argv[1])', { CAPTURE_TEST_NODE: '25.1.0' }), /require Node 26/)
  assert.equal(fs.existsSync(f.marker), false)
})

test('rejects missing checkout, non-Git root and wrong HEAD', t => {
  const f = fixture(t)
  rejected(check(f, 'common.assertPinnedSource()'), /checkout root is required/)
  rejected(check(f, 'common.assertPinnedSource(process.argv[1] + "/missing")'), /does not exist/)
  rejected(check(f, `common.assertPinnedSource(${JSON.stringify(f.directory)})`), /Git working tree/)
  rejected(check(f, 'common.sourceCapture(process.argv[1])', { CAPTURE_TEST_PINS: '{}' }), /pinned revision/)
  assert.equal(fs.existsSync(f.marker), false)
})

test('accepts clean Git roots and ignores Git-ignored files', t => {
  const f = fixture(t)
  fs.writeFileSync(path.join(f.source, 'ignored'), 'ignored\n')
  const result = check(f, 'const c=common.sourceCapture(process.argv[1]); console.log(c.upstream,c.revision,typeof c.req,typeof c.load)', { CAPTURE_TEST_ACCEPT: '1' })
  assert.equal(result.status, 0, result.stderr)
  assert.match(result.stdout, new RegExp(`${SOCIAL_REVISION} function function`))
  assert.equal(fs.readFileSync(f.marker, 'utf8'), 'ts-node\n')
})

for (const state of ['unstaged', 'staged', 'untracked']) {
  test(`rejects ${state} files before any upstream import`, t => {
    const f = fixture(t)
    fs.writeFileSync(path.join(f.source, state === 'untracked' ? 'new-file' : 'tracked'), 'dirty\n')
    if (state === 'staged') f.git('add', 'tracked')
    rejected(check(f, 'common.sourceCapture(process.argv[1])'), /Expected clean/)
    assert.equal(fs.existsSync(f.marker), false)
  })
}

test('keeps hash metadata shape and requires an explicit database URL', t => {
  const f = fixture(t)
  const hash = sha256(f.source, 'tracked')
  assert.match(hash, /^[a-f0-9]{64}$/)
  assert.deepEqual(provenance(f.source, ['tracked'], { runtime: 'fixture' }), { repository: 'decentraland/social-service-ea', revision: SOCIAL_REVISION, sha256: { tracked: hash }, runtime: 'fixture' })
  for (const value of [undefined, null, '', '  ']) assert.throws(() => requireDatabaseUrl(value), /explicit disposable PostgreSQL URL/)
  assert.equal(requireDatabaseUrl('postgresql://fixture.invalid/disposable'), 'postgresql://fixture.invalid/disposable')
})

test('all 19 fixture CLIs reject wrong source HEAD before imports or output writes', t => {
  const f = fixture(t)
  const scripts = fs.readdirSync(captures).filter(name => /^capture-.*\.cjs$/.test(name))
  assert.equal(scripts.length, 19)
  for (const script of scripts) {
    rejected(f.run([path.join(captures, script), f.source, f.output, 'postgresql://fixture.invalid/disposable'], { CAPTURE_TEST_PINS: '{}' }), /pinned revision/)
    assert.equal(fs.existsSync(f.output), false, script)
    assert.equal(fs.existsSync(f.marker), false, script)
  }
})

test('all 19 fixture CLIs reject another Node major before imports or output writes', t => {
  const f = fixture(t)
  for (const script of fs.readdirSync(captures).filter(name => /^capture-.*\.cjs$/.test(name))) {
    rejected(f.run([path.join(captures, script), f.source, f.output, 'postgresql://fixture.invalid/disposable'], { CAPTURE_TEST_NODE: '25.1.0' }), /require Node 26/)
    assert.equal(fs.existsSync(f.marker), false, script)
    assert.equal(fs.existsSync(f.output), false, script)
  }
})

test('both voice CLIs validate both source repositories before dependency imports or output writes', t => {
  const social = fixture(t), gatekeeper = fixture(t)
  for (const script of ['capture-community-voice.cjs', 'capture-community-voice-errors.cjs']) {
    for (const invalid of ['social', 'gatekeeper']) {
      const pins = invalid === 'social' ? { [gatekeeper.source]: GATEKEEPER_REVISION } : { [social.source]: SOCIAL_REVISION }
      rejected(social.run([path.join(captures, script), social.source, gatekeeper.source, social.output], { CAPTURE_TEST_PINS: JSON.stringify(pins) }), /pinned revision/)
      assert.equal(fs.existsSync(social.marker), false)
      assert.equal(fs.existsSync(social.output), false)
    }
    fs.writeFileSync(path.join(gatekeeper.source, 'new-file'), 'dirty\n')
    rejected(social.run([path.join(captures, script), social.source, gatekeeper.source, social.output], { CAPTURE_TEST_PINS: JSON.stringify({ [social.source]: SOCIAL_REVISION, [gatekeeper.source]: GATEKEEPER_REVISION }) }), /Expected clean/)
    fs.unlinkSync(path.join(gatekeeper.source, 'new-file'))
    assert.equal(fs.existsSync(social.marker), false)
    assert.equal(fs.existsSync(social.output), false)
  }
})

test('all catalogued database CLIs reject missing, empty and blank URLs despite ambient PostgreSQL configuration', t => {
  const f = fixture(t)
  const databaseCaptures = catalog.captures.filter(capture => capture.args.includes('postgres-url'))
  assert.equal(databaseCaptures.length, 5)
  for (const capture of databaseCaptures) for (const value of [undefined, '', ' \t ']) {
    assert.deepEqual(capture.args, ['social', 'fixture', 'postgres-url'], capture.id)
    const args = [path.join(repository, capture.script), f.source, f.output]
    if (value !== undefined) args.push(value)
    rejected(f.run(args, { PGHOST: 'fixture.invalid', PGDATABASE: 'ambient', CATALYST_TEST_DATABASE_URL: 'postgresql://fixture.invalid/ambient' }), /explicit disposable PostgreSQL URL/)
    assert.equal(fs.existsSync(f.marker), false, capture.id)
    assert.equal(fs.existsSync(f.output), false, capture.id)
  }
})

for (const name of ['body-limit', 'federation-boundary', 'routing']) {
  test(`${name} failure closes a real loopback listener and exits without forced termination`, t => {
    const f = fixture(t)
    rejected(f.run([path.join(captures, `capture-community-${name}.cjs`), f.source, f.output, 'postgresql://fixture.invalid/disposable'], { CAPTURE_TEST_SERVER: '1' }), /injected failure after listener start/)
    assert.equal(fs.readFileSync(f.events, 'utf8'), 'start\nstop\n')
    assert.equal(fs.existsSync(f.output), false)
  })
}
