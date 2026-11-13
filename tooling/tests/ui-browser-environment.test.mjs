import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, writeFileSync, readFileSync, rmSync, chmodSync } from 'node:fs'
import os from 'node:os'
import { performance } from 'node:perf_hooks'
import path from 'node:path'
import { readFile } from 'node:fs/promises'
import { collectBrowserEnvironment } from '../ui-browser-environment.mjs'

const secret = 'PRIVATE_SENTINEL_DO_NOT_PRINT'
const pressure =
  'some avg10=0.00 avg60=0.00 avg300=0.00 total=0\nfull avg10=0.00 avg60=0.00 avg300=0.00 total=0\n'
function operations(overrides = {}) {
  return {
    env: { BROWSER_PATH: '/private/' + secret, XDG_CACHE_HOME: '/cache' },
    home: () => '/home/' + secret,
    uid: () => 1001,
    euid: () => 1001,
    findBrowser: async () => '/private/' + secret,
    version: async () => ({ stdout: 'Google Chrome 157.0.8103.50\n', stderr: secret }),
    stat: async () => ({ isDirectory: () => true }),
    access: async () => {},
    readFile: async () => pressure,
    ...overrides
  }
}
function error(code) {
  return Object.assign(new Error(secret), { code })
}

describe('when inspecting the browser without changing its environment', () => {
  it('should report only allowlisted metadata and read-only observations', async () => {
    const ops = operations()
    const before = { ...ops.env }
    const calls = []
    ops.findBrowser = async (candidates) => {
      calls.push(candidates)
      return '/private/' + secret
    }
    const report = await collectBrowserEnvironment(ops)
    assert.deepEqual(report.browser, {
      selected: true,
      family: 'Google Chrome',
      version: '157.0.8103.50'
    })
    assert.deepEqual(report.user, { uid: 1001, euid: 1001 })
    assert.deepEqual(report.cache.fontconfigUser, {
      directory: true,
      readable: true,
      writable: true
    })
    assert.deepEqual(report.pressure, { cpuSome10s: false, memorySome10s: false, ioSome10s: false })
    assert.equal(calls[0][0], ops.env.BROWSER_PATH)
    assert.deepEqual(ops.env, before)
    assert.ok(!JSON.stringify(report).includes(secret))
  })
  it('should distinguish actual recent pressure from a valid zero', async () => {
    const report = await collectBrowserEnvironment(
      operations({
        readFile: async (file) =>
          file.endsWith('/cpu') ? pressure.replace('avg10=0.00', 'avg10=2.50') : pressure
      })
    )
    assert.deepEqual(report.pressure, { cpuSome10s: true, memorySome10s: false, ioSome10s: false })
  })
  it('should retain unknown metadata when browser selection fails', async () => {
    const report = await collectBrowserEnvironment(
      operations({
        findBrowser: async () => {
          throw error('EACCES')
        }
      })
    )
    assert.deepEqual(report.browser, { selected: null, family: null, version: null })
    assert.ok(!JSON.stringify(report).includes(secret))
  })
  it('should retain selection but not expose failed version output', async () => {
    const report = await collectBrowserEnvironment(
      operations({
        version: async () => {
          throw Object.assign(error('ETIMEDOUT'), { stdout: secret, stderr: secret })
        }
      })
    )
    assert.deepEqual(report.browser, { selected: true, family: null, version: null })
    assert.ok(!JSON.stringify(report).includes(secret))
  })
  for (const stdout of [
    secret,
    'Google Chrome 157.0.0.1\n' + secret,
    '\u001b[31mGoogle Chrome 157.0.0.1',
    'Google Chrome Infinity',
    'Google Chrome 157.0.0.1 ' + secret,
    null
  ])
    it('should withhold an unexpected version format ' + String(stdout).length, async () => {
      const report = await collectBrowserEnvironment(
        operations({ version: async () => ({ stdout, stderr: secret }) })
      )
      assert.deepEqual(report.browser, { selected: true, family: null, version: null })
      assert.ok(!JSON.stringify(report).includes(secret))
    })
  for (const [stdout, family, version] of [
    ['Chromium 157.0.8103.0', 'Chromium', '157.0.8103.0'],
    ['Brave Browser 1.96.12', 'Brave Browser', '1.96.12'],
    ['Google Chrome for Testing 157.0.1.2', 'Google Chrome for Testing', '157.0.1.2']
  ])
    it('should accept the bounded ' + family + ' version format', async () => {
      const report = await collectBrowserEnvironment(
        operations({ version: async () => ({ stdout }) })
      )
      assert.deepEqual(report.browser, { selected: true, family, version })
    })
})

