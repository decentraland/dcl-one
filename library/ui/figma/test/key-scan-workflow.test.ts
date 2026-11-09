import { execFileSync, spawnSync } from 'node:child_process'
import { existsSync, realpathSync } from 'node:fs'
import { cp, mkdtemp, readFile, rm, writeFile, mkdir } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { parse } from 'yaml'
import { GUARD_COPY, KEY_STEP_TEMPLATES, REPORT_TEMPLATE } from './key-step.js'

const repo = path.resolve(import.meta.dirname, '../../../..')
const head = '1'.repeat(40),
  base = '2'.repeat(40)
type Step = { id?: string; run?: string; name?: string; env?: Record<string, unknown> }
const { jobs } = parse(
  await readFile(path.join(repo, '.github/workflows/ui-library-key-scan.yml'), 'utf8')
) as { jobs: { prepare: { steps: Step[] }; 'key-scan': { steps: Step[] } } }
// prepare holds no secret and hands the scan its inputs; key-scan holds the key.
const steps = jobs.prepare.steps
const scanSteps = jobs['key-scan'].steps
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
// prepare's report, for every scan that does not reach the key-scan job.
async function report(output: Record<string, string>, extra: Record<string, string> = {}) {
  const script = `gh() { printf '%s\\n' "$@" > "$RUNNER_TEMP/report"; }\n` + steps.at(-1)!.run!
  const result = spawnSync('bash', ['-e', '-o', 'pipefail', '-c', script], {
    cwd: root,
    encoding: 'utf8',
    env: { ...defaults(), SKIP_REASON: output.reason ?? '', ...extra }
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
    const { status, stderr, ids } = await selectArtifacts([
      artifact(8, { expired: true }),
      artifact(10, { name: 'ui-review-other' })
    ])
    expect(ids).toEqual([])
    expect(status).not.toBe(0)
    // The count check is the only failure: a missing jq, a filter error or a failed gh would print more.
    expect(stderr.trim()).toBe('The run must provide exactly one Review artifact for its head')
  })
})
const gitIn = (directory: string, ...args: string[]) =>
  execFileSync('git', args, { cwd: directory, stdio: 'pipe' }).toString().trim()
