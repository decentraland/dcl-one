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
  name?: string
  uses?: string
  run?: string
  shell?: string
  'working-directory'?: string
  with?: Record<string, unknown>
  env?: Record<string, unknown>
}
type Job = {
  name?: string
  needs?: string | string[]
  if?: string
  env?: Record<string, unknown>
  environment?: string | { name?: string; deployment?: boolean }
  permissions?: Record<string, string>
  outputs?: Record<string, string>
  container?: unknown
  services?: unknown
  defaults?: unknown
  uses?: string
  secrets?: string | Record<string, unknown>
  steps?: WorkflowStep[]
}
type Workflow = {
  on: Record<string, unknown> | string | string[]
  permissions?: Record<string, string>
  env?: Record<string, unknown>
  defaults?: unknown
  jobs: Record<string, Job>
}
// A reusable-workflow call has no steps.
const stepsOf = (job: Job | undefined) => job?.steps ?? []
const workflowFiles = readdirSync(workflows).filter((file) => /\.ya?ml$/.test(file))
const load = (file: string) => parse(readFileSync(path.join(workflows, file), 'utf8')) as Workflow
const triggers = (workflow: Workflow) =>
  typeof workflow.on === 'string'
    ? [workflow.on]
    : Array.isArray(workflow.on)
      ? workflow.on
      : Object.keys(workflow.on)
const SECRET = 'secrets.FIGMA_FILE_KEY'
// Context names ignore case, so every spelling of the secret counts.
const KEY_REFERENCE = /\bsecrets\.figma_file_key\b/i
// The whole secrets context (toJSON(secrets), secrets['…'], secrets[…]) reaches the key without naming it.
const expressions = (value: unknown) =>
  [...JSON.stringify(value ?? null).matchAll(/\$\{\{([\s\S]*?)\}\}/g)].map((match) => match[1]!)
const readsWholeSecrets = (value: unknown) =>
  expressions(value).some((expression) => /\bsecrets\b(?!\s*\.\s*[A-Za-z_])/i.test(expression))
const referencesKey = (value: unknown) =>
  KEY_REFERENCE.test(JSON.stringify(value ?? null)) || readsWholeSecrets(value)
const environmentName = (job: Job) =>
  String(typeof job.environment === 'string' ? job.environment : (job.environment?.name ?? ''))
// FIGMA_FILE_KEY is a repository secret, so a key reader is any job that references it in any form, including
// a call that passes every secret (secrets: inherit) or names the key in its secrets map.
const passesKey = (job: Job) =>
  typeof job.secrets === 'string'
    ? job.secrets.trim().toLowerCase() === 'inherit'
    : Object.keys(job.secrets ?? {}).some((name) => /^figma_file_key$/i.test(name))
const isKeyReaderJob = (job: Job) => referencesKey(job) || passesKey(job)
const isKeyReader = (workflow: Workflow) =>
  referencesKey(workflow) || Object.values(workflow.jobs ?? {}).some(isKeyReaderJob)
// Environments a key-holding job may declare. None for now: the key needs no environment, so a reader that
// declares one is reported for a decision rather than accepted.
const KEY_ENVIRONMENTS: string[] = []
// A reader may only be started by events that run the default branch's copy of the workflow: workflow_run and
// push to main. deployment_status and deployment run the file at the deployed commit, so they are rejected.
const DEFAULT_BRANCH = 'main'
const PUSH_FILTERS = ['branches', 'paths', 'paths-ignore']
function triggerViolations(workflow: Workflow): string[] {
  const problems: string[] = []
  for (const name of triggers(workflow)) {
    if (name === 'workflow_run') continue
    if (name === 'push') {
      const push =
        typeof workflow.on === 'object' && !Array.isArray(workflow.on)
          ? (workflow.on.push as Record<string, unknown> | null | undefined)
          : undefined
      const branches = push?.branches
      if (
        !push ||
        Object.keys(push).some((key) => !PUSH_FILTERS.includes(key)) ||
        !Array.isArray(branches) ||
        branches.length !== 1 ||
        branches[0] !== DEFAULT_BRANCH
      )
        problems.push(`triggered by push beyond ${DEFAULT_BRANCH}`)
      continue
    }
    problems.push(`triggered by ${name}`)
  }
  return problems
}
// The only workflows that may read the key. web-previews-publish.yml is the CDN reviewer publisher; it is
// allowed before it exists on main, and it must pass the same rules as the key scan once it does.
const KEY_READERS = ['ui-library-key-scan.yml', 'web-previews-publish.yml']
// github.workflow_sha is the commit of the running workflow file. It is trusted only because the trigger rule
// admits only workflow_run and push to main, where that file is main's copy.
const TRUSTED_REFS = ['${{ github.event.repository.default_branch }}', '${{ github.workflow_sha }}']
// Actions a key-holding job may use, each pinned by a full commit sha.
const TRUSTED_ACTIONS =
  /^(actions\/checkout|actions\/setup-node|actions\/download-artifact|decentraland\/oddish-action)@[0-9a-f]{40}$/
