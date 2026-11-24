import assert from 'node:assert/strict'
import path from 'node:path'
import { describe, test } from 'node:test'
import { FakeConsole } from '../driver/console.ts'
import { Explorer } from '../driver/explorer.ts'
import { REPORT_DIR } from '../driver/paths.ts'
import { baseParcel, closeOnSignal, commandLine, parseArgs, type Result, runLines, SIGNAL_EXIT, startFolder } from '../scripts/drive.ts'

async function* from(lines: string[]): AsyncIterable<string> {
  yield* lines
}

describe('drive arguments', () => {
  test('takes every scene, resolved from the working folder, and the options', () => {
    const cwd = path.join(path.sep, 'work')
    const options = parseArgs(['--scene', 'a', '--scene', '/abs/b', '--position', '149,149', '--report', 'out', '--web-app', 'web'], cwd)
    assert.deepEqual(options, {
      scenes: [path.join(cwd, 'a'), path.resolve('/abs/b')],
      position: '149,149',
      report: path.join(cwd, 'out'),
      webApp: path.join(cwd, 'web')
    })
  })

  test('reports into report/drive by default and needs no position', () => {
    const options = parseArgs(['--scene', 'a'])
    assert.equal(options.report, path.join(REPORT_DIR, 'drive'))
    assert.equal(options.position, undefined)
  })

  test('skips the separator a package manager hands on', () => {
    assert.deepEqual(parseArgs(['--', '--scene', 'a'], path.sep).scenes, [path.join(path.sep, 'a')])
  })

  test('refuses a run with no scene, a bad position, a missing value and an unknown flag', () => {
    assert.throws(() => parseArgs([]), /--scene <dir> is needed/)
    assert.throws(() => parseArgs(['--scene', 'a', '--position', '149']), /a position is "x,y"/)
    assert.throws(() => parseArgs(['--scene']), /--scene needs a value/)
    assert.throws(() => parseArgs(['--scene', '--position', '1,1']), /--scene needs a value/)
    assert.throws(() => parseArgs(['--scene', 'a', '--headless']), /unknown argument --headless/)
  })
})

describe('drive stdin protocol', () => {
  test('skips blank and # lines and trims the rest', () => {
    assert.equal(commandLine('   '), undefined)
    assert.equal(commandLine('# a note'), undefined)
    assert.equal(commandLine('  # indented note'), undefined)
    assert.equal(commandLine('  /world_state \r'), '/world_state')
  })

  test('runs each command in turn and keeps going after a failure', async () => {
    const console_ = new FakeConsole({ world_state: '{"inWorld":true}', scene_logs: (line) => `logs for ${line}` })
    const seen: Result[] = []
    const results = await runLines(from(['# setup', '/world_state', '', '/nope', '/scene_logs 20']), new Explorer(console_), (result) => seen.push(result))
    assert.deepEqual(console_.lines, ['/world_state', '/nope', '/scene_logs 20'])
    assert.deepEqual(
      results.map(({ line, ok }) => ({ line, ok })),
      [
        { line: '/world_state', ok: true },
        { line: '/nope', ok: false },
        { line: '/scene_logs 20', ok: true }
      ]
    )
    assert.equal(results[0]!.reply, '{"inWorld":true}')
    assert.match(results[1]!.reply, /^Command not recognized: `\/nope`/)
    assert.equal(results[2]!.reply, 'logs for /scene_logs 20')
    assert.ok(results.every((result) => Number.isInteger(result.ms) && result.ms >= 0))
    assert.deepEqual(seen, results)
  })

  test('reports a transport error as a failed result', async () => {
    const explorer = {
      run: async () => {
        throw new Error('the page closed')
      }
    }
    const results = await runLines(from(['/world_state']), explorer, () => {})
    assert.deepEqual(results.map(({ ok, reply }) => ({ ok, reply })), [{ ok: false, reply: 'the page closed' }])
  })
})

describe('drive arrival', () => {
  const scene = (metadata: Record<string, unknown>) => ({ entity: { id: 'id', type: 'scene' as const, pointers: ['3,4', '2,4'], content: [], metadata } })

  test('arrives at the base parcel, else at the first parcel', () => {
    assert.equal(baseParcel(scene({ scene: { base: '2,4', parcels: ['3,4', '2,4'] } })), '2,4')
    assert.equal(baseParcel(scene({ scene: { parcels: ['3,4', '2,4'] } })), '3,4')
    assert.equal(baseParcel(scene({})), '3,4')
  })
})

describe('drive interruption', () => {
  test('closes once on a signal and exits with its code', async () => {
    const before = { SIGINT: process.listenerCount('SIGINT'), SIGTERM: process.listenerCount('SIGTERM') }
    let closed = 0
    const exits: number[] = []
    let finish!: () => void
    const exited = new Promise<void>((resolve) => (finish = resolve))
    const ending = closeOnSignal(
      async () => {
        closed++
      },
      (code) => {
        exits.push(code)
        finish()
      }
    )
    try {
      assert.equal(ending.ended, false)
      process.emit('SIGTERM')
      assert.equal(ending.ended, true)
      await exited
      assert.equal(closed, 1)
      assert.deepEqual(exits, [SIGNAL_EXIT.SIGTERM])
      // a second signal while closing exits at once and closes nothing again
      process.emit('SIGINT')
      assert.equal(closed, 1)
      assert.deepEqual(exits, [SIGNAL_EXIT.SIGTERM, SIGNAL_EXIT.SIGINT])
    } finally {
      ending.release()
    }
    assert.deepEqual({ SIGINT: process.listenerCount('SIGINT'), SIGTERM: process.listenerCount('SIGTERM') }, before)
  })

  test('ends on request with the code given, and still exits when the close fails', async () => {
    const exits: number[] = []
    let finish!: () => void
    const exited = new Promise<void>((resolve) => (finish = resolve))
    const ending = closeOnSignal(
      async () => {
        throw new Error('the browser is gone')
      },
      (code) => {
        exits.push(code)
        finish()
      }
    )
    try {
      ending.end(1, 'stdout closed')
      await exited
      assert.equal(ending.ended, true)
      assert.deepEqual(exits, [1])
    } finally {
      ending.release()
    }
  })
})

describe('drive credentials', () => {
  const identity = 'eyJwcml2YXRlS2V5IjoiMHhzZWNyZXQifQ=='

  test('redacts a sign-in payload in the line and in the reply, passed or failed', async () => {
    const console_ = new FakeConsole({ login_identity: (line) => `signed in from ${line}` })
    const passed = await runLines(from([`/login_identity ${identity} --guest`]), new Explorer(console_), () => {})
    assert.deepEqual(passed.map(({ line, ok, reply }) => ({ line, ok, reply })), [
      { line: '/login_identity <redacted> --guest', ok: true, reply: 'signed in from /login_identity <redacted> --guest' }
    ])
    const failing = {
      run: async (line: string) => {
        throw new Error(`could not send ${line}`)
      }
    }
    const failed = await runLines(from([`/hud_login ${identity}`]), failing, () => {})
    assert.deepEqual(failed.map(({ line, ok, reply }) => ({ line, ok, reply })), [
      { line: '/hud_login <redacted>', ok: false, reply: 'could not send /hud_login <redacted>' }
    ])
  })
})

describe('drive start folder', () => {
  test('is where the package manager was started, else the working folder', () => {
    assert.equal(startFolder({ INIT_CWD: '/work/scenes' }), '/work/scenes')
    assert.equal(startFolder({}), process.cwd())
  })
})
