'use strict'

const assert = require('node:assert/strict')
const { test } = require('node:test')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { execFileSync, spawn, spawnSync } = require('node:child_process')

const script = path.resolve(__dirname, '../builder-server-capture.cjs')
const guard = path.resolve(__dirname, '../builder-server-offline.cjs')

function scratch(t) {
  const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'builder-capture-test-')))
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }))
  return directory
}

function checkout(t) {
  const directory = scratch(t)
  const git = (...args) => execFileSync('git', ['-C', directory, ...args], { stdio: 'ignore' })
  git('init', '--quiet')
  fs.mkdirSync(path.join(directory, 'src'))
  fs.writeFileSync(path.join(directory, 'src/server.ts'), 'throw Error("upstream code ran")\n')
  git('add', '.')
  git('-c', 'user.name=Capture Test', '-c', 'user.email=capture@example.invalid', '-c', 'commit.gpgsign=false', 'commit', '--quiet', '-m', 'not the pin')
  const preload = path.join(scratch(t), 'node-24.cjs')
  fs.writeFileSync(preload, "Object.defineProperty(process.versions, 'node', { value: '24.0.0' })\n")
  return { root: directory, preload }
}

function capture({ root, preload }, output, databaseUrl = 'postgres://capture@127.0.0.1:1/postgres') {
  const env = { ...process.env, NODE_OPTIONS: `--require=${JSON.stringify(preload)}`, CATALYST_TEST_DATABASE_URL: databaseUrl }
  return spawnSync(process.execPath, [script, root, output], { encoding: 'utf8', env })
}

function pinnedCheckout(t) {
  const source = checkout(t)
  const preload = path.join(scratch(t), 'pinned.cjs')
  fs.writeFileSync(preload, `
const cp = require('node:child_process')
const exec = cp.execFileSync
cp.execFileSync = function (command, args, ...rest) {
  if (command === 'git' && args.includes('--verify') && args.at(-1) === 'HEAD') return '98897af692f26063a088aebb2ea5bb3f683168a2\\n'
  return exec.call(this, command, args, ...rest)
}
require(${JSON.stringify(source.preload)})
`)
  return { ...source, preload }
}

function serialAdmin(log, { createDelay = 0, dropDelay = 0 } = {}) {
  let queue = Promise.resolve()
  const wait = ms => new Promise(resolve => setTimeout(resolve, ms))
  return {
    query(sql) {
      queue = queue.then(async () => {
        if (sql.startsWith('CREATE')) await wait(createDelay)
        if (sql.startsWith('DROP')) await wait(dropDelay)
        log(sql)
      })
      return queue
    },
    async end() {}
  }
}

function fakeAdmin(log) {
  return {
    async query(sql) { log.push(sql) },
    async end() { log.push('end') }
  }
}

test('a checkout at another revision is refused before anything runs', t => {
  const source = checkout(t)
  const output = path.join(source.root, 'out')
  const result = capture(source, output)
  assert.equal(result.status, 1, result.stderr)
  assert.match(result.stderr, /decentraland\/builder-server at pinned revision 98897af692f26063a088aebb2ea5bb3f683168a2/)
  assert.equal(fs.existsSync(output), false)
})

test('a directory inside a checkout is refused', t => {
  const source = checkout(t)
  const result = capture({ ...source, root: path.join(source.root, 'src') }, path.join(source.root, 'out'))
  assert.equal(result.status, 1, result.stderr)
  assert.match(result.stderr, /must be the Git working tree root/)
})

test('a run that fails a check leaves the published fixtures untouched, and a run that passes replaces them', t => {
  const { finish } = require(script)
  const output = path.join(scratch(t), 'source')
  fs.mkdirSync(output)
  fs.writeFileSync(path.join(output, '01-case.json'), 'committed\n')
  fs.writeFileSync(path.join(output, '03-removed-case.json'), 'committed\n')
  const fixtures = [['01-case.json', 'recaptured\n'], ['02-case.json', 'new\n']]
  assert.throws(() => finish(output, fixtures, ['example.org']), /does not fake: example.org/)
  assert.deepEqual(fs.readdirSync(output), ['01-case.json', '03-removed-case.json'])
  assert.equal(fs.readFileSync(path.join(output, '01-case.json'), 'utf8'), 'committed\n')
  finish(output, fixtures, [])
  assert.deepEqual(fs.readdirSync(output), ['01-case.json', '02-case.json'])
  assert.equal(fs.readFileSync(path.join(output, '01-case.json'), 'utf8'), 'recaptured\n')
  assert.equal(fs.readFileSync(path.join(output, '02-case.json'), 'utf8'), 'new\n')
  assert.deepEqual(fs.readdirSync(path.dirname(output)), ['source'])
})

test('the scratch database is dropped when closing upstream fails', async () => {
  const { withScratchDatabase } = require(script)
  const log = []
  await assert.rejects(
    withScratchDatabase(fakeAdmin(log), 'postgres://capture@127.0.0.1:1/postgres', async url => {
      log.push(new URL(url).pathname)
      throw Error('db.close failed')
    }),
    /db.close failed/
  )
  const [create, used, drop, end] = log
  const name = create.match(/^CREATE DATABASE "(builder_capture_\d+_[0-9a-f]{12})"$/)[1]
  assert.equal(used, `/${name}`)
  assert.equal(drop, `DROP DATABASE IF EXISTS "${name}" WITH (FORCE)`)
  assert.equal(end, 'end')
})

async function interrupt(t, { createDelay = 0, dropDelay = 0, signals = 1, readyAfterCreate = true }) {
  const queries = path.join(scratch(t), 'queries')
  const program = `
    const fs = require('node:fs')
    const { withScratchDatabase } = require(${JSON.stringify(script)})
    let queue = Promise.resolve()
    const wait = ms => new Promise(resolve => setTimeout(resolve, ms))
    const admin = {
      query(sql) {
        if (sql.startsWith('CREATE') && !${readyAfterCreate}) process.stdout.write('ready\\n')
        queue = queue.then(async () => {
          await wait(sql.startsWith('CREATE') ? ${createDelay} : ${dropDelay})
          fs.appendFileSync(${JSON.stringify(queries)}, sql + '\\n')
        })
        return queue
      },
      async end() {}
    }
    withScratchDatabase(admin, 'postgres://capture@127.0.0.1:1/postgres', () => {
      if (${readyAfterCreate}) process.stdout.write('ready\\n')
      return new Promise(() => setInterval(() => {}, 1000))
    })
  `
  const child = spawn(process.execPath, ['-e', program], { stdio: ['ignore', 'pipe', 'inherit'] })
  await new Promise(resolve => child.stdout.once('data', resolve))
  for (let i = 0; i < signals; i++) {
    if (i) await new Promise(resolve => setTimeout(resolve, 100))
    child.kill('SIGINT')
  }
  const exit = await new Promise(resolve => child.on('exit', (...args) => resolve(args)))
  return { exit, queries: fs.existsSync(queries) ? fs.readFileSync(queries, 'utf8') : '' }
}

const createThenDrop = /^CREATE DATABASE "(builder_capture_\S+)"\nDROP DATABASE IF EXISTS "\1" WITH \(FORCE\)\n$/

test('a second interrupt waits for the drop', async t => {
  const { exit, queries } = await interrupt(t, { dropDelay: 300, signals: 2 })
  assert.deepEqual(exit, [130, null])
  assert.match(queries, createThenDrop)
})

test('an interrupt while the database is being created drops it', async t => {
  const { exit, queries } = await interrupt(t, { createDelay: 300, readyAfterCreate: false })
  assert.deepEqual(exit, [130, null])
  assert.match(queries, createThenDrop)
})

test('a case may mask only the allowed fields, and only where they are recorded', () => {
  const { scrubVolatile } = require(script)
  const allowed = ['created_at', 'updated_at']
  const body = () => ({ data: { id: 'c1', created_at: '2026-11-18T00:00:00.000Z' } })
  assert.deepEqual(scrubVolatile({ name: 'a', volatile: ['data.created_at'] }, body(), allowed), { data: { id: 'c1', created_at: '<<VOLATILE>>' } })
  assert.throws(() => scrubVolatile({ name: 'a', volatile: ['data'] }, body(), allowed), /a: volatile path data is not one of the volatile_fields/)
  assert.throws(() => scrubVolatile({ name: 'a', volatile: ['data.updated_at'] }, body(), allowed), /a: volatile path data.updated_at matches nothing/)
})

