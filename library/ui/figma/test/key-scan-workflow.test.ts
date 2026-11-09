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
      ...extra
    }
  })
  expect(result.status, result.stderr).toBe(0)
  return readFile(path.join(root, 'report'), 'utf8')
}
describe('when a trusted scan cannot run', () => {
  it('should report a failing check on fork heads without exposing the private key', async () => {
    const { output, status } = await validate({ HEAD_REPO: 'outside/fork' })
    expect(status).toBe(0)
    expect(output.scan).toBe('false')
    expect(await report(output)).toContain('conclusion=failure')
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
  // gh prints one artifact id per line for the matching artifacts; the fake stands in for that output. Actions
  // runs the step as bash -e, so pipefail comes from the step itself.
  const countArtifacts = (ids: string[], status = 0) => {
    const step = steps.find((step) => step.id === 'artifact_check')!
    const gh = `gh() { printf '%s' '${ids.map((id) => `${id}\n`).join('')}'; return ${status}; }\n`
    return spawnSync('bash', ['-e', '-c', gh + step.run], {
      cwd: root,
      encoding: 'utf8',
      env: defaults()
    })
  }
  it('should reject an absent or duplicate Review artifact', async () => {
    for (const ids of [[], ['1', '2']]) expect(countArtifacts(ids).status).not.toBe(0)
  })
  it('should accept exactly one Review artifact', async () => {
    expect(countArtifacts(['7']).status).toBe(0)
  })
  it('should fail when the artifact listing itself fails', async () => {
    expect(countArtifacts(['7'], 1).status).not.toBe(0)
  })
  // The step's own select filter, run by the real jq over one artifacts page that gh would page through.
  const selectArtifacts = async (artifacts: unknown[]) => {
    await writeFile(path.join(root, 'page.json'), JSON.stringify({ artifacts }))
    const gh =
      `gh() { local filter=; while [ $# -gt 0 ]; do [ "$1" != --jq ] || filter=$2; shift; done; ` +
      `jq -r "$filter" "$RUNNER_TEMP/page.json" | tee -a "$RUNNER_TEMP/ids"; }\n`
    const step = steps.find((step) => step.id === 'artifact_check')!
    const result = spawnSync('bash', ['-e', '-c', gh + step.run], {
      cwd: root,
      encoding: 'utf8',
      env: defaults()
    })
    const ids = await readFile(path.join(root, 'ids'), 'utf8').catch(() => '')
    return { status: result.status, stderr: result.stderr, ids: ids.split('\n').filter(Boolean) }
  }
  const artifact = (id: number, change: Record<string, unknown> = {}) => ({
    id,
    name: `ui-review-${head}`,
    expired: false,
    workflow_run: { id: 123, head_sha: head },
    ...change
  })
  it('should select only the unexpired Review artifact of the run and head', async () => {
    const { status, stderr, ids } = await selectArtifacts([
      artifact(7),
      artifact(8, { expired: true }),
      artifact(9, { workflow_run: { id: 123, head_sha: base } }),
      artifact(10, { name: `ui-review-${base}` }),
      artifact(11, { workflow_run: { id: 124, head_sha: head } })
    ])
    expect(ids).toEqual(['7'])
    expect(status, stderr).toBe(0)
  })
  it('should reject a page whose only artifacts fail the filter', async () => {
    const { status, ids } = await selectArtifacts([
      artifact(8, { expired: true }),
      artifact(10, { name: 'ui-review-other' })
    ])
    expect(ids).toEqual([])
    expect(status).not.toBe(0)
  })
})
describe('when selecting the LFS inputs of the scan', () => {
  const git = (...args: string[]) =>
    execFileSync('git', args, { cwd: path.join(root, 'trusted'), stdio: 'pipe' })
      .toString()
      .trim()
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
  // Runs the inputs step with git fetch and node recorded instead of executed.
  async function inputs(env: Record<string, string>) {
    const script =
      `git() { case "$*" in *' fetch '*) printf '%s\\n' "$*" >> "$RUNNER_TEMP/fetches";; *) command git "$@";; esac; }\n` +
      `node() { printf '%s\\n' "$*" >> "$RUNNER_TEMP/node"; }\n` +
      steps.find((step) => step.id === 'inputs')!.run!
    const result = spawnSync('bash', ['-e', '-o', 'pipefail', '-c', script], {
      cwd: root,
      encoding: 'utf8',
      env: { ...defaults(), READ_TOKEN: 'synthetic', ...env }
    })
    return { result, node: await readFile(path.join(root, 'node'), 'utf8') }
  }
  beforeEach(async () => {
    await mkdir(path.join(root, 'trusted'))
    git('init', '-q', '-b', 'main')
    await writeFile(path.join(root, 'trusted/README'), 'clean')
    commit()
  })

  it('should list the whole range, any path, and the library/ui tree when UI changed, then prefetch it', async () => {
    const before = git('rev-parse', 'HEAD')
    await mkdir(path.join(root, 'trusted/library/ui'), { recursive: true })
    await writeFile(path.join(root, 'trusted/library/ui/transient'), 'synthetic input')
    commit()
    await rm(path.join(root, 'trusted/library/ui/transient'))
    commit()
    const after = git('rev-parse', 'HEAD')

    const { result, node } = await inputs({ BASE: before, HEAD_SHA: after })
    expect(result.status, result.stderr).toBe(0)
    expect(await readFile(path.join(root, 'out'), 'utf8')).toContain('artifact_required=true')
    const [list, prefetch] = node.trim().split('\n')
    expect(list).toBe(
      `trusted/library/ui/figma/key-guard.mjs --root trusted --no-worktree --range ${before}..${after} --tree ${after}:library/ui --lfs-objects`
    )
    expect(prefetch).toBe(
      `trusted/library/ui/figma/lfs-prefetch.mjs --root trusted --endpoint https://github.com/example/repo.git/info/lfs --objects ${root}/lfs-objects.jsonl`
    )
    expect(await readFile(path.join(root, 'fetches'), 'utf8')).not.toContain('lfs')
  })

  it('should still list and prefetch the range of a push that changes nothing under library/ui', async () => {
    const before = git('rev-parse', 'HEAD')
    await mkdir(path.join(root, 'trusted/apps/sites/public'), { recursive: true })
    await writeFile(path.join(root, 'trusted/apps/sites/public/clean.bin'), 'pointer stand-in')
    commit()
    const after = git('rev-parse', 'HEAD')

    const { result, node } = await inputs({ BASE: before, HEAD_SHA: after })
    expect(result.status, result.stderr).toBe(0)
    expect(await readFile(path.join(root, 'out'), 'utf8')).toContain('artifact_required=false')
    const [list, prefetch] = node.trim().split('\n')
    expect(list).toBe(
      `trusted/library/ui/figma/key-guard.mjs --root trusted --no-worktree --range ${before}..${after} --lfs-objects`
    )
    expect(prefetch).toContain('lfs-prefetch.mjs')
  })
})
describe('when the privileged scan reads the url allowlist', () => {
  const url = 'https://www.figma' + '.com/design/AllowlistExample123/fixture'
  const entry = JSON.stringify([
    { path: 'docs/link.md', check: 'url', id: 'AllowlistExample123', reason: 'fixture' }
  ])
  const git = (...args: string[]) =>
    execFileSync('git', args, { cwd: path.join(root, 'trusted'), stdio: 'pipe' })
      .toString()
      .trim()
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
    return git('rev-parse', 'HEAD')
  }
  const allowlistFile = () => path.join(root, 'trusted/library/ui/figma/key-guard-allow.json')
  // Runs the real scan step with main's real key guard and a synthetic key.
  const scan = (env: Record<string, string>) =>
    spawnSync(
      'bash',
      ['-e', '-o', 'pipefail', '-c', steps.find((step) => step.id === 'scan')!.run!],
      {
        cwd: root,
        encoding: 'utf8',
        env: {
          ...defaults(),
          GIT_CONFIG_NOSYSTEM: '1',
          GIT_CONFIG_GLOBAL: '/dev/null',
          FIGMA_FILE_KEY: 'SyntheticKeyExample0',
          ARTIFACT_REQUIRED: 'false',
          ...env
        }
      }
    )
  let mainBase: string, headSha: string
  beforeEach(async () => {
    await mkdir(path.join(root, 'trusted/library/ui/figma'), { recursive: true })
    git('init', '-q', '-b', 'main')
    await writeFile(
      path.join(root, 'trusted/library/ui/figma/key-guard.mjs'),
      await readFile(path.join(repo, 'library/ui/figma/key-guard.mjs'))
    )
    await writeFile(allowlistFile(), '[]')
    mainBase = commit()
    git('checkout', '-q', '-b', 'feature')
    await mkdir(path.join(root, 'trusted/docs'))
    await writeFile(path.join(root, 'trusted/docs/link.md'), `See ${url}\n`)
    await writeFile(allowlistFile(), entry)
    headSha = commit()
    git('checkout', '-q', 'main')
  })

  it('should take the allowlist from the default branch, so a head-only entry does not suppress the url', () => {
    const result = scan({ BASE: mainBase, HEAD_SHA: headSha })
    expect(result.status).not.toBe(0)
    expect(result.stdout).toContain(`figma-key-guard at ${mainBase}`)
    expect(result.stdout).toContain('allowlisted public file ids in 0 paths from HEAD')
    expect(result.stdout).toContain(`FAIL url ${headSha.slice(0, 12)}:docs/link.md`)
    expect(result.stdout).not.toContain('SyntheticKeyExample0')
  })

  it('should accept the url once the allowlist entry has landed on the default branch', async () => {
    await writeFile(allowlistFile(), entry)
    const landed = commit()
    const result = scan({ BASE: mainBase, HEAD_SHA: headSha })
    expect(result.status, result.stdout + result.stderr).toBe(0)
    expect(result.stdout).toContain(`figma-key-guard at ${landed}`)
    expect(result.stdout).toContain('allowlisted public file ids in 1 paths from HEAD')
    expect(result.stdout).toContain('result: PASS')
  })
})