// Variables that make the shell or node load code a step did not name.
const LOADER_VARIABLES =
  /^(NODE_OPTIONS|NODE_PATH|LD_PRELOAD|LD_LIBRARY_PATH|BASH_ENV|ENV|PATH|SHELLOPTS)$/i
// Where a command starts in a shell line.
const START = String.raw`(?:^|[;&|(]|\b(?:then|do|else)\b)\s*`
// Pipes and redirects of the key step may only write into $RUNNER_TEMP.
const TEMP_PATH = String.raw`"?\$\{?RUNNER_TEMP\}?/[^"\s]*"?`
const KEY_STEP_COMMANDS = [
  /^set -o pipefail$/,
  /^node (?:trusted\/)?library\/ui\/figma\/key-guard\.mjs(?: .*)?$/,
  new RegExp(`^tee(?: -a)? ${TEMP_PATH}$`),
  new RegExp(`^grep(?: -[A-Za-z]+)* (?:'[^']*'|"[^"$]*")(?: ${TEMP_PATH})?$`),
  new RegExp(`^head(?: -[cn] \\d+)?(?: ${TEMP_PATH})?$`),
  new RegExp(`^cat(?: ${TEMP_PATH})*$`),
  // Plain assignments and tests, which the scan needs to build its arguments; nothing they hold runs.
  /^[A-Za-z_]\w*=(?:\([^()]*\)|"[^"]*"|'[^']*'|[^\s"'()]*)$/,
  /^\[ [^[\]]* \]$/,
  /^fi$/
]
// What the step that maps the key may run: the commands above, joined by pipes, lists and redirects only.
function keyStepViolations(run: string): string[] {
  const problems: string[] = []
  if (/\$\(|`|<\(|>\(|&(?!&)|\/proc\b|environ/.test(run.replace(/&&/g, '')))
    problems.push('substitutes, backgrounds or reads the environment')
  const commands = run
    .replace(/\\\n/g, ' ')
    .split(/\n|&&|\|\||[;|]/)
    .map((part) =>
      part
        .trim()
        .replace(/^(?:if|then|else|elif)(?:\s+|$)/, '')
        .trim()
    )
    .filter(Boolean)
  for (const command of commands) {
    let rest = command
    for (const [redirect, target] of command.matchAll(
      /\d?>>?\s*("[^"]*"|\S+)|<\s*("[^"]*"|\S+)/g
    )) {
      if (!new RegExp(`^${TEMP_PATH}$`).test((target ?? '').trim()))
        problems.push(`redirects outside $RUNNER_TEMP: ${redirect}`)
      rest = rest.replace(redirect, '').trim()
    }
    if (!KEY_STEP_COMMANDS.some((allowed) => allowed.test(rest))) problems.push(`runs ${rest}`)
  }
  return problems
}

// A job that holds the key runs only from the default branch's workflow, declares no environment, checks out
// only trusted code, uses only pinned trusted actions, keeps the secret in a single step's env, runs no package manager or pull
// request script, and never prints the key. Pull request files may only arrive as data (fetched objects or
// downloaded artifacts).
function keyReaderViolations(workflow: Workflow): string[] {
  const problems: string[] = triggerViolations(workflow)
  if (readsWholeSecrets(workflow)) problems.push('reads the whole secrets context')
  const { jobs, ...top } = workflow
  if (referencesKey(top)) problems.push('key in workflow env')
  if (top.defaults !== undefined) problems.push('sets workflow defaults')
  for (const name of Object.keys(workflow.env ?? {}))
    if (LOADER_VARIABLES.test(name)) problems.push(`sets ${name}`)
  for (const [id, job] of Object.entries(jobs)) {
    if (!isKeyReaderJob(job)) continue
    const environment = environmentName(job)
    if (environment !== '' && !KEY_ENVIRONMENTS.includes(environment.toLowerCase()))
      problems.push(`${id}: declares environment ${environment}`)
    if (job.uses !== undefined) problems.push(`${id}: calls a reusable workflow`)
    const { steps: jobSteps = [], ...jobRest } = job
    if (referencesKey(jobRest)) problems.push(`${id}: key outside a step env`)
    if (job.container !== undefined) problems.push(`${id}: runs in a container`)
    if (job.services !== undefined) problems.push(`${id}: starts service containers`)
    if (job.defaults !== undefined) problems.push(`${id}: sets job defaults`)
    for (const name of Object.keys(job.env ?? {}))
      if (LOADER_VARIABLES.test(name)) problems.push(`${id}: sets ${name}`)
    for (const step of jobSteps) {
      const { env, ...stepRest } = step
      if (referencesKey(stepRest)) problems.push(`${id}: key outside a step env`)
      for (const name of Object.keys(env ?? {}))
        if (LOADER_VARIABLES.test(name)) problems.push(`${id}: sets ${name}`)
      if (step.uses !== undefined) {
        if (!TRUSTED_ACTIONS.test(step.uses)) problems.push(`${id}: uses ${step.uses}`)
        if (step.uses.startsWith('actions/checkout@')) {
          const ref = String(step.with?.ref ?? '')
          if (!TRUSTED_REFS.includes(ref))
            problems.push(`${id}: checkout of ${ref || 'the event ref'}`)
          if (step.with?.['persist-credentials'] !== false)
            problems.push(`${id}: checkout persists credentials`)
        }
      }
      const run = step.run ?? ''
      if (referencesKey(env)) {
        if (step.shell !== undefined) problems.push(`${id}: key step sets its shell`)
        if (step['working-directory'] !== undefined)
          problems.push(`${id}: key step changes its working directory`)
        for (const problem of keyStepViolations(run)) problems.push(`${id}: key step ${problem}`)
      }
      for (const tool of ['pnpm', 'npm', 'npx', 'yarn', 'corepack'])
        if (new RegExp(`\\b${tool}\\b`).test(run)) problems.push(`${id}: runs ${tool}`)
      if (/\b(bash|sh|source)\s+\S/.test(run)) problems.push(`${id}: runs a shell script`)
      if (
        new RegExp(`${START}\\.{1,2}/`, 'm').test(run) ||
        new RegExp(`${START}\\.\\s+\\S`, 'm').test(run)
      )
        problems.push(`${id}: runs a local script`)
      for (const [, script] of run.matchAll(
        /\bpython[\d.]*\s+(?:-[A-Za-z]+\s+)*("[^"]*"|'[^']*'|\S+)/g
      ))
        if (!/^["']?(trusted|tooling)\//.test(script!))
          problems.push(`${id}: runs python ${script}`)
      if (new RegExp(`${START}make\\b`, 'm').test(run)) problems.push(`${id}: runs make`)
      if (
        /\b(node|python[\d.]*)\s+(?:-\S+\s+)*["']?(\$\{?RUNNER_TEMP|(\.\/)?\.preview\b)/.test(run)
      )
        problems.push(`${id}: runs a downloaded file`)
      if (/\bgit\b[^\n;|&]*\b(checkout|switch|worktree)\b[^\n;|&]*refs\/key-scan/.test(run))
        problems.push(`${id}: checks out the scanned commits`)
      if (
        /\$\{?!?FIGMA_FILE_KEY\b|\$\{!|\bprintenv\b|set -x|set -o xtrace/i.test(run) ||
        new RegExp(`${START}env\\s*(?:$|[;&|>])`, 'm').test(run)
      )
        problems.push(`${id}: prints the key`)
    }
  }
  return problems
}

describe('the ui-library.yml url scan', () => {
  const workflow = load('ui-library.yml')
  const guardSteps = stepsOf(workflow.jobs['key-guard'])

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
      if (pullRequestTriggered) expect(isKeyReader(load(file))).toBe(false)
    }
  )

  it('should be read only by the key scan and, once it lands, the web preview publisher', () => {
    const holders = workflowFiles.filter((file) => isKeyReader(load(file)))
    expect(holders).toContain('ui-library-key-scan.yml')
    for (const file of holders) expect(KEY_READERS, file).toContain(file)
  })

  it.each(workflowFiles)(
    'should hold the key in %s only under the trusted reader rules',
    (file) => {
      if (!isKeyReader(load(file))) return
      expect(keyReaderViolations(load(file))).toEqual([])
    }
  )

  it('should reject a pull request workflow that checks out the head and runs its scripts', () => {
    const untrusted = parse(`
on: pull_request
jobs:
  test:
    runs-on: ubuntu-26.04
    environment: web-preview
    steps:
      - uses: actions/checkout@v7
        with:
          ref: \${{ github.event.pull_request.head.sha }}
      - env:
          FIGMA_FILE_KEY: \${{ secrets.FIGMA_FILE_KEY }}
        run: pnpm test && echo "$FIGMA_FILE_KEY"
`) as Workflow
    expect(keyReaderViolations(untrusted)).toEqual(
      expect.arrayContaining([
        'triggered by pull_request',
        'test: checkout of ${{ github.event.pull_request.head.sha }}',
        'test: declares environment web-preview',
        'test: checkout persists credentials',
        'test: runs pnpm',
        'test: prints the key'
      ])
    )
  })

  it('should reject a reader that holds the key at job level', () => {
    const loose = parse(`
on:
  workflow_run:
    workflows: [x]
    types: [completed]
jobs:
  scan:
    runs-on: ubuntu-26.04
    env:
      FIGMA_FILE_KEY: \${{ secrets.FIGMA_FILE_KEY }}
    steps:
      - run: node library/ui/figma/key-guard.mjs
`) as Workflow
    expect(keyReaderViolations(loose)).toEqual(
      expect.arrayContaining(['scan: key outside a step env'])
    )
  })

  // A reader that passes every rule; each case below breaks exactly one.
  const PINNED = '3d3c42e5aac5ba805825da76410c181273ba90b1'
  type StepJob = Job & { steps: WorkflowStep[] }
  const reader = (change: (job: StepJob, workflow: Workflow) => void = () => {}): Workflow => {
    const job: StepJob = {
      steps: [
        {
          uses: `actions/checkout@${PINNED}`,
          with: { ref: TRUSTED_REFS[0], path: 'trusted', 'persist-credentials': false }
        },
        { run: 'git -C trusted fetch --no-tags origin "+$HEAD_SHA:refs/key-scan/head"' },
        {
          env: { FIGMA_FILE_KEY: '${{ secrets.FIGMA_FILE_KEY }}' },
          run: [
            'set -o pipefail',
            'node trusted/library/ui/figma/key-guard.mjs --root trusted | tee "$RUNNER_TEMP/out.txt"',
            `grep -qx 'check key: ran' "$RUNNER_TEMP/out.txt"`
          ].join('\n')
        }
      ]
    }
    const workflow: Workflow = {
      on: { workflow_run: { workflows: ['x'], types: ['completed'] } },
      jobs: { scan: job }
    }
    change(job, workflow)
    return workflow
  }
  const otherStep = (run: string) => reader((job) => (job.steps[1] = { run }))
  const keyStep = (run: string, extra: Partial<WorkflowStep> = {}) =>
    reader((job) => Object.assign(job.steps[2]!, { run }, extra))

  it('should accept a reader that follows every rule', () => {
    expect(keyReaderViolations(reader())).toEqual([])
    expect(keyReaderViolations(otherStep('python3 tooling/web-preview.py resolve'))).toEqual([])
  })

  it.each([
    [
      'a lower-case secret name',
      {
        on: 'workflow_run',
        jobs: { a: { steps: [{ env: { K: '${{ secrets.figma_file_key }}' } }] } }
      }
    ],
    [
      'the whole secrets context',
      { on: 'workflow_run', jobs: { a: { steps: [{ env: { ALL: '${{ toJSON(secrets) }}' } }] } } }
    ],
    [
      'an indexed secret',
      {
        on: 'workflow_run',
        jobs: { a: { steps: [{ env: { K: "${{ secrets['FIGMA_FILE_KEY'] }}" } }] } }
      }
    ]
  ] as [string, Workflow][])('should treat %s as a key reader', (_, workflow) => {
    expect(isKeyReader(workflow)).toBe(true)
  })

  it("should reject a reader that runs on deployment_status, which uses the deployed commit's workflow", () => {
    const deployed = reader((_, workflow) => (workflow.on = { deployment_status: null }))
    expect(keyReaderViolations(deployed)).toContain('triggered by deployment_status')
  })

  it('should flag a pull request job that passes every secret to a reusable workflow', () => {
    const inherited = parse(`
on: pull_request
jobs:
  call:
    uses: ./.github/workflows/reusable.yml
    secrets: inherit
`) as Workflow
    expect(isKeyReader(inherited)).toBe(true)
    expect(keyReaderViolations(inherited)).toEqual(
      expect.arrayContaining(['triggered by pull_request', 'call: calls a reusable workflow'])
    )
  })

  it.each([
    ['inherit', 'inherit'],
    ['a secrets map that names the key', { figma_file_key: '${{ secrets.OTHER }}' }]
  ] as [string, Job['secrets']][])(
    'should treat a call passing %s as a key reader',
    (_, secrets) => {
      const call: Workflow = {
        on: { workflow_run: null },
        jobs: { call: { uses: 'owner/repo/.github/workflows/x.yml@main', secrets } }
      }
      expect(isKeyReader(call)).toBe(true)
      expect(keyReaderViolations(call)).toContain('call: calls a reusable workflow')
    }
  )

  it('should not treat a reusable workflow call that passes no key as a reader, nor throw on it', () => {
    const call: Workflow = {
      on: { workflow_run: null },
      jobs: {
        call: { uses: './.github/workflows/x.yml', secrets: { OTHER: '${{ secrets.OTHER }}' } }
      }
    }
    expect(isKeyReader(call)).toBe(false)
    expect(() => keyReaderViolations(call)).not.toThrow()
  })

  it('should not treat a job that never names the key as a reader', () => {
    expect(isKeyReader(reader((job) => (job.steps = [{ run: 'true' }])))).toBe(false)
  })

  it.each([
    { on: { workflow_run: { workflows: ['x'] } } },
    { on: { push: { branches: ['main'] } } },
    { on: { push: { branches: ['main'], paths: ['library/ui/**'] }, workflow_run: null } }
  ])('should accept a reader triggered by %j', ({ on }) => {
    expect(triggerViolations({ on, jobs: {} } as Workflow)).toEqual([])
  })

  it.each([
    [{ pull_request: null }, 'triggered by pull_request'],
    [{ pull_request_target: null }, 'triggered by pull_request_target'],
    [{ workflow_dispatch: null }, 'triggered by workflow_dispatch'],
    [{ deployment_status: null }, 'triggered by deployment_status'],
    [{ deployment: null }, 'triggered by deployment'],
    ['push', 'triggered by push beyond main'],
    [{ push: null }, 'triggered by push beyond main'],
    [{ push: { branches: ['feature'] } }, 'triggered by push beyond main'],
    [{ push: { branches: ['main', 'release/*'] } }, 'triggered by push beyond main'],
    [{ push: { branches: ['main'], tags: ['v*'] } }, 'triggered by push beyond main'],
    [{ push: { 'branches-ignore': ['x'] } }, 'triggered by push beyond main']
  ] as [Workflow['on'], string][])('should reject a reader triggered by %j', (on, problem) => {
    expect(triggerViolations({ on, jobs: {} } as Workflow)).toContain(problem)
  })

  it('should reject a job that reaches the key through the whole secrets context', () => {
    const all = reader((job) => {
      job.steps = [{ env: { ALL: '${{ toJSON(secrets) }}' }, run: 'pnpm build' }]
    })
    expect(keyReaderViolations(all)).toEqual(
      expect.arrayContaining(['reads the whole secrets context', 'scan: runs pnpm'])
    )
    const indexed = reader((job) => {
      job.steps[2]!.env = { FIGMA_FILE_KEY: "${{ secrets['FIGMA_FILE_KEY'] }}" }
    })
    expect(keyReaderViolations(indexed)).toEqual(
      expect.arrayContaining(['reads the whole secrets context'])
    )
  })

  it.each([
    [
      'a lower-case key in a run',
      otherStep('echo ${{ secrets.figma_file_key }}'),
      'scan: key outside a step env'
    ],
    [
      'the key in workflow env',
      reader((_, workflow) => (workflow.env = { K: '${{ secrets.Figma_File_Key }}' })),
      'key in workflow env'
    ],
    [
      'the web-preview environment',
      reader((job) => (job.environment = 'web-preview')),
      'scan: declares environment web-preview'
    ],
    [
      'an environment in object form',
      reader((job) => (job.environment = { name: 'Web-Preview' })),
      'scan: declares environment Web-Preview'
    ],
    [
      'an environment chosen by an expression',
      reader((job) => (job.environment = '${{ inputs.target }}')),
      'scan: declares environment ${{ inputs.target }}'
    ],
    ['a job container', reader((job) => (job.container = 'node:24')), 'scan: runs in a container'],
    [
      'service containers',
      reader((job) => (job.services = { db: { image: 'postgres' } })),
      'scan: starts service containers'
    ],
    [
      'job run defaults',
      reader((job) => (job.defaults = { run: { shell: 'python {0}' } })),
      'scan: sets job defaults'
    ],
    [
      'workflow run defaults',
      reader((_, workflow) => (workflow.defaults = { run: { 'working-directory': '.preview' } })),
      'sets workflow defaults'
    ],
    [
      'a loader variable',
      reader((job) => (job.steps[2]!.env!.NODE_OPTIONS = '--require ./x.js')),
      'scan: sets NODE_OPTIONS'
    ],
    [
      'a third-party action',
      reader((job) => job.steps.push({ uses: 'someone/action@main' })),
      'scan: uses someone/action@main'
    ],
    [
      'a local action',
      reader((job) => job.steps.push({ uses: './.github/actions/x' })),
      'scan: uses ./.github/actions/x'
    ],
    [
      'a docker action',
      reader((job) => job.steps.push({ uses: 'docker://alpine:3' })),
      'scan: uses docker://alpine:3'
    ],
    [
      'a look-alike checkout',
      reader((job) => job.steps.push({ uses: `actions/checkout-pr@${PINNED}` })),
      `scan: uses actions/checkout-pr@${PINNED}`
    ],
    [
      'an action pinned by tag',
      reader((job) => job.steps.push({ uses: 'actions/setup-node@v7' })),
      'scan: uses actions/setup-node@v7'
    ],
    [
      'a key step that runs a downloaded script',
      keyStep('node "$RUNNER_TEMP/pr-artifact/x.mjs"'),
      'scan: key step runs node "$RUNNER_TEMP/pr-artifact/x.mjs"'
    ],
    [
      'a key step that runs python',
      keyStep('python3 trusted/tooling/x.py'),
      'scan: key step runs python3 trusted/tooling/x.py'
    ],
    [
      'a key step that substitutes a command',
      keyStep('node trusted/library/ui/figma/key-guard.mjs $(cat x)'),
      'scan: key step substitutes, backgrounds or reads the environment'
    ],
    [
      'a key step that reads its environment',
      keyStep('cat /proc/self/environ'),
      'scan: key step substitutes, backgrounds or reads the environment'
    ],
    [
      'a key step that redirects outside $RUNNER_TEMP',
      keyStep('node trusted/library/ui/figma/key-guard.mjs > /tmp/out'),
      'scan: key step redirects outside $RUNNER_TEMP: > /tmp/out'
    ],
    [
      'a key step that tees outside $RUNNER_TEMP',
      keyStep('node trusted/library/ui/figma/key-guard.mjs | tee out.txt'),
      'scan: key step runs tee out.txt'
    ],
    [
      'a key step that cats a workspace file',
      keyStep('cat .preview/input/x'),
      'scan: key step runs cat .preview/input/x'
    ],
    [
      'a key step with its own shell',
      keyStep('node trusted/library/ui/figma/key-guard.mjs', { shell: 'python {0}' }),
      'scan: key step sets its shell'
    ],
    [
      'a key step in another directory',
      keyStep('node library/ui/figma/key-guard.mjs', { 'working-directory': '.preview/input' }),
      'scan: key step changes its working directory'
    ],
    ['a relative script', otherStep('./script.sh'), 'scan: runs a local script'],
    ['a sourced script', otherStep('true && . script'), 'scan: runs a local script'],
    [
      'python outside trusted/ and tooling/',
      otherStep('python3 .preview/input/x.py'),
      'scan: runs python .preview/input/x.py'
    ],
    ['inline python', otherStep("python3 -c 'print(1)'"), "scan: runs python 'print(1)'"],
    ['make', otherStep('make build'), 'scan: runs make'],
    [
      'node on the downloaded artifact',
      otherStep('node "$RUNNER_TEMP/pr-artifact/x.mjs"'),
      'scan: runs a downloaded file'
    ],
    [
      'node on the preview input',
      otherStep('node .preview/input/x.mjs'),
      'scan: runs a downloaded file'
    ],
    [
      'a checkout of the scanned head',
      otherStep('git -C trusted checkout refs/key-scan/head'),
      'scan: checks out the scanned commits'
    ],
    [
      'a switch to the scanned head',
      otherStep('git -C trusted switch --detach refs/key-scan/head'),
      'scan: checks out the scanned commits'
    ],
    [
      'a worktree of the scanned head',
      otherStep('git -C trusted worktree add ../x refs/key-scan/head'),
      'scan: checks out the scanned commits'
    ],
    ['printenv', otherStep('printenv'), 'scan: prints the key'],
    ['a bare env', otherStep('env | sort'), 'scan: prints the key'],
    ['indirect expansion', otherStep('v=FIGMA_FILE_KEY; echo "${!v}"'), 'scan: prints the key'],
    ['xtrace', otherStep('set -o xtrace'), 'scan: prints the key'],
    ['a lower-case key variable', otherStep('echo "$figma_file_key"'), 'scan: prints the key']
  ] as [string, Workflow, string][])('should reject %s', (_, workflow, problem) => {
    expect(keyReaderViolations(workflow)).toContain(problem)
  })
})

describe('ui-library-key-scan.yml', () => {
  const workflow = load('ui-library-key-scan.yml')
  const allSteps = Object.values(workflow.jobs).flatMap(stepsOf)
  const runs = allSteps.flatMap((step) => (step.run ? [step.run] : []))

  it('should run only from workflow_run, with the least permissions it needs', () => {
    expect(triggers(workflow)).toEqual(['workflow_run'])
    expect(workflow.permissions).toEqual({ actions: 'read', checks: 'write', contents: 'read' })
  })

  it('should take the key from the repository secret, in one job that declares no environment', () => {
    expect(workflow.jobs['key-scan']!.environment).toBeUndefined()
    expect(Object.keys(workflow.jobs)).toEqual(['key-scan'])
  })

  it('should count the Review artifacts with a gh call gh accepts', () => {
    const check = allSteps.find((step) => step.id === 'artifact_check')!
    // gh rejects --slurp together with --jq before calling the API, which failed every artifact check.
    expect(check.run).not.toMatch(/--slurp/)
    expect(check.run).toContain('--paginate')
    expect(check.run).toMatch(/\| \.id" \| wc -l/)
    expect(check.run).toMatch(/^set -o pipefail$/m)
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
  const runs = (job: string) => stepsOf(ui().jobs[job]).map((step) => step.run ?? '')

  it('should require the port verification record on every pull request and push, before any install', () => {
    const step = stepsOf(ui().jobs['key-guard']).find((item) =>
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

  it('should hydrate only library/ui LFS objects in the library job', () => {
    const steps = stepsOf(ui().jobs.library)
    const checkout = steps.find((step) => step.uses?.startsWith('actions/checkout@'))!
    expect(checkout.with).toMatchObject({ lfs: false })
    const pulls = runs('library').filter((run) => /\blfs\b/.test(run))
    expect(pulls).toEqual(['git lfs pull --include "library/ui/**"'])
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
    const script = stepsOf(load('ui-library-key-scan.yml').jobs['key-scan'])[0]!.run!

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
    const checkout = stepsOf(job).find((step) => step.uses?.startsWith('actions/checkout@'))!
    expect(checkout.with).toMatchObject({ ref: '${{ github.sha }}', 'fetch-depth': 0 })
    expect(JSON.stringify(checkout.with)).not.toContain('pull_request.head')
    const scope = stepsOf(job).find((step) => step.run?.includes('range="$base..$HEAD_SHA"'))!
    expect(scope.env).toMatchObject({
      HEAD_SHA: '${{ github.event.pull_request.head.sha || github.sha }}',
      PR_BASE: '${{ github.event.pull_request.base.sha }}'
    })
  })

  it('should let checkout use its temporary global config while isolating the Git run steps', () => {
    const ui = load('ui-library-key-scan.yml')
    expect(ui.env?.GIT_CONFIG_GLOBAL).toBeUndefined()
    const all = stepsOf(ui.jobs['key-scan'])
    expect(all.find((step) => step.uses?.includes('actions/checkout'))!.with).toMatchObject({
      'set-safe-directory': false
    })
    for (const step of all.filter(
      (step) => step.run?.includes('git -C trusted') || step.run?.includes('node trusted/')
    ))
      expect(step.env).toMatchObject({ GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null' })
  })
})
