import { spawnSync } from 'node:child_process'
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { parse } from 'yaml'

// The workflows that publish Figma output: nothing figma-key-guard rejects may be printed or
// uploaded. Structural checks cover every sink.
const repoRoot = path.resolve(import.meta.dirname, '../../../..')
const workflows = path.join(repoRoot, '.github/workflows')

type Step = { text: string; name: string; id?: string; condition: string; run: string }

function steps(file: string): Step[] {
  const source = readFileSync(path.join(workflows, file), 'utf8')
  return source
    .split(/\n(?= {6}- )/)
    .slice(1)
    .map((text) => {
      const runBlock = /\n {8}run: \|\n((?: {10}.*\n?| *\n)*)/.exec(text)?.[1] ?? ''
      return {
        text,
        name: /- name: (.*)/.exec(text)?.[1] ?? /uses: (\S+)/.exec(text)?.[1] ?? '',
        id: /\n\s+id: (\S+)/.exec(text)?.[1],
        condition: /\n\s+if: (.*)/.exec(text)?.[1] ?? '',
        run: runBlock.replace(/^ {10}/gm, '')
      }
    })
}

const isUpload = (step: Step) => step.text.includes('actions/upload-artifact')
const isGuard = (step: Step) => step.text.includes('key-guard.mjs') && step.id !== undefined
const printsReport = (step: Step) => step.text.includes('cat "$REPORT_FILE"')
const runsAfterFailure = (step: Step) => /always\(\)|failure\(\)/.test(step.condition)

describe.each(['ui-library.yml'])('%s', (file) => {
  it('should run the key guard before every artifact upload', () => {
    const all = steps(file)
    for (const [index, step] of all.entries()) {
      if (!isUpload(step)) continue
      expect(
        all.slice(0, index).some((item) => item.text.includes('key-guard.mjs')),
        step.name
      ).toBe(true)
    }
  })

  it('should gate every sink that can run after a failure on a guard step succeeding', () => {
    const all = steps(file)
    const guardIds = all.filter(isGuard).map((step) => step.id!)
    for (const step of all.filter((item) => isUpload(item) || printsReport(item))) {
      if (!runsAfterFailure(step)) continue
      expect(
        guardIds.some((id) => step.condition.includes(`steps.${id}.outcome == 'success'`)),
        step.name
      ).toBe(true)
    }
  })
})

// The Figma file key never reaches code a pull request controls. A pull_request workflow runs the pull
// request's own workflow file and scripts, so none may reference the secret; the key check runs in a
// workflow_run workflow that executes only the default branch's code and reads the pull request as data.
type WorkflowStep = {
  id?: string
  uses?: string
  run?: string
  with?: Record<string, unknown>
  env?: Record<string, unknown>
}
type Workflow = {
  on: Record<string, unknown> | string | string[]
  permissions?: Record<string, string>
  env?: Record<string, unknown>
  jobs: Record<string, { name?: string; env?: Record<string, unknown>; steps: WorkflowStep[] }>
}
const workflowFiles = readdirSync(workflows).filter((file) => /\.ya?ml$/.test(file))
const load = (file: string) => parse(readFileSync(path.join(workflows, file), 'utf8')) as Workflow
const triggers = (workflow: Workflow) =>
  typeof workflow.on === 'string'
    ? [workflow.on]
    : Array.isArray(workflow.on)
      ? workflow.on
      : Object.keys(workflow.on)
const SECRET = 'secrets.FIGMA_FILE_KEY'

describe('the ui-library.yml url scan', () => {
  const workflow = load('ui-library.yml')
  const guardSteps = workflow.jobs['key-guard']!.steps

  it('should prefetch every LFS object of the outgoing range, unfiltered, before scanning it', () => {
    const prefetch = guardSteps.findIndex((step) => step.run?.includes('lfs-prefetch.mjs'))
    const scan = guardSteps.findIndex(
      (step) =>
        step.run?.trim() ===
        'node library/ui/figma/key-guard.mjs --no-worktree --range "$RANGE" --allowlist-from "$HEAD_SHA"'
    )
    expect(prefetch).toBeGreaterThan(-1)
    expect(scan).toBeGreaterThan(prefetch)
    const run = guardSteps[prefetch]!.run!
    expect(run).toContain('key-guard.mjs --no-worktree --range "$RANGE" --lfs-objects')
    expect(run).toContain('--endpoint "https://github.com/$REPOSITORY.git/info/lfs"')
    expect(run).not.toMatch(/--include|lfs fetch|lfs pull|ui_changed|\$changed/)
    expect(guardSteps[scan]!.env ?? {}).not.toHaveProperty('READ_TOKEN')
  })

  it('should read the url allowlist of the unprivileged scan from the pull request head, not the merge commit', () => {
    const scan = guardSteps.find((step) =>
      step.run?.includes('--no-worktree --range "$RANGE" --allowlist-from')
    )
    expect(scan?.env).toMatchObject({
      HEAD_SHA: '${{ github.event.pull_request.head.sha || github.sha }}'
    })
  })
})