test('the migration process loads the guard from a path with a space', t => {
  const tooling = path.join(scratch(t), 'tool dir', 'tooling')
  fs.mkdirSync(tooling, { recursive: true })
  for (const file of ['builder-server-capture.cjs', 'builder-server-offline.cjs', 'social-capture-common.cjs']) {
    fs.copyFileSync(path.resolve(__dirname, '..', file), path.join(tooling, file))
  }
  const upstream = path.join(scratch(t), 'upstream dir')
  fs.mkdirSync(path.join(upstream, 'migrations'), { recursive: true })
  fs.writeFileSync(path.join(upstream, 'migrations/node-pg-migrate'), `
setTimeout(() => process.exit(4), 2000).unref()
require('node:net').connect({ host: '192.0.2.10', port: 80 }).on('error', error => {
  if (error.message !== 'capture is offline: 192.0.2.10:80') process.exit(3)
  process.exitCode = 0
})
`)
  const { migrate } = require(path.join(tooling, 'builder-server-capture.cjs'))
  assert.throws(() => migrate(upstream, {}), error => /offline capture refused: 192\.0\.2\.10:80/.test(String(error.stderr)))
})

test('upstream runs with only the capture environment: no .env and no unlisted variable of the shell reach it', t => {
  const { useUpstreamEnvironment } = require(script)
  const cwd = scratch(t)
  fs.writeFileSync(path.join(cwd, '.env'), 'CORS_ORIGIN=https://local.example.org\nLOCAL_ONLY=1\n')
  const saved = { ...process.env }
  const savedCwd = process.cwd()
  t.after(() => {
    process.chdir(savedCwd)
    for (const name of Object.keys(process.env)) delete process.env[name]
    Object.assign(process.env, saved)
  })
  process.chdir(cwd)
  process.env.UNLISTED_CAPTURE_PROBE = 'shell'
  process.env.CORS_METHOD = 'DELETE'
  const upstreamEnv = {
    loaded: false,
    load({ path: file = './.env', values = {} } = {}) {
      if (Object.keys(values).length === 0 && fs.existsSync(file)) {
        values = Object.fromEntries(fs.readFileSync(file, 'utf8').trim().split('\n').map(line => line.split('=')))
      }
      Object.assign(process.env, values)
      this.loaded = true
    },
    get(name) {
      if (!this.loaded) this.load()
      return process.env[name]
    }
  }
  useUpstreamEnvironment(name => (name === 'decentraland-commons' ? { env: upstreamEnv } : assert.fail(name)), { CORS_ORIGIN: 'https://capture.example.org' })
  assert.equal(upstreamEnv.get('CORS_ORIGIN'), 'https://capture.example.org')
  assert.equal(upstreamEnv.get('LOCAL_ONLY'), undefined)
  assert.equal(process.env.UNLISTED_CAPTURE_PROBE, undefined)
  assert.equal(process.env.CORS_METHOD, undefined)
  assert.equal(process.env.PATH, saved.PATH)
})

test('the migration process gets only the capture environment and runs where no .env is', t => {
  const { migrate } = require(script)
  const upstream = scratch(t)
  const probe = path.join(scratch(t), 'probe.json')
  fs.mkdirSync(path.join(upstream, 'migrations'))
  fs.writeFileSync(path.join(upstream, '.env'), 'CONNECTION_STRING=postgres://local\n')
  fs.writeFileSync(path.join(upstream, 'migrations/node-pg-migrate'), `
require('node:fs').writeFileSync(process.env.PROBE, JSON.stringify({ names: Object.keys(process.env).sort(), dotEnv: require('node:fs').existsSync('.env'), cwd: process.cwd() }))
`)
  const saved = process.env.UNLISTED_CAPTURE_PROBE
  process.env.UNLISTED_CAPTURE_PROBE = 'shell'
  t.after(() => (saved === undefined ? delete process.env.UNLISTED_CAPTURE_PROBE : (process.env.UNLISTED_CAPTURE_PROBE = saved)))
  migrate(upstream, { PROBE: probe, CONNECTION_STRING: 'postgres://capture' })
  const seen = JSON.parse(fs.readFileSync(probe, 'utf8'))
  assert.equal(seen.dotEnv, false)
  assert.notEqual(seen.cwd, fs.realpathSync(upstream))
  assert.equal(fs.existsSync(seen.cwd), false)
  const allowed = new Set(['PROBE', 'CONNECTION_STRING', 'PATH', 'HOME', 'TMPDIR', 'TZ', '__CF_USER_TEXT_ENCODING'])
  assert.deepEqual(seen.names.filter(name => !allowed.has(name)), [])
})

test('an interrupted capture drops its database', async t => {
  const queries = path.join(scratch(t), 'queries')
  const program = `
    const fs = require('node:fs')
    const { withScratchDatabase } = require(${JSON.stringify(script)})
    const admin = { async query(sql) { fs.appendFileSync(${JSON.stringify(queries)}, sql + '\\n') }, async end() {} }
    withScratchDatabase(admin, 'postgres://capture@127.0.0.1:1/postgres', () => {
      process.stdout.write('ready\\n')
      return new Promise(() => setInterval(() => {}, 1000))
    })
  `
  const child = spawn(process.execPath, ['-e', program], { stdio: ['ignore', 'pipe', 'inherit'] })
  await new Promise(resolve => child.stdout.once('data', resolve))
  child.kill('SIGINT')
  const [code, signal] = await new Promise(resolve => child.on('exit', (...args) => resolve(args)))
  assert.deepEqual([code, signal], [130, null])
  assert.match(fs.readFileSync(queries, 'utf8'), /^CREATE DATABASE "(builder_capture_\S+)"\nDROP DATABASE IF EXISTS "\1" WITH \(FORCE\)\n$/)
})

test('the offline guard refuses addresses and names outside loopback', () => {
  const program = `
    const net = require('node:net'), dns = require('node:dns'), dgram = require('node:dgram')
    const report = {}
    const server = net.createServer(socket => socket.end()).listen(0, '127.0.0.1', () => {
      const local = net.connect(server.address().port, '127.0.0.1', () => {
        report.loopback = 'connected'
        local.destroy()
        net.connect({ host: '192.0.2.10', port: 80 }).on('error', async error => {
          report.address = error.message
          report.resolve = await new Promise(resolve => dns.resolve4('resolve.example.org', error => resolve(error.message)))
          report.resolver = await new Promise(resolve => new dns.Resolver().resolveTxt('resolver.example.org', error => resolve(error.message)))
          report.promises = await dns.promises.resolve('promises.example.org').catch(error => error.message)
          report.promisedLookup = await dns.promises.lookup('lookup.example.org').catch(error => error.message)
          try { dgram.createSocket('udp4').unref().send('x', 53, '192.0.2.53') } catch (error) { report.udp = error.message }
          dns.lookup('builder-api.example.org', error => {
            report.name = error.message
            server.close()
            console.log(JSON.stringify(report))
          })
        })
      })
    })
  `
  const env = { ...process.env, NODE_OPTIONS: `--require=${guard}` }
  for (const name of ['HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY', 'NODE_USE_ENV_PROXY']) {
    delete env[name]
    delete env[name.toLowerCase()]
  }
  const result = spawnSync(process.execPath, ['-e', program], { encoding: 'utf8', env })
  assert.equal(result.status, 1, result.stderr)
  assert.deepEqual(JSON.parse(result.stdout), {
    loopback: 'connected',
    address: 'capture is offline: 192.0.2.10:80',
    resolve: 'capture is offline: resolve.example.org',
    resolver: 'capture is offline: resolver.example.org',
    promises: 'capture is offline: promises.example.org',
    promisedLookup: 'capture is offline: lookup.example.org',
    udp: 'capture is offline: udp 192.0.2.53',
    name: 'capture is offline: builder-api.example.org'
  })
  assert.match(result.stderr, /offline capture refused: 192\.0\.2\.10:80, resolve\.example\.org, resolver\.example\.org/)
})

