import assert from 'node:assert/strict'
import path from 'node:path'
import { describe, test } from 'node:test'
import { FakeConsole } from '../driver/console.ts'
import { Explorer } from '../driver/explorer.ts'
import { REPORT_DIR } from '../driver/paths.ts'
import { commandLine, parseArgs, type Result, runLines } from '../scripts/drive.ts'

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