describe('the Figma file key secret', () => {
  it.each(workflowFiles)(
    'should not be referenced by %s if a pull request can trigger it',
    (file) => {
      const pullRequestTriggered = triggers(load(file)).some((name) =>
        ['pull_request', 'pull_request_target'].includes(name)
      )
      if (pullRequestTriggered)
        expect(readFileSync(path.join(workflows, file), 'utf8')).not.toContain(SECRET)
    }
  )

  it('should be referenced only by the workflow_run key scan', () => {
    const holders = workflowFiles.filter((file) =>
      readFileSync(path.join(workflows, file), 'utf8').includes(SECRET)
    )
    expect(holders).toEqual(['ui-library-key-scan.yml'])
  })
})

describe('ui-library-key-scan.yml', () => {
  const workflow = load('ui-library-key-scan.yml')
  const allSteps = Object.values(workflow.jobs).flatMap((job) => job.steps)
  const runs = allSteps.flatMap((step) => (step.run ? [step.run] : []))

  it('should run only from workflow_run, with the least permissions it needs', () => {
    expect(triggers(workflow)).toEqual(['workflow_run'])
    expect(workflow.permissions).toEqual({ actions: 'read', checks: 'write', contents: 'read' })
  })

  it('should check out only the default branch, into trusted/', () => {
    const checkouts = allSteps.filter((step) => step.uses?.startsWith('actions/checkout@'))
    expect(checkouts).toHaveLength(1)
    expect(checkouts[0]!.with).toMatchObject({
      ref: '${{ github.event.repository.default_branch }}',
      path: 'trusted'
    })
  })

  it('should execute nothing but the trusted copy of the key guard', () => {
    for (const run of runs) {
      expect(run, run).not.toMatch(/\b(pnpm|npm|npx|yarn|corepack)\b/)
      for (const [, script] of run.matchAll(/\bnode\s+(\S+)/g))
        expect(script!.startsWith('trusted/'), run).toBe(true)
      expect(run, run).not.toMatch(/\b(bash|sh|source)\s+\S/)
    }
  })

  it('should give the secret to the key guard step only, never at job or workflow level', () => {
    expect(JSON.stringify(workflow.env ?? {})).not.toContain(SECRET)
    for (const job of Object.values(workflow.jobs))
      expect(JSON.stringify(job.env ?? {})).not.toContain(SECRET)
    const holders = allSteps.filter((step) => JSON.stringify(step).includes(SECRET))
    expect(holders).toHaveLength(1)
    expect(allSteps.indexOf(holders[0]!)).toBeGreaterThan(0)
    expect(Object.keys(holders[0]!.env ?? {})).toContain('FIGMA_FILE_KEY')
    expect(holders[0]!.run).toMatch(/^node trusted\/library\/ui\/figma\/key-guard\.mjs /m)
  })

  it('should read the url allowlist of the privileged scan from the default branch, never the scanned head', () => {
    const scan = allSteps.find((step) => step.id === 'scan')!
    expect(scan.run).toContain('--root trusted --no-worktree')
    expect(scan.run).toContain('--allowlist-from HEAD ')
    expect(scan.run).not.toMatch(/--allowlist-from "\$(HEAD_SHA|BASE)"/)
  })

  it('should validate the triggering run before anything else, requiring a successful run', () => {
    const [first] = allSteps
    expect(first!.uses).toBeUndefined()
    expect(first!.env).toMatchObject({ CONCLUSION: '${{ github.event.workflow_run.conclusion }}' })
    expect(first!.run).toContain('[ "$CONCLUSION" = success ]')
    expect(first!.run).toContain('[ "$HEAD_REPO" = "$REPOSITORY" ]')
    expect(JSON.stringify(first)).not.toContain('secrets.')
  })

  it('should pin the triggering workflow by file path and id, never by display name', () => {
    const [first] = allSteps
    expect(first!.env).toMatchObject({
      WORKFLOW_PATH: '${{ github.event.workflow_run.path }}',
      WORKFLOW_ID: '${{ github.event.workflow_run.workflow_id }}'
    })
    expect(first!.run).toContain('[ "$WORKFLOW_PATH" = .github/workflows/ui-library.yml ]')
    expect(first!.run).toContain('actions/workflows/ui-library.yml" --jq .id')
    expect(first!.run).toContain('[ "$WORKFLOW_ID" = "$expected_id" ]')
    expect(JSON.stringify(first)).not.toMatch(/workflow_run\.name|Shared UI library/)
  })

  it('should tie the run, the pull request head and the artifact together before anything is downloaded', () => {
    const [first] = allSteps
    expect(first!.run).toContain('[ "$head" = "$HEAD_SHA" ]')
    expect(first!.run).toContain('[ "$(jq \'length\' <<< "$PULL_REQUESTS")" = 1 ]')
    const artifact = allSteps.find((step) => step.run?.includes('/artifacts'))!
    expect(artifact.run).toContain('ui-review-$HEAD_SHA')
    expect(artifact.run).toContain('actions/runs/$RUN_ID/artifacts')
    expect(artifact.run).toContain('.workflow_run.id == $RUN_ID')
    expect(artifact.run).toContain('.workflow_run.head_sha == \\"$HEAD_SHA\\"')
    const download = allSteps.find((step) => step.uses?.startsWith('actions/download-artifact@'))!
    expect(download.with).toMatchObject({
      name: 'ui-review-${{ steps.trigger.outputs.head }}',
      'run-id': '${{ github.event.workflow_run.id }}',
      path: '${{ runner.temp }}/pr-artifact'
    })
  })

  it('should read no Git configuration but its own, and keep the checkout credential-free', () => {
    expect(workflow.env?.GIT_CONFIG_GLOBAL).toBeUndefined()
    for (const step of allSteps.filter(
      (step) => step.run?.includes('git -C trusted') || step.run?.includes('node trusted/')
    ))
      expect(step.env).toMatchObject({ GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null' })
    const checkout = allSteps.find((step) => step.uses?.startsWith('actions/checkout@'))!
    expect(checkout.with).toMatchObject({
      'persist-credentials': false,
      lfs: false,
      'set-safe-directory': false
    })
  })

  it('should prefetch every LFS object the scan reads, by oid, from the fixed repository endpoint', () => {
    const lfs = allSteps.find((step) => step.run?.includes('lfs-prefetch.mjs'))!
    expect(lfs.run).toContain(
      'node trusted/library/ui/figma/key-guard.mjs --root trusted --no-worktree --range "$range"'
    )
    expect(lfs.run).toContain('--lfs-objects > "$RUNNER_TEMP/lfs-objects.jsonl"')
    expect(lfs.run).toContain('[ "$required" != true ] || tree=(--tree "$HEAD_SHA:library/ui")')
    expect(lfs.run).toContain(
      '--endpoint "https://github.com/$REPOSITORY.git/info/lfs" --objects "$RUNNER_TEMP/lfs-objects.jsonl"'
    )
    // No path filter may narrow the history the scan covers.
    expect(lfs.run).not.toMatch(/--include|lfs fetch|lfs pull/)
    expect(lfs.run).toContain('::add-mask::')
    expect(lfs.run).not.toMatch(/git (config|credential)|--global|--system|\.lfsconfig/)
    expect(lfs.env).toMatchObject({ REPOSITORY: '${{ github.repository }}' })
    expect(JSON.stringify(lfs)).not.toContain(SECRET)
    const scan = allSteps.find((step) => JSON.stringify(step).includes(SECRET))!
    expect(allSteps.indexOf(lfs)).toBeLessThan(allSteps.indexOf(scan))
    expect(JSON.stringify(scan)).not.toMatch(/READ_TOKEN|LFS_AUTHORIZATION|github\.token/)
  })

  it('should build the check summary from the scanner stdout alone', () => {
    const scan = allSteps.find((step) => JSON.stringify(step).includes(SECRET))!
    expect(scan.run).toContain('| tee "$RUNNER_TEMP/key-scan.txt"')
    expect(scan.run).not.toMatch(/2>&1|&>|2>\s*"?\$RUNNER_TEMP\/key-scan/)
    const report = allSteps.find((step) => step.run?.includes('check-runs'))!
    expect(report.run).toContain('head -c 60000 "$RUNNER_TEMP/key-scan.txt"')
    expect(report.run!.match(/key-scan\.txt/g)).toHaveLength(1)
    expect(JSON.stringify(report)).not.toContain(SECRET)
  })

  it('should scan the artifact through the limited, link-refusing enumerator', () => {
    const scan = allSteps.find((step) => JSON.stringify(step).includes(SECRET))!
    expect(scan.run).toContain('--artifact "$RUNNER_TEMP/pr-artifact"')
    expect(scan.run).not.toContain('--path')
  })

  it('should never let a skipped scan, a fork run included, conclude a passing check', () => {
    const [first] = allSteps
    const report = allSteps.find((step) => step.run?.includes('check-runs'))!
    expect(first!.run).not.toMatch(/neutral|skipped|conclusion=success/)
    expect(JSON.stringify(report.env)).not.toContain('outputs.conclusion')
    expect(report.run).toMatch(/if \[ -n "\$SKIP_REASON" \]; then\n\s+conclusion=failure\n/)
  })

  it('should report a failed check that says the key check could not run on a fork run, without the key', () => {
    const [first] = allSteps
    const report = allSteps.find((step) => step.run?.includes('check-runs'))!
    const root = mkdtempSync(path.join(os.tmpdir(), 'key-scan-fork-'))
    const sentinel = 'fake-figma-file-key-sentinel'
    try {
      const env = {
        PATH: process.env.PATH,
        FIGMA_FILE_KEY: sentinel,
        REPOSITORY: 'example/repo',
        HEAD_REPO: 'outside/fork',
        HEAD_SHA: '1'.repeat(40),
        RUN_ID: '123',
        WORKFLOW_PATH: '.github/workflows/ui-library.yml',
        WORKFLOW_ID: '42',
        EVENT: 'pull_request',
        CONCLUSION: 'success',
        PULL_REQUESTS: '[]',
        GITHUB_OUTPUT: path.join(root, 'out'),
        RUNNER_TEMP: root
      }
      writeFileSync(env.GITHUB_OUTPUT, '')
      const validate = spawnSync(
        'bash',
        ['-e', '-o', 'pipefail', '-c', `gh() { echo 42; }\n${first!.run!}`],
        { encoding: 'utf8', env }
      )
      expect(validate.status, validate.stderr).toBe(0)
      const output = Object.fromEntries(
        readFileSync(env.GITHUB_OUTPUT, 'utf8')
          .trim()
          .split('\n')
          .map((line) => line.split(/=(.*)/s).slice(0, 2))
      )
      expect(output.scan).toBe('false')
      const posted = spawnSync(
        'bash',
        [
          '-e',
          '-o',
          'pipefail',
          '-c',
          `gh() { printf '%s\\n' "$@" > "$RUNNER_TEMP/report"; }\n${report.run!}`
        ],
        {
          encoding: 'utf8',
          env: {
            ...env,
            OUTCOME: 'skipped',
            SKIP_REASON: output.reason,
            SKIP_CONCLUSION: 'neutral'
          }
        }
      )
      expect(posted.status, posted.stderr).toBe(0)
      const request = readFileSync(path.join(root, 'report'), 'utf8')
      expect(request).toContain('conclusion=failure')
      expect(request).not.toMatch(/conclusion=(neutral|success|skipped)/)
      expect(request).toMatch(
        /output\[summary\]=Not scanned: fork run: the key check could not run/
      )
      for (const text of [request, validate.stdout, validate.stderr, posted.stdout, posted.stderr])
        expect(text).not.toContain(sentinel)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})

describe('when the Figma capture lives in figma-design-system', () => {
  const ui = () => load('ui-library.yml')
  const runs = (job: string) => ui().jobs[job]!.steps.map((step) => step.run ?? '')

  it('should require the port verification record on every pull request and push, before any install', () => {
    const step = ui().jobs['key-guard']!.steps.find((item) =>
      item.run?.includes('port-verification.mjs')
    )!
    expect(step.run).toContain('--base "$base" --head "$HEAD_SHA"')
    expect(step.env).toMatchObject({
      PR_BASE: '${{ github.event.pull_request.base.sha }}',
      HEAD_SHA: '${{ github.event.pull_request.head.sha || github.sha }}'
    })
    expect(JSON.stringify(ui().jobs['key-guard'])).not.toContain('pnpm install')
  })

  it('should check the lock digests before installing, and the built gallery after building it', () => {
    const steps = runs('library')
    const digests = steps.findIndex((run) => run === 'node figma/lock-digests.mjs')
    const install = steps.findIndex((run) => run.startsWith('pnpm install'))
    const build = steps.findIndex((run) => run === 'pnpm review:build')
    const check = steps.findIndex((run) =>
      run.startsWith('node figma/review/scripts/check-build.mjs')
    )
    expect(digests).toBeGreaterThanOrEqual(0)
    expect(digests).toBeLessThan(install)
    expect(check).toBeGreaterThan(build)
  })

  it('should check out the library job without LFS and download no LFS object', () => {
    const steps = ui().jobs.library!.steps
    const checkout = steps.find((step) => step.uses?.startsWith('actions/checkout@'))!
    expect(checkout.with).toMatchObject({ lfs: false })
    expect(runs('library').join('\n')).not.toMatch(/\blfs\b/)
  })

  it('should run no check that needs the capture in this repository', () => {
    const all = [...runs('key-guard'), ...runs('library')].join('\n')
    for (const command of [
      'figma diff',
      'figma guard',
      'check-mappings',
      'promotion-verify',
      'review-manifest',
      'review:check',
      'fetch-source'
    ])
      expect(all).not.toContain(command)
  })
})

describe('the controller-owned push boundary', () => {
  it('should bind the metadata job name to the push event and keep PR check names stable', () => {
    const name = load('ui-library.yml').jobs['key-guard']!.name!

    expect(name).toContain("github.event_name == 'push'")
    expect(name).toContain(
      "format('UI key push boundary {0}..{1}', github.event.before, github.sha)"
    )
    expect(name).toContain("|| 'key-guard'")
  })

  it('should accept metadata only from a successful job of the triggering run and head', () => {
    const script = load('ui-library-key-scan.yml').jobs['key-scan']!.steps[0]!.run!

    expect(script).toContain('actions/runs/$RUN_ID/jobs')
    expect(script).toContain('.run_id == $RUN_ID')
    const query = script.split(String.fromCharCode(92)).join('')
    expect(query).toContain('.head_sha == "$HEAD_SHA"')
    expect(query).toContain('.conclusion == "success"')
    expect(script).not.toContain('base="$HEAD_SHA^1"')
  })
})

describe('when key scanning covers the whole repository', () => {
  it('should trigger without path filters and gate only the heavy library job', () => {
    const ui = load('ui-library.yml')
    expect(ui.on).toMatchObject({ push: { branches: ['main'] }, pull_request: null })
    expect(JSON.stringify(ui.on)).not.toContain('paths')
    expect(Object.keys(ui.on)).not.toContain('workflow_call')
    const concurrency = parse(
      readFileSync(path.join(workflows, 'ui-library.yml'), 'utf8')
    ).concurrency
    expect(concurrency.group).toContain(
      "github.event_name == 'pull_request' && github.ref || github.run_id"
    )
    expect((ui.jobs.library as unknown as { if: string }).if).toContain('ui_changed')
  })
  it('should run the key guard from the merge commit so a branch older than the scripts still has them', () => {
    const job = load('ui-library.yml').jobs['key-guard']!
    const checkout = job.steps.find((step) => step.uses?.startsWith('actions/checkout@'))!
    expect(checkout.with).toMatchObject({ ref: '${{ github.sha }}', 'fetch-depth': 0 })
    expect(JSON.stringify(checkout.with)).not.toContain('pull_request.head')
    const scope = job.steps.find((step) => step.run?.includes('range="$base..$HEAD_SHA"'))!
    expect(scope.env).toMatchObject({
      HEAD_SHA: '${{ github.event.pull_request.head.sha || github.sha }}',
      PR_BASE: '${{ github.event.pull_request.base.sha }}'
    })
  })

  it('should let checkout use its temporary global config while isolating the Git run steps', () => {
    const ui = load('ui-library-key-scan.yml')
    expect(ui.env?.GIT_CONFIG_GLOBAL).toBeUndefined()
    const all = ui.jobs['key-scan']!.steps
    expect(all.find((step) => step.uses?.includes('actions/checkout'))!.with).toMatchObject({
      'set-safe-directory': false
    })
    for (const step of all.filter(
      (step) => step.run?.includes('git -C trusted') || step.run?.includes('node trusted/')
    ))
      expect(step.env).toMatchObject({ GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null' })
  })
})