test('the offline guard reads a string first argument as a port wherever Node does', () => {
  const program = `
    const net = require('node:net')
    const { refused } = require(${JSON.stringify(guard)})
    for (const port of [' 443', '0x50', '8e1']) new net.Socket().on('error', () => {}).connect(port, '192.0.2.10').destroy()
    console.log(JSON.stringify(refused))
  `
  const result = spawnSync(process.execPath, ['-e', program], { encoding: 'utf8' })
  assert.deepEqual(JSON.parse(result.stdout), ['192.0.2.10: 443', '192.0.2.10:0x50', '192.0.2.10:8e1'])
})

test('the offline guard refuses an http request to an address outside loopback', () => {
  const program = `
    const http = require('node:http')
    const { refused } = require(${JSON.stringify(guard)})
    http.get('http://192.0.2.10:8080/x', { signal: AbortSignal.timeout(2000) })
      .on('error', error => console.log(JSON.stringify({ error: error.message, refused })))
  `
  const result = spawnSync(process.execPath, ['-e', program], { encoding: 'utf8' })
  assert.deepEqual(JSON.parse(result.stdout), { error: 'capture is offline: 192.0.2.10:8080', refused: ['192.0.2.10:8080'] })
})

test('the offline guard lets a socket connect to a loopback port with only a callback', () => {
  const program = `
    const net = require('node:net')
    const { refused } = require(${JSON.stringify(guard)})
    const server = net.createServer(socket => socket.end()).listen(0, () => {
      const socket = new net.Socket().on('error', error => {
        server.close()
        console.log(JSON.stringify({ error: error.message, refused }))
      })
      socket.connect(server.address().port, () => {
        socket.destroy()
        server.close()
        console.log(JSON.stringify({ connected: true, refused }))
      })
    })
  `
  const result = spawnSync(process.execPath, ['-e', program], { encoding: 'utf8' })
  assert.deepEqual(JSON.parse(result.stdout), { connected: true, refused: [] })
})

test('the offline guard refuses to start under a proxy, in any spelling', () => {
  for (const [name, value] of [['HTTPS_PROXY', 'http://127.0.0.1:9'], ['http_proxy', 'http://127.0.0.1:9'], ['NODE_USE_ENV_PROXY', '1']]) {
    const env = { ...process.env, [name]: value, NODE_OPTIONS: `--require=${guard}` }
    const result = spawnSync(process.execPath, ['-e', 'console.log("ran")'], { encoding: 'utf8', env })
    assert.notEqual(result.status, 0, name)
    assert.equal(result.stdout, '', name)
    assert.match(result.stderr, new RegExp(`unset .*${name}`), name)
  }
})

test('an error report never carries the database password', t => {
  const { redact } = require(script)
  const url = 'postgres://capture:s3cr%2Fet@127.0.0.1:5432/postgres'
  assert.equal(redact('failed for s3cr/et and s3cr%2Fet', url), 'failed for <redacted> and <redacted>')
  const malformed = 'postgres://postgres:example-secret-123@localhost:notaport/postgres'
  assert.equal(redact(`cannot reach ${malformed}`, malformed), 'cannot reach <redacted>')
  assert.equal(redact('plain', 'postgres://capture:%E0%A4%A@127.0.0.1/postgres'), 'plain')
  const result = capture(pinnedCheckout(t), path.join(scratch(t), 'out'), malformed)
  assert.equal(result.status, 1, result.stderr)
  assert.match(result.stderr, /CATALYST_TEST_DATABASE_URL is not a valid URL/)
  assert.doesNotMatch(result.stdout + result.stderr, /example-secret-123/)
})

test('a database outside loopback and Unix sockets is refused before anything runs', t => {
  const source = pinnedCheckout(t)
  for (const url of ['postgres://capture@postgres:5432/postgres', 'postgres://capture@localhost/postgres?host=db.example', 'postgres://capture@192.0.2.10/postgres']) {
    const result = capture(source, path.join(scratch(t), 'out'), url)
    assert.equal(result.status, 1, result.stderr)
    assert.match(result.stderr, /CATALYST_TEST_DATABASE_URL must name a loopback host or a Unix socket/, url)
  }
  for (const url of ['postgres://capture@%2Ftmp/postgres', 'postgres:///postgres?host=/tmp', 'postgres://capture@[::1]:5432/postgres']) {
    const result = capture(source, path.join(scratch(t), 'out'), url)
    assert.equal(result.status, 1, result.stderr)
    assert.match(result.stderr, /npm ci failed/, url)
  }
})

test('a recorded marker is refused before anything is published', () => {
  const { refuseMarkers } = require(script)
  refuseMarkers('a', { request: { headers: {} }, response: { body_json: { ok: true } } })
  assert.throws(() => refuseMarkers('a', { response: { body_json: { data: '<<VOLATILE>>' } } }), /a: upstream recorded <<VOLATILE>> at response.body_json.data/)
  assert.throws(() => refuseMarkers('a', { request: { headers: { authorization: 'x <<REDACTED:jwt>>' } } }), /a: upstream recorded <<REDACTED at request.headers.authorization/)
})

test('timestamps that were equal before masking are recorded as equal', () => {
  const { volatileEqualities } = require(script)
  const at = '2026-11-18T00:00:00.000Z'
  assert.deepEqual(volatileEqualities({ volatile: ['data.created_at', 'data.updated_at'] }, { data: { created_at: at, updated_at: at } }), [['data.created_at', 'data.updated_at']])
  assert.deepEqual(volatileEqualities({ volatile: ['data.created_at', 'data.updated_at'] }, { data: { created_at: at, updated_at: '2026-11-18T00:00:01.000Z' } }), [])
  assert.deepEqual(volatileEqualities({ volatile: ['data.created_at'] }, { data: { created_at: at } }), [])
})

test('an uncaught error drops the scratch database and reports redacted', async t => {
  const queries = path.join(scratch(t), 'queries')
  const program = `
    const fs = require('node:fs')
    const { withScratchDatabase, installFailureHandlers } = require(${JSON.stringify(script)})
    installFailureHandlers()
    const admin = { async query(sql) { fs.appendFileSync(${JSON.stringify(queries)}, sql + '\\n') }, async end() {} }
    withScratchDatabase(admin, 'postgres://capture@127.0.0.1:1/postgres', () => {
      setTimeout(() => { throw Error('boom for example-secret-123') }, 10)
      return new Promise(() => setInterval(() => {}, 1000))
    })
  `
  const env = { ...process.env, CATALYST_TEST_DATABASE_URL: 'postgres://capture:example-secret-123@127.0.0.1:1/postgres' }
  const result = spawnSync(process.execPath, ['-e', program], { encoding: 'utf8', env, timeout: 10000 })
  assert.equal(result.status, 1, result.stderr)
  assert.match(result.stderr, /builder capture failed: boom for <redacted>/)
  assert.doesNotMatch(result.stdout + result.stderr, /example-secret-123/)
  assert.match(fs.readFileSync(queries, 'utf8'), /^CREATE DATABASE "(builder_capture_\S+)"\nDROP DATABASE IF EXISTS "\1" WITH \(FORCE\)\n$/)
})

test('a case cannot record a signature defect the replay would drop', () => {
  const { recordedSign } = require(script)
  assert.deepEqual(recordedSign({ name: 'a', sign: { path: '/v1/other' } }), { path: '/v1/other', metadata: '{}' })
  assert.deepEqual(recordedSign({ name: 'a', sign: { untimestamped: true, withHeaders: true } }), { untimestamped: true, withHeaders: true })
  assert.equal(recordedSign({ name: 'a' }), undefined)
  for (const [sign, problem] of [
    [{}, 'names no defect'],
    [{ offset: 0 }, 'offset 0 is no defect'],
    [{ withHeaders: true, path: '/v1/other' }, 'withHeaders needs untimestamped'],
    [{ untimestamped: true, path: '/v1/other' }, 'untimestamped takes no path, offset, metadata or sentMetadata'],
    [{ untimestamped: true, offset: 1 }, 'untimestamped takes no path, offset, metadata or sentMetadata'],
    [{ untimestamped: true, sentMetadata: '{}' }, 'untimestamped takes no path, offset, metadata or sentMetadata']
  ]) {
    assert.throws(() => recordedSign({ name: 'a', sign }), new RegExp(`a: sign ${problem}`), JSON.stringify(sign))
  }
  assert.throws(() => recordedSign({ name: 'a', metadata: '{"x":1}', sign: { untimestamped: true } }), /a: sign untimestamped takes no path, offset, metadata or sentMetadata/)
})