describe('when cache paths cannot be inspected', () => {
  it('should distinguish a missing directory from a denied write', async () => {
    const missing = await collectBrowserEnvironment(
      operations({
        stat: async () => {
          throw error('ENOENT')
        }
      })
    )
    assert.deepEqual(missing.cache.fontconfigSystem, {
      directory: false,
      readable: null,
      writable: null
    })
    const denied = await collectBrowserEnvironment(
      operations({
        access: async (_file, mode) => {
          if (mode & 2) throw error('EACCES')
        }
      })
    )
    assert.deepEqual(denied.cache.fontconfigSystem, {
      directory: true,
      readable: true,
      writable: false
    })
  })
  for (const code of ['EIO', 'EPERM', undefined])
    it(
      'should report unknown directory state after unexpected or denied stat ' + code,
      async () => {
        const report = await collectBrowserEnvironment(
          operations({
            stat: async () => {
              throw error(code)
            }
          })
        )
        assert.deepEqual(report.cache.fontconfigSystem, {
          directory: null,
          readable: null,
          writable: null
        })
        assert.ok(!JSON.stringify(report).includes(secret))
      }
    )
  it('should not mistake an existing file for a writable cache directory', async () => {
    const report = await collectBrowserEnvironment(
      operations({ stat: async () => ({ isDirectory: () => false }) })
    )
    assert.deepEqual(report.cache.fontconfigSystem, {
      directory: false,
      readable: null,
      writable: null
    })
  })
  it('should keep access races and unexpected access I/O unknown', async () => {
    for (const code of ['ENOENT', 'EIO']) {
      const report = await collectBrowserEnvironment(
        operations({
          access: async () => {
            throw error(code)
          }
        })
      )
      assert.deepEqual(report.cache.fontconfigSystem, {
        directory: true,
        readable: null,
        writable: null
      })
    }
  })
  it('should not guess a home directory or effective user', async () => {
    const report = await collectBrowserEnvironment(
      operations({
        home: () => {
          throw error('EIO')
        },
        uid: () => secret,
        euid: () => {
          throw error('EPERM')
        },
        env: {}
      })
    )
    assert.deepEqual(report.user, { uid: null, euid: null })
    assert.deepEqual(report.cache.fontconfigLegacy, {
      directory: null,
      readable: null,
      writable: null
    })
  })
  it('should not resolve relative XDG cache configuration against a guessed directory', async () => {
    const report = await collectBrowserEnvironment(operations({ env: { XDG_CACHE_HOME: secret } }))
    assert.deepEqual(report.cache.fontconfigUser, {
      directory: null,
      readable: null,
      writable: null
    })
    assert.ok(!JSON.stringify(report).includes(secret))
  })
})

describe('when pressure observations are absent or malformed', () => {
  for (const input of [
    '',
    'garbage ' + secret,
    'full avg10=1.00 avg60=0.00 avg300=0.00 total=1',
    pressure + pressure,
    pressure.replace('avg10=0.00', 'avg10=NaN'),
    pressure.replace('avg10=0.00', 'avg10=101.00'),
    null
  ])
    it(
      'should preserve unknown rather than claim no pressure for malformed input length ' +
        String(input).length,
      async () => {
        const report = await collectBrowserEnvironment(operations({ readFile: async () => input }))
        assert.deepEqual(report.pressure, {
          cpuSome10s: null,
          memorySome10s: null,
          ioSome10s: null
        })
        assert.ok(!JSON.stringify(report).includes(secret))
      }
    )
  for (const code of ['ENOENT', 'EACCES', 'EIO'])
    it('should preserve unknown after pressure read failure ' + code, async () => {
      const report = await collectBrowserEnvironment(
        operations({
          readFile: async () => {
            throw error(code)
          }
        })
      )
      assert.deepEqual(report.pressure, { cpuSome10s: null, memorySome10s: null, ioSome10s: null })
    })
})

describe('when a tooling-only change selects the UI gate', () => {
  it('should select both diagnostic inputs and execute their tests before the actual UI test gate', async () => {
    const workflow = await readFile(
      new URL('../../.github/workflows/ui-library.yml', import.meta.url),
      'utf8'
    )
    const selection = workflow.split('\n').find((line) => line.includes('files=$(git log'))
    assert.ok(selection.includes('tooling/ui-browser-environment.mjs'))
    assert.ok(selection.includes('tooling/tests/ui-browser-environment.test.mjs'))
    const unit = workflow.indexOf(
      'run: node --test ../../tooling/tests/ui-browser-environment.test.mjs'
    )
    const probe = workflow.indexOf('run: node ../../tooling/ui-browser-environment.mjs')
    const actual = workflow.indexOf('run: pnpm test')
    assert.ok(unit >= 0 && probe > unit && actual > probe)
  })
})

