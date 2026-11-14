const assert = require('node:assert/strict')
const { EventEmitter } = require('node:events')
const { PassThrough } = require('node:stream')
const { createInitDiagnostics, getInitSceneDirectory } = require('./init-diagnostics')

describe('blackbox init diagnostics', () => {
  test('isolated CI uses a distinct literal fixture without accepting an arbitrary deletion path', () => {
    assert.equal(getInitSceneDirectory(undefined), 'tmp/blackbox-scene')
    assert.equal(getInitSceneDirectory('isolated'), 'tmp/blackbox-scene-isolated')
    assert.equal(getInitSceneDirectory('/private'), 'tmp/blackbox-scene')
    assert.equal(getInitSceneDirectory('../other'), 'tmp/blackbox-scene')
  })
  test('reports the active wait before it settles and preserves its result', async () => {
    const receipts = []
    let clock = 10
    const diagnostics = createInitDiagnostics((receipt) => receipts.push(receipt), () => clock)
    let complete
    const value = {}
    const pending = diagnostics.step('init', () => new Promise((resolve) => (complete = resolve)))
    clock = 35
    diagnostics.reportPending()
    assert.deepEqual(receipts.map((receipt) => JSON.parse(receipt)), [
      { stage: 'init', state: 'start', elapsedMs: 0 },
      { stage: 'init', state: 'pending', elapsedMs: 25 }
    ])
    complete(value)
    assert.equal(await pending, value)
    diagnostics.reportPending()
    assert.equal(receipts.length, 3)
    assert.deepEqual(JSON.parse(receipts[2]), { stage: 'init', state: 'complete', elapsedMs: 25 })
  })

  test('keeps the original failure and emits only an allowlisted error code', async () => {
    const receipts = []
    const diagnostics = createInitDiagnostics((receipt) => receipts.push(receipt), () => 0)
    const error = Object.assign(new Error('https://secret.invalid/?key=private /home/private Bearer secret'), {
      code: 'ENOSPC'
    })
    await assert.rejects(diagnostics.step('init', async () => { throw error }), (actual) => actual === error)
    assert.deepEqual(JSON.parse(receipts[1]), { stage: 'init', state: 'failed', elapsedMs: 0, code: 'ENOSPC' })
    await assert.rejects(diagnostics.step('build', async () => { throw { code: 'private-token' } }))
    assert.equal(JSON.parse(receipts[3]).code, undefined)
    assert.doesNotMatch(receipts.join('\n'), /secret|private|https:|Bearer/)
  })

  test('observes the real response body separately without changing fetch arguments or response identity', async () => {
    const receipts = []
    const diagnostics = createInitDiagnostics((receipt) => receipts.push(receipt), () => 0)
    const response = new Response('zip')
    const options = { headers: { authorization: 'secret' } }
    const fetcher = diagnostics.wrapFetch({
      async fetch(url, init) {
        assert.equal(url, 'https://template.invalid/private')
        assert.equal(init, options)
        return response
      }
    })
    assert.equal(await fetcher.fetch('https://template.invalid/private', options), response)
    assert.equal(Buffer.from(await response.arrayBuffer()).toString(), 'zip')
    assert.deepEqual(receipts.map((receipt) => JSON.parse(receipt).stage), [
      'template-fetch', 'template-fetch', 'template-body', 'template-body'
    ])
    assert.doesNotMatch(receipts.join('\n'), /secret|private|https:|authorization/)
  })

  test('reports body and archive write failures without replacing them', async () => {
    const receipts = []
    const diagnostics = createInitDiagnostics((receipt) => receipts.push(receipt), () => 0)
    const failure = new Error('private')
    const fetcher = diagnostics.wrapFetch({ async fetch() { return { async arrayBuffer() { throw failure } } } })
    const response = await fetcher.fetch('private')
    await assert.rejects(response.arrayBuffer(), (actual) => actual === failure)
    const fs = diagnostics.wrapFs({ async writeFile() { throw failure } })
    await assert.rejects(fs.writeFile('/private/temp-zip-project.zip', Buffer.from('zip')), (actual) => actual === failure)
    assert.equal(JSON.parse(receipts.at(-1)).stage, 'template-write')
    assert.equal(JSON.parse(receipts.at(-1)).state, 'failed')
    assert.doesNotMatch(receipts.join('\n'), /private/)
  })

  test('delegates unrelated writes and archive writes with their receiver and result intact', async () => {
    const receipts = []
    const diagnostics = createInitDiagnostics((receipt) => receipts.push(receipt), () => 0)
    const content = Buffer.from('zip')
    const fs = {
      async writeFile(file, data, encoding) {
        assert.equal(this, fs)
        assert.equal(data, content)
        assert.equal(encoding, 'utf8')
        return file
      }
    }
    const wrapped = diagnostics.wrapFs(fs)
    assert.equal(await wrapped.writeFile('unrelated', content, 'utf8'), 'unrelated')
    assert.equal(receipts.length, 0)
    assert.equal(await wrapped.writeFile('/private/temp-zip-project.zip', content, 'utf8'), '/private/temp-zip-project.zip')
    assert.deepEqual(receipts.map((receipt) => JSON.parse(receipt).state), ['start', 'complete'])
  })

  test('observes child exit and a bounded safe error-code tail and detaches stream observers', () => {
    const receipts = []
    const diagnostics = createInitDiagnostics((receipt) => receipts.push(receipt), () => 0)
    const child = Object.assign(new EventEmitter(), { stderr: new PassThrough() })
    const args = ['install']
    const options = { cwd: '/private', env: { TOKEN: 'secret' } }
    const wrappedSpawn = diagnostics.wrapSpawn((command, actualArgs, actualOptions) => {
      assert.equal(command, 'npm')
      assert.equal(actualArgs, args)
      assert.equal(actualOptions, options)
      return child
    })
    assert.equal(wrappedSpawn('npm', args, options), child)
    assert.equal(child.listenerCount('error'), 0)
    child.stderr.write('npm error code E401\n' + 'private'.repeat(2000))
    child.stderr.write('\nnpm error code ECONNRESET\nhttps://private.invalid Bearer secret\n')
    child.emit('close', 1, null)
    assert.equal(child.stderr.listenerCount('data'), 0)
    assert.deepEqual(JSON.parse(receipts.at(-1)), {
      stage: 'dependency-child', state: 'close', elapsedMs: 0, exitCode: 1, signal: null, npmCodes: ['ECONNRESET']
    })
    assert.doesNotMatch(receipts.join('\n'), /private|secret|Bearer|https:/)
    assert.ok(receipts.join('\n').length < 512)
  })

  test('records a child signal and does not emit successful child output', () => {
    const receipts = []
    const diagnostics = createInitDiagnostics((receipt) => receipts.push(receipt), () => 0)
    const child = Object.assign(new EventEmitter(), { stderr: new PassThrough() })
    diagnostics.wrapSpawn(() => child)('npm', ['install'], {})
    child.stderr.write('npm error code E401\nsecret')
    child.emit('close', null, 'SIGTERM')
    assert.deepEqual(JSON.parse(receipts.at(-1)), {
      stage: 'dependency-child', state: 'close', elapsedMs: 0, exitCode: null, signal: 'SIGTERM', npmCodes: ['E401']
    })
    const success = Object.assign(new EventEmitter(), { stderr: new PassThrough() })
    diagnostics.wrapSpawn(() => success)('npm', ['install'], {})
    success.stderr.write('npm error code E401\nsecret')
    success.emit('close', 0, null)
    assert.equal(JSON.parse(receipts.at(-1)).npmCodes, undefined)
  })

  test('reports a stalled child with bounded safe diagnostics before close', () => {
    const receipts = []
    let clock = 0
    const diagnostics = createInitDiagnostics((receipt) => receipts.push(receipt), () => clock)
    const child = Object.assign(new EventEmitter(), { stderr: new PassThrough() })
    diagnostics.wrapSpawn(() => child)('npm', ['install'], {})
    child.stderr.write('npm error co')
    child.stderr.write('de ENOSPC\nprivate-token')
    clock = 30
    diagnostics.reportPending()
    assert.deepEqual(JSON.parse(receipts.at(-1)), {
      stage: 'dependency-child', state: 'pending', elapsedMs: 30, npmCodes: ['ENOSPC']
    })
    assert.doesNotMatch(receipts.join('\n'), /private-token/)
    child.emit('close', 1, null)
    const count = receipts.length
    diagnostics.reportPending()
    assert.equal(receipts.length, count)
  })

  test('preserves synchronous spawn failure and excludes unknown signals and npm codes', () => {
    const receipts = []
    const diagnostics = createInitDiagnostics((receipt) => receipts.push(receipt), () => 0)
    const failure = Object.assign(new Error('private'), { code: 'ENOSPC' })
    assert.throws(diagnostics.wrapSpawn(() => { throw failure }), (actual) => actual === failure)
    assert.deepEqual(JSON.parse(receipts.at(-1)), {
      stage: 'dependency-child', state: 'failed', elapsedMs: 0, code: 'ENOSPC'
    })
    const child = Object.assign(new EventEmitter(), { stderr: new PassThrough() })
    diagnostics.wrapSpawn(() => child)('npm', ['install'], {})
    child.stderr.write('npm error code PRIVATE_TOKEN\n')
    child.emit('close', null, 'private-token')
    assert.deepEqual(JSON.parse(receipts.at(-1)), {
      stage: 'dependency-child', state: 'close', elapsedMs: 0, exitCode: null, signal: null, npmCodes: []
    })
    assert.doesNotMatch(receipts.join('\n'), /private|PRIVATE/)
  })

  test('diagnostic writer failure does not replace a command result or error', async () => {
    const diagnostics = createInitDiagnostics(() => { throw new Error('reporter failure') }, () => 0)
    assert.equal(await diagnostics.step('init', async () => 42), 42)
    const failure = new Error('command failure')
    await assert.rejects(diagnostics.step('build', async () => { throw failure }), (actual) => actual === failure)
  })
})