test('the fake subgraph answers the rows its where argument selects and reports a filter it cannot apply', () => {
  const { selected } = require(script)
  const rows = [
    { id: '0x5eed000000000000000000000000000000000006', creator: '0xc1', managers: [], minters: ['0xm1'] },
    { id: '0x5eed000000000000000000000000000000000007', creator: '0xc2', managers: ['0xa1'], minters: [] }
  ]
  const variable = name => ({ kind: 'Variable', name: { value: name } })
  const field = fields => ({
    name: { value: 'collections' },
    arguments: [{ name: { value: 'where' }, value: { kind: 'ObjectValue', fields: Object.entries(fields).map(([key, value]) => ({ name: { value: key }, value })) } }]
  })
  const unfaked = []
  const ids = found => found.map(row => row.id.slice(-1))
  assert.deepEqual(ids(selected(field({ id: variable('id') }), rows, { id: '0x5eed000000000000000000000000000000000007' }, unfaked)), ['7'])
  assert.deepEqual(ids(selected(field({ id_in: { kind: 'ListValue', values: [variable('a')] } }), rows, { a: rows[0].id }, unfaked)), ['6'])
  assert.deepEqual(ids(selected(field({ managers_contains: variable('users') }), rows, { users: ['0xa1'] }, unfaked)), ['7'])
  assert.deepEqual(ids(selected(field({ minters_contains: variable('users') }), rows, { users: ['0xm1'] }, unfaked)), ['6'])
  assert.deepEqual(ids(selected(field({ creator: { kind: 'StringValue', value: '0xc2' } }), rows, {}, unfaked)), ['7'])
  assert.deepEqual(ids(selected({ name: { value: 'collections' }, arguments: [] }, rows, {}, unfaked)), ['6', '7'])
  assert.deepEqual(unfaked, [])
  assert.deepEqual(selected(field({ isApproved: { kind: 'BooleanValue', value: true } }), rows, {}, unfaked), [])
  assert.deepEqual(unfaked, ['graph collections where isApproved', 'graph collections where isApproved'])
})

test('the fake subgraph compares ids and creators exactly and refuses pages The Graph refuses', () => {
  const { selected } = require(script)
  const rows = [{ id: '0xab', creator: '0xc1', managers: [], minters: [], collection: { id: '0xab' } }]
  const string = value => ({ kind: 'StringValue', value })
  const field = (where, args = {}) => ({
    name: { value: 'items' },
    arguments: [
      { name: { value: 'where' }, value: { kind: 'ObjectValue', fields: Object.entries(where).map(([key, value]) => ({ name: { value: key }, value })) } },
      ...Object.entries(args).map(([key, value]) => ({ name: { value: key }, value: { kind: 'IntValue', value } }))
    ]
  })
  const unfaked = []
  const ids = (where, args) => selected(field(where, args), rows, {}, unfaked).map(row => row.id)
  assert.deepEqual(ids({ id: string('0xAB') }), [], 'a mixed-case id matches no lowercase id')
  assert.deepEqual(ids({ id_in: { kind: 'ListValue', values: [string('0xAB')] } }), [])
  assert.deepEqual(ids({ creator: string('0xC1') }), [], 'a mixed-case creator matches no lowercase creator')
  assert.deepEqual(ids({ collection: string('0xAB') }), [])
  assert.deepEqual(ids({ id: string('0xab'), creator: string('0xc1'), collection: string('0xab') }), ['0xab'])
  assert.deepEqual(ids({}, { first: '1000', skip: '5000' }), [])
  assert.throws(() => ids({}, { first: '1001' }), /first/)
  assert.throws(() => ids({}, { skip: '5001' }), /skip/)
  assert.deepEqual(unfaked, [])
})

test('cases run in the order of their numbers, which no two share', () => {
  const { caseOrderErrors, CASES } = require(script)
  assert.deepEqual(caseOrderErrors(CASES), [])
  assert.deepEqual(caseOrderErrors([{ name: '99-last' }, { name: '100-after' }]), [])
  assert.deepEqual(caseOrderErrors([{ name: '100-after' }, { name: '99-last' }]), ['99-last runs after 100-after'])
  assert.deepEqual(caseOrderErrors([{ name: '05-one' }, { name: '5-two' }]), ['5-two runs after 05-one'])
  assert.deepEqual(caseOrderErrors([{ name: 'unnumbered' }]), ['unnumbered: a case name starts with its number'])
})

test('every source capture a gap of the ledger lists is a case the script records', () => {
  const { ledgerCaptureErrors, CASES } = require(script)
  const ledger = JSON.parse(fs.readFileSync(path.join(__dirname, '../../services/catalyst/tests/fixtures/conformance/builder/parity.json'), 'utf8'))
  assert.deepEqual(ledgerCaptureErrors(ledger, CASES), [])
  const listing = captures => ({ deliberate_gaps: { kept: { captures } }, open_gaps: { missing: { captures: ['source/02-gone.json'] } } })
  assert.deepEqual(ledgerCaptureErrors(listing(['source/01-info.json', 'live/01-public.json']), [{ name: '01-info' }]), [
    'open gap missing: lists source/02-gone.json, which is no capture'
  ])
  assert.deepEqual(ledgerCaptureErrors(listing(['source/03-renamed.json']), [{ name: '03-renamed-case' }]), [
    'deliberate gap kept: lists source/03-renamed.json, which is no capture',
    'open gap missing: lists source/02-gone.json, which is no capture'
  ])
})

test('the fake subgraph filters, orders and pages as The Graph does', () => {
  const { selected } = require(script)
  const rows = [
    { id: '0xb', creator: '0xc', managers: ['0xa1', '0xa2'], minters: [] },
    { id: '0xa', creator: '0xc', managers: ['0xa1'], minters: ['0xm1'] },
    { id: '0xc', creator: '0xd', managers: [], minters: [] }
  ]
  const list = values => ({ kind: 'ListValue', values: values.map(value => ({ kind: 'StringValue', value })) })
  const field = (where, args = {}) => ({
    name: { value: 'collections' },
    arguments: [
      { name: { value: 'where' }, value: { kind: 'ObjectValue', fields: Object.entries(where).map(([key, value]) => ({ name: { value: key }, value })) } },
      ...Object.entries(args).map(([key, value]) => ({ name: { value: key }, value }))
    ]
  })
  const unfaked = []
  const ids = (where, args) => selected(field(where, args), rows, {}, unfaked).map(row => row.id)
  assert.deepEqual(ids({ managers_contains: list([]) }), ['0xa', '0xb', '0xc'], 'an empty _contains is contained in every list')
  assert.deepEqual(ids({ managers_contains: list(['0xa1', '0xa2']) }), ['0xb'], '_contains needs every value')
  assert.deepEqual(ids({ managers_contains: list(['0xA1']) }), [], '_contains compares exactly')
  assert.deepEqual(ids({ managers_contains_nocase: list(['0xA1']) }), ['0xa', '0xb'], '_contains_nocase ignores case')
  assert.deepEqual(ids({ managers_contains_nocase: list([]) }), ['0xa', '0xb', '0xc'])
  assert.deepEqual(ids({ id_in: list([]) }), [], 'an empty _in matches nothing')
  assert.deepEqual(ids({ id_gt: { kind: 'StringValue', value: '0xa' } }), ['0xb', '0xc'])
  assert.deepEqual(ids({}, { first: { kind: 'IntValue', value: '2' }, skip: { kind: 'IntValue', value: '1' } }), ['0xb', '0xc'], 'ordered by id, then skipped and bounded')
  assert.deepEqual(ids({}, { orderBy: { kind: 'EnumValue', value: 'id' }, orderDirection: { kind: 'EnumValue', value: 'desc' } }), ['0xc', '0xb', '0xa'])
  assert.deepEqual(unfaked, [])
})

