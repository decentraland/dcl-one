import { spawnSync } from 'node:child_process'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'

// A child process isolates an unhandled server rejection from the test runner.
const inspectRequest = [
  'const { checkBuild } = await import(process.argv[1]);',
  'try {',
  '  await checkBuild({',
  '    dist: process.argv[2], prefix: "/gallery/", browser: "unused",',
  '    open: async () => ({',
  '      navigate: async (url) => {',
  '        const response = await fetch(new URL(process.argv[3], url));',
  '        console.log(JSON.stringify({ status: response.status, body: await response.text() }));',
  '        throw new Error("request inspected");',
  '      },',
  '      close: async () => {}',
  '    })',
  '  });',
  '} catch (error) {',
  '  if (error.message !== "request inspected") throw error;',
  '}'
].join('\n')

const CHILD_TIMEOUT_MS = 10_000

describe('when the built gallery server receives a file request', () => {
  let scratch: string
  let dist: string

  beforeEach(async () => {
    scratch = await mkdtemp(path.join(tmpdir(), 'check-build-paths-'))
    dist = path.join(scratch, 'review-dist')
    await mkdir(path.join(dist, 'nested'), { recursive: true })
    await writeFile(path.join(dist, 'index.html'), 'gallery entry')
    await writeFile(path.join(dist, 'nested/file.txt'), 'nested asset')
    await mkdir(path.join(scratch, 'review-dist2'))
    await writeFile(path.join(scratch, 'review-dist2/secret.txt'), 'outside dist')
  })
  afterEach(async () => {
    await rm(scratch, { recursive: true, force: true })
  })

  describe.each([
    { route: '/gallery/%E0', status: 400, body: '' },
    { route: '/gallery/%', status: 400, body: '' },
    { route: '/gallery/%ZZ', status: 400, body: '' },
    { route: '/gallery/..%2freview-dist2%2fsecret.txt', status: 403, body: '' },
    { route: '/gallery/..%2fsecret.txt', status: 403, body: '' },
    { route: '/other/index.html', status: 404, body: '' },
    { route: '/gallery/', status: 200, body: 'gallery entry' },
    { route: '/gallery/nested/file.txt', status: 200, body: 'nested asset' }
  ])('and the path is $route', ({ route, status, body }) => {
    it(
      'should return the expected response without crashing or serving outside the build',
      () => {
        const result = spawnSync(
          process.execPath,
          [
            '--input-type=module',
            '-e',
            inspectRequest,
            new URL('../figma/review/scripts/check-build.mjs', import.meta.url).href,
            dist,
            route
          ],
          { encoding: 'utf8', timeout: CHILD_TIMEOUT_MS }
        )
        // A child killed by the timeout reports status null; name the cause instead.
        expect(result.error, `child did not finish within ${CHILD_TIMEOUT_MS} ms`).toBeUndefined()
        expect(result).toMatchObject({
          status: 0,
          stdout: JSON.stringify({ status, body }) + '\n',
          stderr: ''
        })
      },
      CHILD_TIMEOUT_MS * 2
    )
  })
})
