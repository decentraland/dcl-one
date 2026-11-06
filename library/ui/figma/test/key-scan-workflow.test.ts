import { execFileSync, spawnSync } from 'node:child_process'
import { mkdtemp, readFile, rm, writeFile, mkdir } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { parse } from 'yaml'

const repo = path.resolve(import.meta.dirname, '../../../..')
const head = '1'.repeat(40),
  base = '2'.repeat(40)
type Step = { id?: string; run?: string; name?: string }
const steps = (
  parse(await readFile(path.join(repo, '.github/workflows/ui-library-key-scan.yml'), 'utf8')) as {
    jobs: { 'key-scan': { steps: Step[] } }
  }
).jobs['key-scan'].steps
let root: string
beforeEach(async () => {
  root = await mkdtemp(path.join(os.tmpdir(), 'key-workflow-'))
})
afterEach(async () => {
  await rm(root, { recursive: true, force: true })
})
const defaults = () => ({
  ...process.env,
  REPOSITORY: 'example/repo',
  HEAD_REPO: 'example/repo',
  HEAD_SHA: head,
  WORKFLOW_PATH: '.github/workflows/ui-library.yml',
  WORKFLOW_ID: '42',
  RUN_ID: '123',
  EVENT: 'pull_request',
  CONCLUSION: 'success',
  HEAD_BRANCH: 'feature',
  DEFAULT_BRANCH: 'main',
  PULL_REQUESTS: JSON.stringify([{ base: { sha: base }, head: { sha: head } }]),
  GITHUB_OUTPUT: path.join(root, 'out'),
  RUNNER_TEMP: root
})
async function validate(overrides: Record<string, string | undefined> = {}) {
  await writeFile(path.join(root, 'out'), '')
  const script =
    `gh() { case "$*" in *actions/workflows/ui-library.yml*) echo 42;; */pulls*) [ "$LOOKUP_FAIL" != true ] || return 7; printf '%s' "$LOOKUP_RESPONSE";; *) return 8;; esac; }\n` +
    steps[0]!.run!
  const result = spawnSync('bash', ['-e', '-o', 'pipefail', '-c', script], {
    cwd: root,
    encoding: 'utf8',
    env: { ...defaults(), ...overrides }
  })
  const output = Object.fromEntries(
    (await readFile(path.join(root, 'out'), 'utf8'))
      .trim()
      .split('\n')
      .map((line) => line.split(/=(.*)/s).slice(0, 2))
  )
  return { ...result, output }
}
async function report(output: Record<string, string>, extra: Record<string, string> = {}) {
  const script =
    `gh() { printf '%s\\n' "$@" > "$RUNNER_TEMP/report"; }\n` +
    steps.find((step) => step.name === 'Report the result on the scanned commit')!.run!
  const result = spawnSync('bash', ['-e', '-o', 'pipefail', '-c', script], {
    cwd: root,
    encoding: 'utf8',
    env: {
      ...defaults(),
      OUTCOME: 'skipped',
      SKIP_REASON: output.reason ?? '',
      SKIP_CONCLUSION: output.conclusion ?? '',
      ...extra
    }
  })
  expect(result.status, result.stderr).toBe(0)
  return readFile(path.join(root, 'report'), 'utf8')
}
describe('when a trusted scan cannot run', () => {
  it('should report a neutral check on fork heads without exposing the private key', async () => {
    const { output, status } = await validate({ HEAD_REPO: 'outside/fork' })
    expect(status).toBe(0)
    expect(output.scan).toBe('false')
    expect(await report(output)).toContain('conclusion=neutral')
    expect(await report(output)).toContain(`head_sha=${head}`)
  })
  it('should resolve an empty workflow_run PR list from the repository API', async () => {
    const { output, status } = await validate({
      PULL_REQUESTS: '[]',
      LOOKUP_RESPONSE: JSON.stringify([{ base: { sha: base }, head: { sha: head } }])
    })
    expect(status).toBe(0)
    expect(output).toMatchObject({ scan: 'true', base, head })
  })
  it.each([{ LOOKUP_FAIL: 'true' }, { LOOKUP_RESPONSE: '[]' }])(
    'should report failure when PR association is unavailable: %j',
    async (extra) => {
      const { output } = await validate({ PULL_REQUESTS: '[]', ...extra })
      expect(output.scan).toBe('false')
      expect(await report(output)).toContain('conclusion=failure')
      expect(await report(output)).toContain('Not scanned:')
    }
  )
  it('should post a failure when artifact enumeration fails rather than leave an Expected check', async () => {
    const step = steps.find((step) => step.id === 'artifact_check')!
    const result = spawnSync(
      'bash',
      ['-e', '-o', 'pipefail', '-c', 'gh() { return 7; }\n' + step.run],
      { cwd: root, encoding: 'utf8', env: defaults() }
    )
    expect(result.status).not.toBe(0)
    expect(await report({}, { ARTIFACT_OUTCOME: 'failure' })).toContain(
      'required Review artifact was missing'
    )
  })
  it('should reject an absent or duplicate Review artifact', async () => {
    const step = steps.find((step) => step.id === 'artifact_check')!
    for (const count of [0, 2]) {
      const result = spawnSync(
        'bash',
        ['-e', '-o', 'pipefail', '-c', `gh() { echo ${count}; }\n` + step.run],
        { cwd: root, encoding: 'utf8', env: defaults() }
      )
      expect(result.status).not.toBe(0)
    }
  })
})
describe('when selecting historical UI LFS inputs', () => {
  it('should prefetch intermediate UI commits even if the pushed tip removes their file', async () => {
    const git = (...args: string[]) =>
      execFileSync('git', args, { cwd: path.join(root, 'trusted'), stdio: 'pipe' })
        .toString()
        .trim()
    await mkdir(path.join(root, 'trusted'))
    git('init', '-q', '-b', 'main')
    const commit = () => {
      git('add', '-A')
      git(
        '-c',
        'user.name=Test',
        '-c',
        'user.email=test@example.com',
        '-c',
        'commit.gpgsign=false',
        'commit',
        '-qm',
        'fixture'
      )
    }
    await writeFile(path.join(root, 'trusted/README'), 'clean')
    commit()
    const before = git('rev-parse', 'HEAD')
    await mkdir(path.join(root, 'trusted/library/ui'), { recursive: true })
    await writeFile(path.join(root, 'trusted/library/ui/transient'), 'synthetic input')
    commit()
    const intermediate = git('rev-parse', 'HEAD')
    await rm(path.join(root, 'trusted/library/ui/transient'))
    commit()
    const after = git('rev-parse', 'HEAD')
    const script =
      `git() { case "$*" in *' fetch '*) printf '%s\\n' "$*" >> "$RUNNER_TEMP/fetches";; *) command git "$@";; esac; }\n` +
      steps.find((step) => step.id === 'inputs')!.run!
    const result = spawnSync('bash', ['-e', '-o', 'pipefail', '-c', script], {
      cwd: root,
      encoding: 'utf8',
      env: { ...defaults(), BASE: before, HEAD_SHA: after, READ_TOKEN: 'synthetic' }
    })
    expect(result.status, result.stderr).toBe(0)
    expect(await readFile(path.join(root, 'out'), 'utf8')).toContain('artifact_required=true')
    expect(await readFile(path.join(root, 'fetches'), 'utf8')).toContain(
      `lfs fetch --include library/ui/** origin ${after}`
    )
    expect(await readFile(path.join(root, 'fetches'), 'utf8')).toContain(intermediate)
  })
})