test('the fake third-party subgraph answers each seeded provider with its approval, kind and managers', () => {
  const { registryRows, selected } = require(script)
  const rows = registryRows([
    { id: 'urn:decentraland:matic:collections-thirdparty:b', name: 'B', managers: ['manager'] },
    { id: 'urn:decentraland:matic:collections-thirdparty:a', name: 'A', managers: ['manager', 'minter'], approved: false, programmatic: true }
  ], role => `0x${role}`)
  assert.deepEqual(rows.map(row => [row.id.slice(-1), row.isApproved, row.isProgrammatic, row.managers]), [
    ['b', true, false, ['0xmanager']],
    ['a', false, true, ['0xmanager', '0xminter']]
  ])
  assert.deepEqual(rows[1].metadata, { type: 'third_party_v2', thirdParty: { name: 'A', description: '', contracts: [] } })
  assert.ok(!('__typename' in rows[1]), 'The Graph answers __typename only when a query selects it, and upstream\'s don\'t')
  const variable = name => ({ kind: 'Variable', name: { value: name } })
  const field = (where, args = []) => ({
    name: { value: 'thirdParties' },
    arguments: [{ name: { value: 'where' }, value: { kind: 'ObjectValue', fields: Object.entries(where).map(([key, value]) => ({ name: { value: key }, value })) } }, ...args]
  })
  const unfaked = []
  const first = { name: { value: 'first' }, value: { kind: 'IntValue', value: '1' } }
  assert.deepEqual(selected(field({ id: variable('id') }, [first]), rows, { id: rows[1].id }, unfaked).map(row => row.id), [rows[1].id])
  assert.deepEqual(selected(field({ managers_contains_nocase: variable('managers') }), rows, { managers: ['0xMINTER'] }, unfaked).map(row => row.id), [rows[1].id])
  assert.deepEqual(unfaked, [])
})

test('a case\'s cheque is its signer\'s signature of the registry\'s typed data for its provider, quantity and salt', async () => {
  const { chequeTypedData, withCheque } = require(script)
  const salt = `0x${'0b'.repeat(32)}`
  const typed = chequeTypedData('urn:decentraland:matic:collections-thirdparty:registered', 2, salt)
  assert.deepEqual(typed.domain, {
    name: 'Decentraland Third Party Registry',
    version: '1',
    verifyingContract: '0x1C436C1EFb4608dFfDC8bace99d2B03c314f3348',
    salt: `0x${'00'.repeat(31)}89`
  })
  assert.deepEqual(typed.types, { ConsumeSlots: [{ name: 'thirdPartyId', type: 'string' }, { name: 'qty', type: 'uint256' }, { name: 'salt', type: 'bytes32' }] })
  assert.deepEqual(typed.value, { thirdPartyId: 'urn:decentraland:matic:collections-thirdparty:registered', qty: 2, salt })
  const signed = []
  const wallet = { _signTypedData: async (domain, types, value) => { signed.push([domain, types, value]); return '0xsigned' } }
  const item = { name: 'case', body: { itemIds: ['a'] }, cheque: { signer: 'manager', provider: 'urn:decentraland:matic:collections-thirdparty:registered', qty: 2, salt } }
  assert.deepEqual(await withCheque(item, { manager: { root: wallet } }), { itemIds: ['a'], cheque: { qty: 2, salt, signature: '0xsigned' } })
  assert.deepEqual(signed, [[typed.domain, typed.types, typed.value]])
  const sent = { ...item, cheque: { ...item.cheque, sentQty: 3 } }
  assert.deepEqual((await withCheque(sent, { manager: { root: wallet } })).cheque.qty, 3)
  assert.deepEqual(await withCheque({ name: 'plain', body: { itemIds: [] } }, {}), { itemIds: [] })
})

test('a multipart case sends its parts as form data and records them', async () => {
  const { requestBody } = require(script)
  const parts = [{ name: 'bafkreiexample', filename: 'blob', type: 'application/octet-stream', text: 'conformance bytes' }]
  const { body, recorded } = requestBody({ name: '01-upload', multipart: parts })
  assert.ok(body instanceof FormData)
  const file = body.get('bafkreiexample')
  assert.equal(file.name, 'blob')
  assert.equal(file.type, 'application/octet-stream')
  assert.equal(await file.text(), 'conformance bytes')
  assert.deepEqual(recorded, { body: null, multipart: parts })
  assert.deepEqual(requestBody({ name: '02-json', body: { a: 1 } }).recorded, { body: { a: 1 } })
  assert.equal(requestBody({ name: '03-none' }).body, undefined)
  assert.throws(() => requestBody({ name: '04-both', body: {}, multipart: parts }), /04-both: a case sends a body or multipart parts, not both/)
  assert.throws(() => requestBody({ name: '05-bad', multipart: [{ name: 'x', text: 'y' }] }), /05-bad: a multipart part names name, filename, type and text/)
})

test('the fake S3 stores, copies and deletes objects as the AWS SDK expects', () => {
  const { s3Answer } = require(script)
  const objects = new Map()
  const put = s3Answer('PUT', '/s3/conformance/upload-1', {}, Buffer.from('abc'), objects)
  assert.equal(put.status, 200)
  assert.ok(put.headers.etag)
  const copy = s3Answer('PUT', '/s3/conformance/contents/bafk1', { 'x-amz-copy-source': '/conformance/upload-1' }, Buffer.alloc(0), objects)
  assert.equal(copy.status, 200)
  assert.match(copy.body, /<CopyObjectResult>/)
  assert.equal(s3Answer('DELETE', '/s3/conformance/upload-1', {}, Buffer.alloc(0), objects).status, 204)
  assert.deepEqual([...objects.keys()], ['contents/bafk1'])
  assert.equal(objects.get('contents/bafk1').toString(), 'abc')
  assert.deepEqual(s3Answer('GET', '/s3/conformance/contents/bafk1', {}, Buffer.alloc(0), objects), { status: 200, headers: {}, body: Buffer.from('abc') })
  assert.equal(s3Answer('GET', '/s3/conformance/contents/bafk2', {}, Buffer.alloc(0), objects).status, 404)
  assert.equal(s3Answer('HEAD', '/s3/conformance/contents/bafk1', {}, Buffer.alloc(0), objects), undefined)
})

test('a redirect into the fake bucket is recorded under the bucket URL upstream answers for a reserved storage host', () => {
  const { bucketNamed } = require(script)
  const bucket = 'http://127.0.0.1:41234/s3/conformance'
  assert.equal(bucketNamed(`Moved Permanently. Redirecting to ${bucket}/contents/bafk1`, bucket), 'Moved Permanently. Redirecting to https://s3.example/conformance/contents/bafk1')
  assert.equal(bucketNamed('unchanged', undefined), 'unchanged')
})

test('the fake subgraph lists the seeded rarities with the fields upstream selects', () => {
  const { FAKED_GRAPH_FIELDS, rarityRows, selected } = require(script)
  assert.ok(FAKED_GRAPH_FIELDS.has('rarities'))
  const rows = rarityRows([
    { name: 'epic', price: '20', max_supply: '1000', mana_price: '8' },
    { name: 'common', price: '10', max_supply: '100000', mana_price: '4' }
  ])
  assert.deepEqual(rows, [
    { id: 'epic', name: 'epic', price: '20', maxSupply: '1000' },
    { id: 'common', name: 'common', price: '10', maxSupply: '100000' }
  ])
  const unfaked = []
  assert.deepEqual(selected({ name: { value: 'rarities' }, arguments: [] }, rows, {}, unfaked).map(r => r.id), ['common', 'epic'], 'ordered by id')
  assert.deepEqual(unfaked, [])
})

