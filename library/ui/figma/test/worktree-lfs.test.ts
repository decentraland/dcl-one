import { execFile, execFileSync } from 'node:child_process'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { createServer, type Server } from 'node:http'
import type { AddressInfo } from 'node:net'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { parse } from 'yaml'

const repo = path.resolve(import.meta.dirname, '../../../..')
const workflow = parse(
  await readFile(path.join(repo, '.github/workflows/ui-library.yml'), 'utf8')
) as { jobs: { library: { steps: { name?: string; run?: string }[] } } }
const step = workflow.jobs.library.steps.find(
  (item) => item.name === 'Check retirements and removals against the base'
)!.run!
let root: string
let server: Server
let requests: string[]
const pointer =
  'version https://git-lfs.github.com/spec/v1\noid sha256:' + 'f'.repeat(64) + '\nsize 1\n'
const git = (...args: string[]) =>
  execFileSync('git', args, { cwd: root, stdio: 'pipe' }).toString().trim()
const commit = () => {
  git('add', '-A')
  git('-c', 'commit.gpgsign=false', 'commit', '-qm', 'fixture')
}
beforeEach(async () => {
  root = await mkdtemp(path.join(os.tmpdir(), 'ui-base-lfs-'))
  requests = []
  server = createServer((request, response) => {
    requests.push(request.url ?? '')
    request.resume()
    response
      .writeHead(404, { 'content-type': 'application/json' })
      .end('{"message":"no remote assets in this test"}')
  })
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
  git('init', '-q', '-b', 'main')
  git('config', 'user.name', 'Fixture Dev')
  git('config', 'user.email', 'fixture@example.com')
  git('lfs', 'install', '--local')
  git('config', 'lfs.url', `http://127.0.0.1:${(server.address() as AddressInfo).port}/lfs`)
  await writeFile(
    path.join(root, '.gitattributes'),
    'outside.bin filter=lfs diff=lfs merge=lfs -text\n'
  )
  await writeFile(path.join(root, 'outside.bin'), pointer)
  await mkdir(path.join(root, 'library/ui'), { recursive: true })
  await writeFile(path.join(root, 'library/ui/README.md'), 'base\n')
  commit()
  await writeFile(path.join(root, 'library/ui/README.md'), 'head\n')
  commit()
  await mkdir(path.join(root, 'runner-temp'))
})
afterEach(async () => {
  server?.closeAllConnections()
  if (server?.listening) await new Promise<void>((resolve) => server.close(() => resolve()))
  if (root) await rm(root, { recursive: true, force: true })
})
async function runStep(source: string) {
  const env: NodeJS.ProcessEnv = { ...process.env, RUNNER_TEMP: path.join(root, 'runner-temp') }
  delete env.GIT_LFS_SKIP_SMUDGE
  return new Promise<{ code: number; stdout: string; stderr: string }>((resolve) => {
    execFile(
      'bash',
      ['-e', '-o', 'pipefail', '-c', source],
      {
        cwd: path.join(root, 'library/ui'),
        env,
        timeout: 10_000
      },
      (error, stdout, stderr) => resolve({ code: error ? 1 : 0, stdout, stderr })
    )
  })
}

describe('when the retirement guard creates its base checkout', () => {
  it('should not smudge or download LFS files outside UI', async () => {
    const result = await runStep(step)
    expect(result.code, result.stderr).toBe(0)
    expect(requests).toEqual([])
    expect(result.stdout).toContain('first introduction of library/ui')
    expect(await readFile(path.join(root, 'runner-temp/figma-base/outside.bin'), 'utf8')).toBe(
      pointer
    )
  }, 15_000)

  it('should reproduce the download attempt without the skip-smudge environment (control)', async () => {
    const result = await runStep(step.replace('GIT_LFS_SKIP_SMUDGE=1 ', ''))
    expect(result.code).not.toBe(0)
    expect(requests.some((url) => url.endsWith('/objects/batch'))).toBe(true)
  }, 15_000)
})