describe('when the trusted artifact scan classifies a tooling-only change', () => {
  it('should require the same artifact paths that select the unprivileged UI build', async () => {
    const ui = await readFile(
      new URL('../../.github/workflows/ui-library.yml', import.meta.url),
      'utf8'
    )
    const scan = await readFile(
      new URL('../../.github/workflows/ui-library-key-scan.yml', import.meta.url),
      'utf8'
    )
    const paths = (text) =>
      [...text.matchAll(/--diff-merges=cc "\$range" -- ([^)\n]+)\)/g)].map((match) =>
        match[1].trim().split(/\s+/)
      )
    assert.equal(paths(ui).length, 1)
    assert.deepEqual(paths(scan), paths(ui))
  })
})

describe('when the actual version child cannot finish cooperatively', () => {
  function probe(body) {
    const scratch = mkdtempSync(path.join(os.tmpdir(), 'ui-version-deadline-'))
    const pidFile = path.join(scratch, 'owned.pid')
    const descendantFile = path.join(scratch, 'descendant.pid')
    const executable = path.join(scratch, 'browser')
    writeFileSync(executable, `#!${process.execPath}\n${body({ pidFile, descendantFile })}`)
    chmodSync(executable, 0o700)
    const script = path.join(scratch, 'probe.mjs')
    writeFileSync(
      script,
      `import {collectBrowserEnvironment} from ${JSON.stringify(new URL('../ui-browser-environment.mjs', import.meta.url).href)}; const report=await collectBrowserEnvironment({findBrowser:async()=>${JSON.stringify(executable)},env:{},home:()=>null,stat:async()=>{throw Object.assign(new Error(),{code:'ENOENT'})},readFile:async()=>{throw new Error()}});console.log(JSON.stringify(report.browser))`
    )
    const started = performance.now()
    try {
      const stdout = execFileSync(process.execPath, [script], {
        timeout: 4500,
        killSignal: 'SIGKILL',
        encoding: 'utf8',
        maxBuffer: 8192
      })
      return { browser: JSON.parse(stdout), elapsed: performance.now() - started }
    } finally {
      for (const file of [pidFile, descendantFile]) {
        try {
          process.kill(Number(readFileSync(file, 'utf8')), 'SIGKILL')
        } catch {
          /* The recorded fixture child may already have exited. */
        }
      }
      rmSync(scratch, { recursive: true, force: true })
    }
  }

  it('should bound a real version child that ignores SIGTERM without printing its private output', () => {
    const result = probe(
      ({ pidFile }) =>
        `const fs=require('node:fs');fs.writeFileSync(${JSON.stringify(pidFile)},String(process.pid));process.on('SIGTERM',()=>{});process.stdout.write(${JSON.stringify(secret)});setInterval(()=>{},1000)`
    )
    assert.deepEqual(result.browser, { selected: true, family: null, version: null })
    assert.ok(result.elapsed >= 2500 && result.elapsed < 4200)
  })

  it('should finish and release the caller when an orphan keeps inherited stdout open', () => {
    const result = probe(
      ({ descendantFile }) =>
        `const {spawn}=require('node:child_process');const fs=require('node:fs');const child=spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:['ignore',1,2]});fs.writeFileSync(${JSON.stringify(descendantFile)},String(child.pid));process.stdout.write('Google Chrome 157.0.8103.50\\n');process.exit(0)`
    )
    assert.deepEqual(result.browser, { selected: true, family: null, version: null })
    assert.ok(result.elapsed >= 2500 && result.elapsed < 4200)
  })

  it('should accept actual complete version output only after successful exit', () => {
    const result = probe(() => `process.stdout.write('Google Chrome 157.0.8103.50\\n')`)
    assert.deepEqual(result.browser, {
      selected: true,
      family: 'Google Chrome',
      version: '157.0.8103.50'
    })
  })

  it('should reject excessive actual output without exposing it', () => {
    const result = probe(() => `process.stdout.write(${JSON.stringify(secret)}.repeat(1000))`)
    assert.deepEqual(result.browser, { selected: true, family: null, version: null })
    assert.ok(result.elapsed < 2500)
  })

  it('should discard otherwise valid output after a nonzero actual child exit', () => {
    const result = probe(
      () => `process.stdout.write('Google Chrome 157.0.8103.50\\n');process.exitCode=1`
    )
    assert.deepEqual(result.browser, { selected: true, family: null, version: null })
  })

  it('should keep actual executable selection failure metadata unknown', async () => {
    const overrides = operations({
      findBrowser: async () => '/nonexistent/ui-version-probe',
      version: undefined
    })
    delete overrides.version
    const result = await collectBrowserEnvironment(overrides)
    assert.deepEqual(result.browser, { selected: true, family: null, version: null })
  })
})