test('the fake oracle answers getRarityByName as the rarities-with-oracle contract encodes it', () => {
  const { oracleAnswer } = require(script)
  const rarities = [{ name: 'common', price: '10', max_supply: '100000', mana_price: '4000000000000000000' }]
  const call = data => [{ to: '0xA9158E22F89Bb3F69c5600338895Cb5FB81e5090', data }, 'latest']
  const common = '0xb5262333' +
    '0000000000000000000000000000000000000000000000000000000000000020' +
    '0000000000000000000000000000000000000000000000000000000000000006' +
    '636f6d6d6f6e0000000000000000000000000000000000000000000000000000'
  assert.deepEqual(oracleAnswer(call(common), rarities), {
    result: '0x' +
      '0000000000000000000000000000000000000000000000000000000000000020' +
      '0000000000000000000000000000000000000000000000000000000000000060' +
      '00000000000000000000000000000000000000000000000000000000000186a0' +
      '0000000000000000000000000000000000000000000000003782dace9d900000' +
      '0000000000000000000000000000000000000000000000000000000000000006' +
      '636f6d6d6f6e0000000000000000000000000000000000000000000000000000'
  })
  const epic = '0xb5262333' +
    '0000000000000000000000000000000000000000000000000000000000000020' +
    '0000000000000000000000000000000000000000000000000000000000000004' +
    '6570696300000000000000000000000000000000000000000000000000000000'
  assert.deepEqual(oracleAnswer(call(epic), rarities), { error: { code: 3, message: 'execution reverted' } })
  assert.equal(oracleAnswer([{ to: '0x0000000000000000000000000000000000000001', data: common }, 'latest'], rarities), undefined)
  assert.equal(oracleAnswer(call('0xdeadbeef'), rarities), undefined)
})

test('the fake warehouse records an event sent with its token and refuses anything else', () => {
  const { warehouseAnswer, WAREHOUSE_TOKEN } = require(script)
  const received = []
  const event = { context: 'builder', event: 'publish_collection_tos', body: { email: 'jane.doe@example.com' } }
  const sent = Buffer.from(JSON.stringify(event))
  assert.deepEqual(warehouseAnswer('POST', '/warehouse', { 'x-token': WAREHOUSE_TOKEN }, sent, received), { status: 200, body: {} })
  assert.deepEqual(received, [event])
  assert.equal(warehouseAnswer('POST', '/warehouse', { 'x-token': 'other' }, sent, received).status, 401)
  assert.equal(warehouseAnswer('POST', '/warehouse', {}, sent, received).status, 401)
  assert.equal(warehouseAnswer('POST', '/warehouse', { 'x-token': WAREHOUSE_TOKEN }, Buffer.from('not json'), received).status, 400)
  assert.equal(warehouseAnswer('GET', '/warehouse', { 'x-token': WAREHOUSE_TOKEN }, Buffer.alloc(0), received), undefined)
  assert.equal(warehouseAnswer('POST', '/elsewhere', { 'x-token': WAREHOUSE_TOKEN }, sent, received), undefined)
  assert.equal(received.length, 1)
})

test('the seed\'s curations and forum fields reach upstream\'s tables', async () => {
  const { seedDatabase, readSeed } = require(script)
  assert.deepEqual(Object.keys(readSeed()).filter(key => key.endsWith('curations')), ['collection_curations', 'item_curations'])
  const queries = []
  const client = { async query(sql, values) { queries.push([sql.split(' ').slice(0, 3).join(' '), values]) } }
  const collection = { id: 'c1', name: 'C', owner: 'owner', salt: '0x00', contract_address: null, forum_link: 'https://forum.example.org/t/1', forum_id: 1, created_at: 'a', updated_at: 'b' }
  await seedDatabase(client, {
    third_parties: [],
    collections: [collection],
    items: [],
    collection_curations: [{ id: 'cc', collection_id: 'c1', status: 'pending', assignee: 'committee', created_at: 'c', updated_at: 'd' }],
    item_curations: [{ id: 'ic', item_id: 'i1', status: 'approved', content_hash: 'ab', is_mapping_complete: true, created_at: 'e', updated_at: 'f' }]
  }, { owner: { address: '0xowner' }, committee: { address: '0xcommittee' } })
  assert.deepEqual(queries.map(([into]) => into), ['INSERT INTO collections', 'INSERT INTO collection_curations', 'INSERT INTO item_curations'])
  assert.deepEqual(queries[0][1].slice(-4), ['https://forum.example.org/t/1', 1, 'a', 'b'])
  assert.deepEqual(queries[1][1], ['cc', 'c1', 'pending', '0xcommittee', 'c', 'd'])
  assert.deepEqual(queries[2][1], ['ic', 'i1', 'approved', 'ab', true, 'e', 'f'])
})

test('the fake Slack answers chat.postMessage as Slack\'s Web API does and records each message it took', () => {
  const { slackAnswer, SLACK_TOKEN, SLACK_CHANNEL } = require(script)
  const auth = { authorization: `Bearer ${SLACK_TOKEN}` }
  const body = value => Buffer.from(JSON.stringify(value))
  const received = []
  const first = slackAnswer('POST', '/api/chat.postMessage', auth, body({ channel: SLACK_CHANNEL, text: 'one', unfurl_links: false }), received)
  assert.deepEqual(first, { status: 200, body: { ok: true, channel: SLACK_CHANNEL, ts: '1767225600.000001', message: { type: 'message', text: 'one' } } })
  const second = slackAnswer('POST', '/api/chat.postMessage', auth, body({ channel: SLACK_CHANNEL, text: 'two' }), received)
  assert.equal(second.body.ts, '1767225600.000002')
  assert.deepEqual(received, [{ channel: SLACK_CHANNEL, text: 'one', unfurl_links: false }, { channel: SLACK_CHANNEL, text: 'two' }])
  const refusal = (headers, sent) => slackAnswer('POST', '/api/chat.postMessage', headers, sent, received)
  assert.deepEqual(refusal({ authorization: 'Bearer xoxb-other' }, body({ channel: SLACK_CHANNEL, text: 'x' })), { status: 200, body: { ok: false, error: 'invalid_auth' } })
  assert.deepEqual(refusal({}, body({ channel: SLACK_CHANNEL, text: 'x' })), { status: 200, body: { ok: false, error: 'not_authed' } })
  assert.deepEqual(refusal(auth, body({ channel: '', text: 'x' })), { status: 200, body: { ok: false, error: 'channel_not_found' } })
  assert.deepEqual(refusal(auth, body({ channel: SLACK_CHANNEL })), { status: 200, body: { ok: false, error: 'no_text' } })
  assert.equal(refusal(auth, Buffer.from('not json')).body.ok, false)
  assert.equal(received.length, 2)
  assert.equal(slackAnswer('POST', '/api/chat.update', auth, body({}), received), undefined)
  assert.equal(slackAnswer('GET', '/api/chat.postMessage', auth, Buffer.alloc(0), received), undefined)
})

test('only a request to slack.com is sent to the fake Slack, over loopback http with its path, method and headers', () => {
  const { slackRoute } = require(script)
  const headers = { Authorization: 'Bearer x', 'Content-Type': 'application/json' }
  const routed = slackRoute({ protocol: 'https:', hostname: 'slack.com', host: 'slack.com', port: null, path: '/api/chat.postMessage', method: 'POST', headers, agent: {} }, 4321)
  assert.deepEqual(routed, { protocol: 'http:', hostname: '127.0.0.1', port: 4321, path: '/slack/api/chat.postMessage', method: 'POST', headers })
  assert.equal(slackRoute({ protocol: 'https:', hostname: 'api.slack.com', path: '/api/chat.postMessage' }, 4321), undefined)
  assert.equal(slackRoute({ protocol: 'https:', hostname: 'example.org', path: '/' }, 4321), undefined)
  assert.equal(slackRoute('https://slack.com/api/chat.postMessage', 4321), undefined)
})

test('a capture records the Slack messages of its case in order, and nothing when there are none', () => {
  const { slackRecord } = require(script)
  assert.deepEqual(slackRecord([]), {})
  assert.deepEqual(slackRecord([{ text: 'b' }, { text: 'a' }]), { slack: [{ text: 'b' }, { text: 'a' }] })
})