const commitIn = (directory: string) => {
  gitIn(directory, 'add', '-A')
  gitIn(
    directory,
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
  return gitIn(directory, 'rev-parse', 'HEAD')
}
// Runs prepare's inputs step in `job` (its workspace and RUNNER_TEMP), with the fetch from origin standing in as
// a ref update over objects already local and node recorded instead of executed.
async function prepareInputs(job: string, env: Record<string, string>) {
  const script =
    `git() { case "$*" in *' fetch '*) printf '%s\\n' "$*" >> "$RUNNER_TEMP/fetches"; ` +
    `spec="\${@: -1}"; spec="\${spec#+}"; command git -C trusted update-ref "\${spec#*:}" "\${spec%%:*}";; ` +
    `*) command git "$@";; esac; }\n` +
    `node() { printf '%s\\n' "$*" >> "$RUNNER_TEMP/node"; }\n` +
    steps.find((step) => step.id === 'inputs')!.run!
  const result = spawnSync('bash', ['-e', '-o', 'pipefail', '-c', script], {
    cwd: job,
    encoding: 'utf8',
    env: {
      ...defaults(),
      GITHUB_OUTPUT: path.join(job, 'out'),
      RUNNER_TEMP: job,
      READ_TOKEN: 'synthetic',
      ...env
    }
  })
  const node = await readFile(path.join(job, 'node'), 'utf8').catch(() => '')
  return { result, node }
}
describe('when selecting the LFS inputs of the scan', () => {
  const trusted = () => path.join(root, 'trusted')
  const git = (...args: string[]) => gitIn(trusted(), ...args)
  const commit = () => commitIn(trusted())
  beforeEach(async () => {
    await mkdir(trusted())
    git('init', '-q', '-b', 'main')
    await writeFile(path.join(trusted(), 'README'), 'clean')
    commit()
  })

  it('should list the whole range, any path, and the library/ui tree when UI changed, then prefetch it', async () => {
    const before = git('rev-parse', 'HEAD')
    await mkdir(path.join(trusted(), 'library/ui'), { recursive: true })
    await writeFile(path.join(trusted(), 'library/ui/transient'), 'synthetic input')
    commit()
    await rm(path.join(trusted(), 'library/ui/transient'))
    commit()
    const after = git('rev-parse', 'HEAD')

    const { result, node } = await prepareInputs(root, { BASE: before, HEAD_SHA: after })
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
    await mkdir(path.join(trusted(), 'apps/sites/public'), { recursive: true })
    await writeFile(path.join(trusted(), 'apps/sites/public/clean.bin'), 'pointer stand-in')
    commit()
    const after = git('rev-parse', 'HEAD')

    const { result, node } = await prepareInputs(root, { BASE: before, HEAD_SHA: after })
    expect(result.status, result.stderr).toBe(0)
    expect(await readFile(path.join(root, 'out'), 'utf8')).toContain('artifact_required=false')
    const [list, prefetch] = node.trim().split('\n')
    expect(list).toBe(
      `trusted/library/ui/figma/key-guard.mjs --root trusted --no-worktree --range ${before}..${after} --lfs-objects`
    )
    expect(prefetch).toContain('lfs-prefetch.mjs')
  })

  // What prepare uploads as key-scan-input: the commits as a bundle, and the prefetched LFS objects.
  it('should bundle the scanned refs on top of the default branch commit and copy the LFS store', async () => {
    const before = git('rev-parse', 'HEAD')
    git('checkout', '-q', '-b', 'feature')
    await writeFile(path.join(trusted(), 'feature.txt'), 'scanned')
    const head = commit()
    git('checkout', '-q', 'main')
    const oid = 'a'.repeat(64)
    const object = path.join(trusted(), '.git/lfs/objects/aa/aa', oid)
    await mkdir(path.dirname(object), { recursive: true })
    await writeFile(object, 'prefetched')

    const { result } = await prepareInputs(root, { BASE: before, HEAD_SHA: head })
    expect(result.status, result.stderr).toBe(0)
    const bundle = path.join(root, 'key-scan-input/commits.bundle')
    const heads = git('bundle', 'list-heads', bundle)
    expect(heads).toContain(`${head} refs/key-scan/head`)
    expect(heads).toContain(`${before} refs/key-scan/base`)
    expect(heads).toContain(`${before} refs/key-scan/main`)
    expect(await readFile(path.join(root, 'key-scan-input/lfs/aa/aa', oid), 'utf8')).toBe(
      'prefetched'
    )
  })

  it('should still write a bundle when every scanned commit is already on the default branch', async () => {
    const before = git('rev-parse', 'HEAD')
    await writeFile(path.join(trusted(), 'pushed.txt'), 'pushed')
    const head = commit()
    const { result } = await prepareInputs(root, { BASE: before, HEAD_SHA: head })
    expect(result.status, result.stderr).toBe(0)
    expect(git('bundle', 'list-heads', path.join(root, 'key-scan-input/commits.bundle'))).toContain(
      `${head} refs/key-scan/head`
    )
  })

  it('should bundle a push with no earlier commit, and no base ref', async () => {
    const head = git('rev-parse', 'HEAD')
    const { result } = await prepareInputs(root, { BASE: '0'.repeat(40), HEAD_SHA: head })
    expect(result.status, result.stderr).toBe(0)
    const heads = git('bundle', 'list-heads', path.join(root, 'key-scan-input/commits.bundle'))
    expect(heads).toContain(`${head} refs/key-scan/head`)
    expect(heads).not.toContain('refs/key-scan/base')
  })
})
describe('when the privileged scan reads the url allowlist', () => {
  const url = 'https://www.figma' + '.com/design/AllowlistExample123/fixture'
  const entry = JSON.stringify([
    { path: 'docs/link.md', check: 'url', id: 'AllowlistExample123', reason: 'fixture' }
  ])
  // root is the key-scan job's workspace and RUNNER_TEMP; root/trusted its checkout of the default branch.
  const git = (...args: string[]) => gitIn(path.join(root, 'trusted'), ...args)
  const commit = () => commitIn(path.join(root, 'trusted'))
  const allowlistFile = () => path.join(root, 'trusted/library/ui/figma/key-guard-allow.json')
  const scanStep = scanSteps.find((step) => step.id === 'scan')!
  // The literal env of the scan step as the workflow writes it (GIT_NO_REPLACE_OBJECTS and the git config
  // isolation), so dropping one from the workflow drops it here too. Expressions are filled in below.
  const stepEnv = Object.fromEntries(
    Object.entries(scanStep.env ?? {}).flatMap(([name, value]) =>
      String(value).includes('${{') ? [] : [[name, String(value)]]
    )
  )
  // The prepare job, on its own machine: a full clone (the default branch, the scanned branch as a remote), whose inputs
  // step writes key-scan-input. The artifact is then copied into the key-scan workspace, and the scanned branch
  // and its objects are removed there, so the scan finds them only in the bundle.
  async function handOver(env: Record<string, string>) {
    const job = path.join(root, 'prepare')
    await mkdir(job)
    execFileSync('git', ['clone', '-q', '--no-local', path.join(root, 'trusted'), 'trusted'], {
      cwd: job,
      stdio: 'pipe'
    })
    const { result } = await prepareInputs(job, env)
    expect(result.status, result.stderr).toBe(0)
    await cp(path.join(job, 'key-scan-input'), path.join(root, 'key-scan-input'), {
      recursive: true
    })
    await rm(job, { recursive: true, force: true })
    git('branch', '-D', 'feature')
    git('reflog', 'expire', '--expire=now', '--all')
    git('gc', '-q', '--prune=now')
  }
  // Runs the real scan step with main's real key guard and a synthetic key, pinned like the workflow pins it:
  // TRUSTED_SHA is the event's default branch commit, here the checkout's HEAD when the run starts unless a
  // case sets it. RUNNER_TEMP is a real path, as on a runner: the guard only starts when the path node was
  // given is the path of its own module.
  const run = (env: Record<string, string>, without: string[] = []) =>
    spawnSync('bash', ['-e', '-o', 'pipefail', '-c', scanStep.run!], {
      cwd: root,
      encoding: 'utf8',
      env: Object.fromEntries(
        Object.entries({
          ...defaults(),
          ...stepEnv,
          TRUSTED_SHA: git('rev-parse', 'HEAD'),
          FIGMA_FILE_KEY: 'SyntheticKeyExample0',
          ARTIFACT_REQUIRED: 'false',
          RUNNER_TEMP: realpathSync(root),
          ...env
        }).filter(([name]) => !without.includes(name))
      )
    })
  const scan = async (env: Record<string, string>, without: string[] = []) => {
    await handOver({ BASE: env.BASE!, HEAD_SHA: env.HEAD_SHA! })
    return run(env, without)
  }
  // Where the step writes the guard copy.
  const copy = () => path.join(root, path.basename(GUARD_COPY))

  it('should run the scan template of the policy', () => {
    expect(scanStep.run!.trim()).toBe(KEY_STEP_TEMPLATES[0]!.run)
    expect(stepEnv).toMatchObject({ GIT_NO_REPLACE_OBJECTS: '1' })
  })
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

  it('should take the allowlist from the default branch, so a head-only entry does not suppress the url', async () => {
    await handOver({ BASE: mainBase, HEAD_SHA: headSha })
    // The scanned commit is not in the key-scan checkout: only the bundle brings it.
    expect(() => git('cat-file', '-e', headSha)).toThrow()
    const result = run({ BASE: mainBase, HEAD_SHA: headSha })
    expect(result.status).not.toBe(0)
    expect(result.stdout).toContain(`figma-key-guard at ${mainBase}`)
    expect(result.stdout).toContain(`allowlisted public file ids in 0 paths from ${mainBase}`)
    expect(result.stdout).toContain(`range: ${mainBase}..${headSha}`)
    expect(result.stdout).toContain(`FAIL url ${headSha.slice(0, 12)}:docs/link.md`)
    expect(result.stdout).not.toContain('SyntheticKeyExample0')
  })

  it('should accept the url once the allowlist entry has landed on the default branch', async () => {
    await writeFile(allowlistFile(), entry)
    const landed = commit()
    const result = await scan({ BASE: mainBase, HEAD_SHA: headSha })
    expect(result.status, result.stdout + result.stderr).toBe(0)
    expect(result.stdout).toContain(`figma-key-guard at ${landed}`)
    expect(result.stdout).toContain(`allowlisted public file ids in 1 paths from ${landed}`)
    expect(result.stdout).toContain('result: PASS')
  })

  it('should fail when the scan inputs are missing, before running anything', async () => {
    const result = run({ BASE: mainBase, HEAD_SHA: headSha })
    expect(result.status).not.toBe(0)
    expect(result.stdout).not.toContain('check key: ran')
  })

  it.each([
    ['HEAD_SHA', '--output=x'],
    ['BASE', 'main'],
    ['HEAD_SHA', '']
  ])('should fail before reading anything when %s is %j', async (name, value) => {
    await handOver({ BASE: mainBase, HEAD_SHA: headSha })
    const result = run({ BASE: mainBase, HEAD_SHA: headSha, [name]: value })
    expect(result.status).not.toBe(0)
    expect(existsSync(copy())).toBe(false)
  })

  // Stands in for a key guard an earlier step replaced: it would leak the key and claim the check ran.
  const tampered = [
    "import { writeFileSync } from 'node:fs'",
    "writeFileSync(`${process.env.RUNNER_TEMP}/leak`, process.env.FIGMA_FILE_KEY ?? '')",
    "console.log('check key: ran')",
    "console.log('result: PASS')"
  ].join('\n')

  // The key-scan job has no step before the key step that could make these changes; the cases below make them
  // anyway, to show the key step's own pinning holds without that rule.
  it('should run the committed key guard, never a working tree copy that was replaced', async () => {
    await writeFile(allowlistFile(), entry)
    const landed = commit()
    const committed = await readFile(path.join(root, 'trusted/library/ui/figma/key-guard.mjs'))
    await writeFile(path.join(root, 'trusted/library/ui/figma/key-guard.mjs'), tampered)
    const result = await scan({ BASE: mainBase, HEAD_SHA: headSha })
    expect(result.status, result.stdout + result.stderr).toBe(0)
    expect(existsSync(path.join(root, 'leak'))).toBe(false)
    expect(result.stdout).toContain(`allowlisted public file ids in 1 paths from ${landed}`)
    expect(await readFile(copy())).toEqual(committed)
  })

  it('should run the pinned guard when a tampered one is committed onto the checkout HEAD', async () => {
    await writeFile(allowlistFile(), entry)
    const pinned = commit()
    const committed = await readFile(path.join(root, 'trusted/library/ui/figma/key-guard.mjs'))
    // A tampered guard, and an allowlist that would hide every url, committed onto HEAD.
    await writeFile(path.join(root, 'trusted/library/ui/figma/key-guard.mjs'), tampered)
    await writeFile(allowlistFile(), '[]')
    const moved = commit()
    expect(git('cat-file', 'blob', 'HEAD:library/ui/figma/key-guard.mjs')).toContain('leak')
    const result = await scan({ BASE: mainBase, HEAD_SHA: headSha, TRUSTED_SHA: pinned })
    expect(result.status, result.stdout + result.stderr).toBe(0)
    expect(existsSync(path.join(root, 'leak'))).toBe(false)
    expect(await readFile(copy())).toEqual(committed)
    expect(result.stdout).toContain(`allowlisted public file ids in 1 paths from ${pinned}`)
    // The summary names the commit the guard was read from, not the checkout HEAD that moved.
    expect(result.stdout).toContain(`figma-key-guard at ${pinned}\n`)
    expect(result.stdout).not.toContain(moved)
  })

  // GIT_NO_REPLACE_OBJECTS is left out of this run, so only the copy's own --no-replace-objects keeps the
  // replacement out.
  it('should ignore a replace ref for the committed key guard', async () => {
    await writeFile(allowlistFile(), entry)
    commit()
    const committed = await readFile(path.join(root, 'trusted/library/ui/figma/key-guard.mjs'))
    await handOver({ BASE: mainBase, HEAD_SHA: headSha })
    await writeFile(path.join(root, 'tampered.mjs'), tampered)
    const evil = git('hash-object', '-w', path.join(root, 'tampered.mjs'))
    git('replace', git('rev-parse', 'HEAD:library/ui/figma/key-guard.mjs'), evil)
    // Without --no-replace-objects, cat-file would print the tampered blob.
    expect(git('cat-file', 'blob', 'HEAD:library/ui/figma/key-guard.mjs')).toContain('leak')
    const result = run({ BASE: mainBase, HEAD_SHA: headSha }, ['GIT_NO_REPLACE_OBJECTS'])
    expect(result.status, result.stdout + result.stderr).toBe(0)
    expect(existsSync(path.join(root, 'leak'))).toBe(false)
    expect(await readFile(copy())).toEqual(committed)
  })

  // The step env's GIT_NO_REPLACE_OBJECTS is what keeps the guard's own reads of the scanned commits honest. The
  // scanned blob is not in the checkout yet, so the replace ref is written by name; the bundle brings the blob.
  it('should scan the committed blob of a scanned file that a replace ref swaps for a clean one', async () => {
    const blob = git('rev-parse', `${headSha}:docs/link.md`)
    await handOver({ BASE: mainBase, HEAD_SHA: headSha })
    await writeFile(path.join(root, 'clean.md'), 'nothing to see\n')
    git('update-ref', `refs/replace/${blob}`, git('hash-object', '-w', path.join(root, 'clean.md')))
    const result = run({ BASE: mainBase, HEAD_SHA: headSha })
    expect(git('cat-file', 'blob', `${headSha}:docs/link.md`)).toBe('nothing to see')
    expect(result.stdout).toContain(`FAIL url ${headSha.slice(0, 12)}:docs/link.md`)
    expect(result.status).not.toBe(0)
  })

  // The bundle is data: its refs land only under refs/key-scan/, so one that names refs/replace changes nothing.
  it('should take only refs/key-scan/ from the bundle, never a replace ref it carries', async () => {
    await handOver({ BASE: mainBase, HEAD_SHA: headSha })
    const job = path.join(root, 'forge')
    execFileSync('git', ['clone', '-q', '--no-local', path.join(root, 'trusted'), job], {
      stdio: 'pipe'
    })
    gitIn(
      job,
      'fetch',
      '-q',
      path.join(root, 'key-scan-input/commits.bundle'),
      '+refs/key-scan/*:refs/key-scan/*'
    )
    await writeFile(path.join(root, 'tampered.mjs'), tampered)
    const evil = gitIn(job, 'hash-object', '-w', path.join(root, 'tampered.mjs'))
    gitIn(job, 'replace', gitIn(job, 'rev-parse', 'HEAD:library/ui/figma/key-guard.mjs'), evil)
    gitIn(
      job,
      'bundle',
      'create',
      path.join(root, 'key-scan-input/commits.bundle'),
      'refs/key-scan/head',
      'refs/key-scan/base',
      '--all'
    )
    const result = run({ BASE: mainBase, HEAD_SHA: headSha }, ['GIT_NO_REPLACE_OBJECTS'])
    expect(git('for-each-ref', 'refs/replace')).toBe('')
    expect(existsSync(path.join(root, 'leak'))).toBe(false)
    expect(result.stdout).toContain(`FAIL url ${headSha.slice(0, 12)}:docs/link.md`)
  })

  it('should replace a key guard copy planted in $RUNNER_TEMP before running it', async () => {
    await writeFile(copy(), tampered)
    const result = await scan({ BASE: mainBase, HEAD_SHA: headSha })
    expect(existsSync(path.join(root, 'leak'))).toBe(false)
    expect(result.stdout).toContain(`FAIL url ${headSha.slice(0, 12)}:docs/link.md`)
    expect(result.status).not.toBe(0)
  })

  // A directory at the copy path holds a package whose main is a tampered guard, which node would run if the
  // step went on to run the copy after the write failed.
  it('should fail before running anything when the committed key guard cannot be written out', async () => {
    await mkdir(copy())
    await writeFile(path.join(copy(), 'package.json'), JSON.stringify({ main: 'main.mjs' }))
    await writeFile(path.join(copy(), 'main.mjs'), tampered)
    const result = await scan({ BASE: mainBase, HEAD_SHA: headSha })
    expect(result.status).not.toBe(0)
    expect(result.stdout).not.toContain('check key: ran')
    expect(existsSync(path.join(root, 'leak'))).toBe(false)
  })
})
describe('when the key-scan job reports its result', () => {
  const post = async (env: Record<string, string>) => {
    const script =
      `gh() { printf '%s\\n' "$@" > "$RUNNER_TEMP/report"; }\n` + scanSteps.at(-1)!.run!
    const result = spawnSync('bash', ['-e', '-o', 'pipefail', '-c', script], {
      cwd: root,
      encoding: 'utf8',
      env: { ...defaults(), ...env }
    })
    expect(result.status, result.stderr).toBe(0)
    return readFile(path.join(root, 'report'), 'utf8')
  }

  it('should run the report template of the policy', () => {
    expect(scanSteps.at(-1)!.run!.trim()).toBe(REPORT_TEMPLATE.run)
  })

  it('should pass only a scan that succeeded, with the scanner stdout as the summary', async () => {
    await writeFile(path.join(root, 'key-scan.txt'), 'check key: ran\nresult: PASS\n')
    const request = await post({ OUTCOME: 'success' })
    expect(request).toContain('conclusion=success')
    expect(request).toContain('output[summary]=check key: ran\nresult: PASS')
    expect(request).toContain(`head_sha=${head}`)
  })

  it.each(['failure', 'skipped', 'cancelled', ''])(
    'should fail the check when the scan outcome is %j',
    async (outcome) => {
      const request = await post({ OUTCOME: outcome })
      expect(request).toContain('conclusion=failure')
      expect(request).toContain('output[summary]=The scan did not run.')
    }
  )
})
