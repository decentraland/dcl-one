import assert from 'node:assert/strict'
import path from 'node:path'
import { describe, test } from 'node:test'
import { environment } from '../scripts/build-scenes.ts'

describe('build-scenes environment', () => {
  test('a CLI on the PATH leaves the environment as it is', () => {
    const env = { PATH: '/usr/bin', HOME: '/home/x' }
    assert.equal(environment('dcl-scene-cli', env), env)
  })

  test("a CLI named by path puts its folder first on the PATH, under the variable's own spelling", () => {
    const bin = path.join(path.sep, 'ci', 'scene-cli', 'dcl-scene-cli.exe')
    const env = environment(bin, { Path: 'C:\\Windows', HOME: '/home/x' })
    assert.equal(env.Path, [path.resolve(path.join(path.sep, 'ci', 'scene-cli')), 'C:\\Windows'].join(path.delimiter))
    assert.equal(env.HOME, '/home/x')
    assert.equal('PATH' in env, false)
    assert.equal(environment(bin, {}).PATH, path.resolve(path.join(path.sep, 'ci', 'scene-cli')))
  })
})