test('a seeded item\'s local content hash reaches upstream\'s items table', async () => {
  const { seedDatabase, readSeed } = require(script)
  const queries = []
  const client = { async query(sql, values) { queries.push([sql, values]) } }
  const item = { id: 'i1', name: 'I', owner: 'owner', collection_id: null, blockchain_item_id: null, data: {}, metrics: {}, thumbnail: 't', created_at: 'a', updated_at: 'b' }
  const hash = 'a'.repeat(64)
  await seedDatabase(client, { third_parties: [], collections: [], items: [item, { ...item, id: 'i2', local_content_hash: hash }], collection_curations: [], item_curations: [] }, { owner: { address: '0xowner' } })
  assert.match(queries[0][0], /local_content_hash/)
  assert.deepEqual(queries.map(([, values]) => values.at(-1)), [null, hash])
  const seeded = readSeed().items.filter(row => row.local_content_hash)
  assert.deepEqual(seeded.map(row => row.id.slice(-3)), ['4a1', '4a2', '4a3', '4b1', '4b2', '4c1', '4d1', '4d2', '4e1', '5aa', '5ab', '5ac', '5ad'])
  assert.ok(seeded.every(row => /^[0-9a-f]{64}$/.test(row.local_content_hash)))
})

test('the fake forum answers the calls upstream makes as Discourse does and records each one', () => {
  const { forumAnswer, forumState, FORUM_KEY, FORUM_USERNAME, FORUM_CATEGORY } = require(script)
  const forum = forumState([{ forum_link: 'https://forum.example.org/t/forum-hats/401', forum_id: 401 }, { forum_link: null, forum_id: null }])
  const body = value => Buffer.from(JSON.stringify(value))
  const key = { 'api-key': FORUM_KEY }
  const user = { ...key, 'api-username': FORUM_USERNAME }
  const created = forumAnswer('POST', '/posts.json', key, body({ title: 'Collection \'Red Hats\' is ready!', raw: '# Red', category: FORUM_CATEGORY }), forum)
  assert.deepEqual(created, { status: 200, body: { id: 5001, topic_id: 1001, topic_slug: 'collection-red-hats-is-ready', post_number: 1 } })
  assert.deepEqual(forumAnswer('POST', '/posts.json', key, body({ title: 'collection \'red hats\' is ready!', raw: 'x', category: FORUM_CATEGORY }), forum),
    { status: 422, body: { action: 'create_post', errors: ['Title has already been used'] } })
  assert.deepEqual(forumAnswer('GET', '/posts/401.json', user, Buffer.alloc(0), forum), { status: 200, body: { id: 401, topic_id: 401, raw: '' } })
  assert.deepEqual(forumAnswer('GET', '/posts/5001.json', user, Buffer.alloc(0), forum), { status: 200, body: { id: 5001, topic_id: 1001, raw: '' } })
  assert.deepEqual(forumAnswer('POST', '/posts.json', key, body({ topic_id: 401, raw: 'assigned' }), forum),
    { status: 200, body: { id: 5002, topic_id: 401, topic_slug: 'forum-hats', post_number: 2 } })
  const notFound = { status: 404, body: { errors: ['The requested URL or resource could not be found.'], error_type: 'not_found' } }
  assert.deepEqual(forumAnswer('POST', '/posts.json', key, body({ topic_id: 9, raw: 'x' }), forum), notFound)
  assert.deepEqual(forumAnswer('GET', '/posts/9.json', user, Buffer.alloc(0), forum), notFound)
  assert.equal(forumAnswer('POST', '/posts.json', {}, body({ topic_id: 401, raw: 'x' }), forum).status, 403)
  assert.equal(forumAnswer('POST', '/posts.json', { 'api-key': 'sk-other' }, body({ topic_id: 401, raw: 'x' }), forum).status, 403)
  assert.equal(forumAnswer('POST', '/posts.json', key, body({ raw: 'x' }), forum).status, 422)
  assert.equal(forumAnswer('POST', '/posts.json', key, Buffer.from('{'), forum).status, 422)
  assert.equal(forumAnswer('PUT', '/posts/401.json', user, body({ raw: 'x' }), forum), undefined)
  assert.equal(forumAnswer('GET', '/t/401.json', user, Buffer.alloc(0), forum), undefined)
  assert.deepEqual(forum.calls.map(call => [call.method, call.path, call.username]), [
    ['POST', '/posts.json', null], ['POST', '/posts.json', null], ['GET', '/posts/401.json', FORUM_USERNAME], ['GET', '/posts/5001.json', FORUM_USERNAME],
    ['POST', '/posts.json', null], ['POST', '/posts.json', null], ['GET', '/posts/9.json', FORUM_USERNAME], ['POST', '/posts.json', null],
    ['POST', '/posts.json', null], ['POST', '/posts.json', null], ['POST', '/posts.json', null]
  ])
  assert.deepEqual(forum.calls[0].body, { title: 'Collection \'Red Hats\' is ready!', raw: '# Red', category: FORUM_CATEGORY })
  assert.equal(forum.calls.at(-1).body, null)
})

test('only a request to the forum\'s host is sent to the fake forum, over loopback http under /forum', () => {
  const { forumRoute } = require(script)
  const headers = { 'Api-Key': 'k' }
  assert.deepEqual(forumRoute({ protocol: 'https:', hostname: 'forum.example.org', path: '/posts.json', method: 'POST', headers }, 4321),
    { protocol: 'http:', hostname: '127.0.0.1', port: 4321, path: '/forum/posts.json', method: 'POST', headers })
  assert.equal(forumRoute({ protocol: 'https:', hostname: 'forum.example.org.evil.example', path: '/' }, 4321), undefined)
  assert.equal(forumRoute({ protocol: 'https:', hostname: 'slack.com', path: '/' }, 4321), undefined)
  assert.equal(forumRoute('https://forum.example.org/posts.json', 4321), undefined)
})

test('a capture records the forum calls of its case in order, and nothing when there are none', () => {
  const { forumRecord } = require(script)
  assert.deepEqual(forumRecord([]), {})
  const calls = [{ method: 'GET', path: '/posts/1.json', username: 'u', body: null }]
  assert.deepEqual(forumRecord(calls), { forum: calls })
})

test('forum draft and no-topic cases address collections not deleted earlier in the capture', () => {
  const { CASES } = require(script)
  for (const prefix of ['435-', '444-', '450-']) {
    const index = CASES.findIndex(item => item.name.startsWith(prefix))
    const collection = CASES[index].path.split('/').slice(0, 4).join('/')
    assert.ok(!CASES.slice(0, index).some(item => item.method === 'DELETE' && item.path === collection), CASES[index].name)
  }
})

test('the events topic records a validation request on its topic and refuses anything else', () => {
  const { snsAnswer, queueRecord, SNS_TOPIC } = require(script)
  const manifest = { collectionId: 'c011ec71-0000-4000-8000-000000000302', items: [{ itemId: 'i' }] }
  const publish = (fields, message) => Buffer.from(new URLSearchParams({
    Action: 'Publish',
    TopicArn: SNS_TOPIC,
    Message: JSON.stringify({ type: 'builder', subType: 'collection-validation-requested', key: manifest.collectionId, timestamp: 1, metadata: { validationId: 'v', ...manifest }, ...message }),
    ...fields
  }).toString())
  const queued = []
  assert.equal(snsAnswer('POST', '/sns/', publish({}, {}), queued).status, 200)
  assert.deepEqual(queued, [manifest], 'recorded without its random validationId')
  for (const [label, bytes] of [
    ['another topic', publish({ TopicArn: 'arn:aws:sns:us-east-1:000000000000:other' }, {})],
    ['another action', publish({ Action: 'Subscribe' }, {})],
    ['another event', publish({}, { subType: 'other' })],
    ['a key that is not the collection', publish({}, { key: 'other' })]
  ]) {
    assert.equal(snsAnswer('POST', '/sns', bytes, queued), undefined, label)
  }
  assert.equal(snsAnswer('GET', '/sns', publish({}, {}), queued), undefined)
  assert.equal(queued.length, 1)
  assert.deepEqual(queueRecord([]), {})
  assert.deepEqual(queueRecord(queued), { queue: queued })
})

test('a verdict case sends its collection\'s newest start and records the body it names, and only review cases turn the flow on', async () => {
  const { sentBody, REVIEW_CASES, CASES } = require(script)
  const verdict = REVIEW_CASES.find(item => item.validation_of)
  const asked = []
  const sent = JSON.parse(await sentBody(verdict, JSON.stringify(verdict.body), async collection => {
    asked.push(collection)
    return 'c011ec71-0000-4000-8000-0000000009ff'
  }))
  assert.deepEqual(asked, [verdict.validation_of])
  assert.deepEqual(sent, { ...verdict.body, validationId: 'c011ec71-0000-4000-8000-0000000009ff' })
  assert.notEqual(verdict.body.validationId, sent.validationId)
  const plain = CASES.find(item => !item.validation_of && item.body)
  assert.equal(await sentBody(plain, 'unchanged', () => assert.fail('read a start')), 'unchanged')
  assert.ok(CASES.filter(item => item.review_flow).every(item => REVIEW_CASES.includes(item)))
})

test('the fake newsletter service records each subscription as sent and fails only its failing email', () => {
  const { newsletterAnswer, newsletterRecord, NEWSLETTER_PUBLICATION, NEWSLETTER_FAILING_EMAIL } = require(script)
  const received = []
  const path = `/v2/publications/${NEWSLETTER_PUBLICATION}/subscriptions`
  const headers = { authorization: 'Bearer example-newsletter-key', accept: 'application/json', 'content-type': 'application/json' }
  const sent = { email: 'jane.doe@example.com', reactivate_existing: true, send_welcome_email: false, utm_source: 'auth', utm_medium: 'organic' }
  assert.equal(newsletterAnswer('POST', path, headers, Buffer.from(JSON.stringify(sent)), received).status, 201)
  assert.equal(newsletterAnswer('POST', path, headers, Buffer.from(JSON.stringify({ ...sent, email: NEWSLETTER_FAILING_EMAIL })), received).status, 500)
  assert.equal(newsletterAnswer('POST', '/v2/publications/other/subscriptions', headers, Buffer.from('{}'), received), undefined)
  assert.equal(newsletterAnswer('GET', path, headers, Buffer.alloc(0), received), undefined)
  assert.deepEqual(received[0], { method: 'POST', path, authorization: headers.authorization, accept: 'application/json', content_type: 'application/json', body: sent })
  assert.equal(received.length, 2)
  assert.deepEqual(newsletterRecord([]), {})
  assert.deepEqual(newsletterRecord(received), { newsletter: received })
})

test('a draft save sends the initialize calldata its creator\'s capture recorded, byte for byte', () => {
  const { CASES, initializeCalldata } = require(script)
  const folder = path.resolve(__dirname, '../../services/catalyst/tests/fixtures/conformance/builder/source')
  const saves = CASES.filter(item => String(item.body?.data).startsWith('0xd14fee77'))
  assert.equal(saves.length, 15)
  const recorded = saves.map(item => JSON.parse(fs.readFileSync(path.join(folder, `${item.name}.json`), 'utf8')).request.body.data)
  assert.deepEqual(saves.map(item => item.body.data), recorded)
  assert.deepEqual([...new Set(recorded)].sort(), [
    initializeCalldata('0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e'),
    initializeCalldata('0xcd6884d4258edc833f31ac773e24affe6a7d3154')
  ].sort())
})

test('every committed capture and the seed parse into the shape the replay reads', () => {
  const { CASES, readSeed } = require(script)
  const folder = path.resolve(__dirname, '../../services/catalyst/tests/fixtures/conformance/builder')
  const { BUILDER_REVISION } = require('../social-capture-common.cjs')
  const optional = { volatile_equal: 'array', auth: 'string', sign: 'object', slack: 'array', queue: 'array', forum: 'array', validator_sign: 'object', review_flow: 'boolean', validation_of: 'string', newsletter: 'array', repeat: 'number' }
  const kind = value => (Array.isArray(value) ? 'array' : value === null ? 'null' : typeof value)
  const strings = value => kind(value) === 'object' && Object.values(value).every(entry => typeof entry === 'string')
  const problems = []
  const files = fs.readdirSync(path.join(folder, 'source')).sort()
  assert.deepEqual(files, CASES.map(item => `${item.name}.json`).sort())
  for (const name of files) {
    const capture = JSON.parse(fs.readFileSync(path.join(folder, 'source', name), 'utf8'))
    const problem = message => problems.push(`${name}: ${message}`)
    for (const [key, value] of Object.entries(capture)) {
      if (['description', 'captured_from', 'captured_at'].includes(key)) { if (typeof value !== 'string') problem(`${key} is not a string`) }
      else if (key === 'volatile_paths') { if (!Array.isArray(value) || !value.every(entry => typeof entry === 'string')) problem('volatile_paths is not a list of strings') }
      else if (key === 'request' || key === 'response') continue
      else if (!optional[key]) problem(`unknown field ${key}`)
      else if (kind(value) !== optional[key]) problem(`${key} is not ${optional[key]}`)
    }
    if (capture.captured_from !== `github.com/decentraland/builder-server@${BUILDER_REVISION}`) problem('captured_from is not the pinned upstream')
    const { request, response } = capture
    if (kind(request) !== 'object' || kind(response) !== 'object') { problem('request or response is not an object'); continue }
    if (typeof request.method !== 'string' || typeof request.path !== 'string' || !request.path.startsWith('/')) problem('request method or path')
    if (kind(request.query) !== 'object' || !Object.values(request.query).every(value => typeof value === 'string' || (Array.isArray(value) && value.every(entry => typeof entry === 'string')))) problem('request query')
    if (!strings(request.headers)) problem('request headers')
    if (request.multipart !== undefined) {
      if (request.body !== null) problem('a multipart request also has a body')
      for (const part of request.multipart) {
        if (Object.keys(part).sort().join() !== 'filename,name,text,type' || !strings(part)) problem('multipart part')
      }
    }
    const unknownRequest = Object.keys(request).filter(key => !['method', 'path', 'query', 'headers', 'body', 'multipart'].includes(key))
    if (unknownRequest.length) problem(`unknown request fields ${unknownRequest}`)
    if (!Number.isInteger(response.status) || !strings(response.headers)) problem('response status or headers')
    const bodies = ['body_json', 'body_bytes_b64'].filter(key => Object.hasOwn(response, key))
    if (bodies.length !== 1) problem('response carries not exactly one body')
    if (bodies[0] === 'body_bytes_b64' && Buffer.from(response.body_bytes_b64, 'base64').toString('base64') !== response.body_bytes_b64) problem('body_bytes_b64 is not base64')
    const unknownResponse = Object.keys(response).filter(key => !['status', 'headers', 'body_json', 'body_bytes_b64'].includes(key))
    if (unknownResponse.length) problem(`unknown response fields ${unknownResponse}`)
  }
  assert.deepEqual(problems, [])
  assert.deepEqual(fs.readdirSync(folder).sort(), ['README.md', 'parity.json', 'seed.json', 'source'])
  const seed = readSeed()
  const objects = new Set(['wallets', 'chain'])
  for (const [key, value] of Object.entries(seed)) assert.equal(kind(value), objects.has(key) ? 'object' : 'array', key)
  assert.ok(Object.values(seed.wallets).every(address => /^0x[0-9a-f]{40}$/.test(address)))
})

test('the edge cases send their origin and keep the headers they are about, and only the limiter case repeats', () => {
  const { EDGE_CASES } = require(script)
  for (const item of EDGE_CASES.filter(item => item.name.includes('-cors-'))) {
    assert.ok(item.send.origin, item.name)
    assert.ok(item.keep.includes('access-control-allow-origin'), item.name)
  }
  assert.deepEqual(EDGE_CASES.filter(item => item.repeat).map(item => [item.name, item.repeat, item.repeat_status]), [['521-review-writes-per-minute', 30, 404]])
})
