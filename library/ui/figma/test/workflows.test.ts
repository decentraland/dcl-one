import { spawnSync } from 'node:child_process'
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { parse } from 'yaml'
import {
  GIT_HARDENING,
  GUARD_COPY,
  KEY_JOB_ACTIONS,
  KEY_SECRET,
  KEY_STEP_TEMPLATES,
  REPORT_TEMPLATE,
  TRUSTED_SHAS
} from './key-step.js'

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
  if?: string
  uses?: string
  run?: string
  shell?: string
  'working-directory'?: string
  with?: Record<string, unknown>
  env?: Record<string, unknown>
}
type Job = {
  name?: string
  'runs-on'?: string
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
// Context names ignore case, so every spelling of a secret counts.
const namesSecret = (name: string) => new RegExp(`\\bsecrets\\s*\\.\\s*${name}\\b`, 'i')
const KEY_REFERENCE = namesSecret('FIGMA_FILE_KEY')
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
const passesSecret = (job: Job, secret: string) =>
  typeof job.secrets === 'string'
    ? job.secrets.trim().toLowerCase() === 'inherit'
    : Object.keys(job.secrets ?? {}).some((name) => name.toLowerCase() === secret.toLowerCase())
const passesKey = (job: Job) => passesSecret(job, 'FIGMA_FILE_KEY')
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
// Every guarded repository secret and the workflows that may read it, in any form: by name in any case, through
// toJSON(secrets) or secrets[...], or passed on with secrets: inherit or a secrets map. No workflow reads the
// Figma API token today, the CDN publisher included.
const GUARDED_SECRETS: Record<string, string[]> = {
  FIGMA_FILE_KEY: KEY_READERS,
  FIGMA_TOKEN: []
}
const readsSecret = (workflow: Workflow, secret: string) =>
  namesSecret(secret).test(JSON.stringify(workflow)) ||
  readsWholeSecrets(workflow) ||
  Object.values(workflow.jobs ?? {}).some((job) => passesSecret(job, secret))
function guardedSecretViolations(file: string, workflow: Workflow): string[] {
  return Object.entries(GUARDED_SECRETS).flatMap(([secret, readers]) =>
    readsSecret(workflow, secret) && !readers.includes(file) ? [`${file} reads ${secret}`] : []
  )
}
// github.workflow_sha is the commit of the running workflow file. It is trusted only because the trigger rule
// admits only workflow_run and push to main, where that file is main's copy.
const TRUSTED_REFS = ['${{ github.event.repository.default_branch }}', '${{ github.workflow_sha }}']
// Actions a key-holding job may use, each at exactly the sha KEY_JOB_ACTIONS pins.
const isTrustedAction = (uses: string) =>
  Object.entries(KEY_JOB_ACTIONS).some(([action, sha]) => uses === `${action}@${sha}`)
// The inputs each action may take in a key-holding job; anything else could change where code comes from.
const ACTION_INPUTS: Record<string, string[]> = {
  'actions/checkout': [
    'ref',
    'repository',
    'path',
    'fetch-depth',
    'lfs',
    'persist-credentials',
    'set-safe-directory',
    'sparse-checkout',
    'sparse-checkout-cone-mode'
  ],
  'actions/setup-node': ['node-version'],
  'actions/download-artifact': ['name', 'path', 'run-id', 'github-token', 'repository']
}
const THIS_REPOSITORY = '${{ github.repository }}'

// Every step a key-holding job may have: these pinned actions, the key step and, last, its report. No other step
// runs, so no shell runs before the key step and nothing it could write needs to be read.
const ACTION_STEP_KEYS = ['id', 'name', 'if', 'uses', 'with']
const RUN_STEP_KEYS = ['id', 'name', 'if', 'env', 'run']
// A key job runs on a fresh GitHub-hosted machine, so no other job leaves files where it runs.
const HOSTED_RUNNER = /^ubuntu-[\d.]+$|^ubuntu-latest$/
const isCheckout = (step: WorkflowStep) => step.uses?.startsWith('actions/checkout@') ?? false
const isDownload = (step: WorkflowStep) =>
  step.uses?.startsWith('actions/download-artifact@') ?? false
const keyStepTemplate = (step: WorkflowStep | undefined) =>
  KEY_STEP_TEMPLATES.find((template) => template.run === step?.run?.trim())
// The key step's env: the key, the pinned commit, the git hardening values and the template's data variables.
const keyStepEnv = (template: (typeof KEY_STEP_TEMPLATES)[number]): Record<string, string[]> => ({
  FIGMA_FILE_KEY: [KEY_SECRET],
  TRUSTED_SHA: TRUSTED_SHAS,
  ...Object.fromEntries(Object.entries(GIT_HARDENING).map(([name, value]) => [name, [value]])),
  ...template.data
})
// An env that holds exactly the expected names, each set to one of its allowed values.
function envViolations(
  label: string,
  env: Record<string, unknown> | undefined,
  expected: Record<string, string[]>
): string[] {
  const actual = env ?? {}
  const problems = Object.keys(actual).flatMap((name) =>
    name in expected ? [] : [`${label} env sets ${name}`]
  )
  for (const [name, values] of Object.entries(expected))
    if (!(name in actual)) problems.push(`${label} env lacks ${name}`)
    else if (!values.includes(String(actual[name])))
      problems.push(`${label} env ${name} is ${String(actual[name])}`)
  return problems
}
function actionViolations(id: string, step: WorkflowStep): string[] {
  if (!isTrustedAction(step.uses!)) return [`${id}: uses ${step.uses}`]
  const problems: string[] = []
  const action = step.uses!.split('@')[0]!
  for (const key of Object.keys(step))
    if (!ACTION_STEP_KEYS.includes(key)) problems.push(`${id}: ${action} step sets ${key}`)
  if (referencesKey(step.env)) problems.push(`${id}: key in an action step`)
  const inputs = ACTION_INPUTS[action] ?? []
  for (const input of Object.keys(step.with ?? {}))
    if (!inputs.includes(input)) problems.push(`${id}: ${action} sets ${input}`)
  const repository = step.with?.repository
  if (repository !== undefined && repository !== THIS_REPOSITORY)
    problems.push(`${id}: ${action} from ${String(repository)}`)
  if (action === 'actions/checkout') {
    const ref = String(step.with?.ref ?? '')
    if (!TRUSTED_REFS.includes(ref)) problems.push(`${id}: checkout of ${ref || 'the event ref'}`)
    if (step.with?.['persist-credentials'] !== false)
      problems.push(`${id}: checkout persists credentials`)
  }
  return problems
}
// The steps of a job that holds the key, against the template its key step runs.
function keyJobViolations(id: string, steps: WorkflowStep[]): string[] {
  const problems: string[] = []
  const keySteps = steps.flatMap((step, index) =>
    step.uses === undefined && referencesKey(step.env) ? [index] : []
  )
  if (keySteps.length > 1) problems.push(`${id}: maps the key into ${keySteps.length} steps`)
  const keyIndex = keySteps[0] ?? -1
  const template = keyStepTemplate(steps[keyIndex])
  for (const [index, step] of steps.entries()) {
    const { env, ...rest } = step
    if (referencesKey(rest)) problems.push(`${id}: key outside a step env`)
    if (step.uses !== undefined) {
      problems.push(...actionViolations(id, step))
      if (keyIndex >= 0 && index > keyIndex)
        problems.push(`${id}: runs an action after the key step`)
      continue
    }
    const extraKeys = Object.keys(step).filter((key) => !RUN_STEP_KEYS.includes(key))
    if (index === keyIndex) {
      for (const key of extraKeys) problems.push(`${id}: key step sets ${key}`)
      if (!template) problems.push(`${id}: key step does not run a key step template`)
      else problems.push(...envViolations(`${id}: key step`, env, keyStepEnv(template)))
      continue
    }
    if (
      keyIndex >= 0 &&
      index === keyIndex + 1 &&
      index === steps.length - 1 &&
      step.run?.trim() === REPORT_TEMPLATE.run
    ) {
      for (const key of extraKeys) problems.push(`${id}: report step sets ${key}`)
      problems.push(...envViolations(`${id}: report step`, env, REPORT_TEMPLATE.env))
      if (steps[keyIndex]!.id !== 'scan') problems.push(`${id}: report reads no key step id scan`)
      continue
    }
    problems.push(`${id}: runs a command beside the key`)
  }
  if (template) {
    const checkouts = steps.filter(isCheckout)
    if (checkouts.length !== 1 || String(checkouts[0]!.with?.path ?? '') !== template.checkout)
      problems.push(
        `${id}: the key step needs one checkout at ${template.checkout || 'the workspace root'}`
      )
    for (const step of steps.filter(isDownload)) {
      const target = String(step.with?.path ?? '')
      if (!template.downloads.includes(target))
        problems.push(`${id}: downloads an artifact into ${target || 'the workspace'}`)
    }
  }
  return problems
}

// A job that holds the key runs only from the default branch's workflow, declares no environment, and has only
// the steps keyJobViolations allows. Pull request files may only arrive as data (a bundle of objects or a
// downloaded artifact).
function keyReaderViolations(workflow: Workflow): string[] {
  const problems: string[] = triggerViolations(workflow)
  if (readsWholeSecrets(workflow)) problems.push('reads the whole secrets context')
  const { jobs, ...top } = workflow
  if (referencesKey(top)) problems.push('key in workflow env')
  if (top.defaults !== undefined) problems.push('sets workflow defaults')
  // Workflow and key job env reach the key step, whose env is an exact template, so neither may set any.
  if (Object.keys(workflow.env ?? {}).length > 0) problems.push('sets workflow env')
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
    if (job.uses === undefined && !HOSTED_RUNNER.test(String(job['runs-on'] ?? '')))
      problems.push(`${id}: runs on ${JSON.stringify(job['runs-on'] ?? null)}`)
    if (Object.keys(job.env ?? {}).length > 0) problems.push(`${id}: sets job env`)
    problems.push(...keyJobViolations(id, jobSteps))
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

  it.each(workflowFiles)('should let %s read only the guarded secrets it is allowed', (file) => {
    expect(guardedSecretViolations(file, load(file))).toEqual([])
  })

  it.each([
    ['by name', { FIGMA_TOKEN: '${{ secrets.FIGMA_TOKEN }}' }],
    ['in another case', { T: '${{ secrets.figma_token }}' }],
    ['with spaces', { T: '${{ secrets . Figma_Token }}' }],
    ['through toJSON(secrets)', { ALL: '${{ toJSON(secrets) }}' }],
    ['by index', { T: "${{ secrets['FIGMA_TOKEN'] }}" }]
  ])('should flag the key scan itself reading FIGMA_TOKEN %s', (_, env) => {
    const scan = load('ui-library-key-scan.yml')
    stepsOf(scan.jobs['key-scan']).push({ env, run: 'true' })
    expect(guardedSecretViolations('ui-library-key-scan.yml', scan)).toContain(
      'ui-library-key-scan.yml reads FIGMA_TOKEN'
    )
  })

  it.each([
    ['secrets: inherit', 'inherit'],
    ['a secrets map that names the token', { figma_token: '${{ secrets.OTHER }}' }]
  ] as [string, Job['secrets']][])(
    'should flag a call passing FIGMA_TOKEN through %s',
    (_, secrets) => {
      const call: Workflow = {
        on: { workflow_run: null },
        jobs: { call: { uses: './.github/workflows/x.yml', secrets } }
      }
      expect(guardedSecretViolations('web-previews-publish.yml', call)).toContain(
        'web-previews-publish.yml reads FIGMA_TOKEN'
      )
    }
  )

  it('should not flag a workflow that reads neither guarded secret', () => {
    const other: Workflow = {
      on: { push: { branches: ['main'] } },
      jobs: { a: { steps: [{ env: { T: '${{ secrets.FIGMA_TOKEN_OLD }}' }, run: 'true' }] } }
    }
    expect(guardedSecretViolations('x.yml', other)).toEqual([])
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
        'test: uses actions/checkout@v7',
        'test: declares environment web-preview',
        'test: key step does not run a key step template'
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
      expect.arrayContaining([
        'scan: key outside a step env',
        'scan: runs a command beside the key'
      ])
    )
  })

  // Readers that pass every rule, one per key step template; each case below breaks exactly one rule.
  const PINNED = '3d3c42e5aac5ba805825da76410c181273ba90b1'
  const SETUP_NODE = `actions/setup-node@${KEY_JOB_ACTIONS['actions/setup-node']}`
  const DOWNLOAD = `actions/download-artifact@${KEY_JOB_ACTIONS['actions/download-artifact']}`
  const [SCAN, ROOT_SCAN] = KEY_STEP_TEMPLATES as [
    (typeof KEY_STEP_TEMPLATES)[number],
    (typeof KEY_STEP_TEMPLATES)[number]
  ]
  const first = (values: Record<string, string[]>) =>
    Object.fromEntries(Object.entries(values).map(([name, [value]]) => [name, value!]))
  const keyEnv = (template: typeof SCAN): Record<string, unknown> => ({
    FIGMA_FILE_KEY: KEY_SECRET,
    TRUSTED_SHA: TRUSTED_SHAS[0],
    ...GIT_HARDENING,
    ...first(template.data)
  })
  type StepJob = Job & { steps: WorkflowStep[] }
  // The key step's index in both readers.
  const KEY = 3
  const reader = (change: (job: StepJob, workflow: Workflow) => void = () => {}): Workflow => {
    const job: StepJob = {
      'runs-on': 'ubuntu-26.04',
      steps: [
        {
          uses: `actions/checkout@${PINNED}`,
          with: { ref: TRUSTED_REFS[0], path: 'trusted', 'persist-credentials': false }
        },
        { uses: SETUP_NODE, with: { 'node-version': 24 } },
        {
          uses: DOWNLOAD,
          with: { name: 'key-scan-input', path: SCAN.downloads[0] }
        },
        { id: 'scan', env: keyEnv(SCAN), run: SCAN.run },
        { if: 'always()', env: first(REPORT_TEMPLATE.env), run: REPORT_TEMPLATE.run }
      ]
    }
    const workflow: Workflow = {
      on: { workflow_run: { workflows: ['x'], types: ['completed'] } },
      jobs: { scan: job }
    }
    change(job, workflow)
    return workflow
  }
  // The CDN publisher's form: a workspace root checkout and the Review build in a dot directory.
  const rootReader = (change: (job: StepJob) => void = () => {}) =>
    reader((job) => {
      job.steps = [
        {
          uses: `actions/checkout@${PINNED}`,
          with: { ref: TRUSTED_REFS[1], 'persist-credentials': false }
        },
        { uses: SETUP_NODE, with: { 'node-version': 24 } },
        {
          uses: DOWNLOAD,
          with: { name: 'web-preview-reviewer', path: ROOT_SCAN.downloads[0] }
        },
        { env: keyEnv(ROOT_SCAN), run: ROOT_SCAN.run }
      ]
      change(job)
    })
  // A run step before the key step, where every earlier attack lived.
  const otherStep = (run: string) => reader((job) => job.steps.splice(KEY, 0, { run }))
  const rootCheckout = (run: string) => rootReader((job) => job.steps.splice(KEY, 0, { run }))
  const keyStep = (run: string, extra: Partial<WorkflowStep> = {}) =>
    reader((job) => Object.assign(job.steps[KEY]!, { run }, extra))

  it('should accept a reader for each key step template', () => {
    expect(keyReaderViolations(reader())).toEqual([])
    expect(keyReaderViolations(rootReader())).toEqual([])
    // The report step is optional, and the template's text may carry surrounding whitespace.
    expect(keyReaderViolations(reader((job) => job.steps.pop()))).toEqual([])
    expect(keyReaderViolations(reader((job) => (job.steps[KEY]!.run = `\n${SCAN.run}\n`)))).toEqual(
      []
    )
  })

  // The two key step forms of #548, which the policy used to read as shell, as their old helpers spelled them.
  const OLD_COPY =
    'git -C trusted --no-replace-objects cat-file blob "$TRUSTED_SHA:library/ui/figma/key-guard.mjs" > "$RUNNER_TEMP/key-guard.mjs"'
  const OLD_NODE = 'node "$RUNNER_TEMP/key-guard.mjs"'
  const oldGuardRun = (args: string) =>
    [OLD_COPY, `${OLD_NODE} --root trusted ${args}`.trim()].join('\n')
  const COPY_HEAD = 'cp "$RUNNER_TEMP/h" trusted/.git/HEAD'

  // Every command the shell parser had to judge before the key step: the writes, rewrites, downloads, inline code
  // and loader tricks reported against it, the forms that slipped past it (& inside quotes, env -S, find -exec,
  // awk system and print >, patch, truncate, git -c protocol.ext) and the harmless ones it let through. None can
  // run now: a key job has no run step but the key step and its report.
  it.each([
    'python3 tooling/web-preview.py resolve',
    'mkdir -p .preview && cp x .preview/y',
    './script.sh',
    'true && . script',
    'python3 .preview/input/x.py',
    "python3 -c 'print(1)'",
    'make build',
    'node "$RUNNER_TEMP/pr-artifact/x.mjs"',
    'node .preview/input/x.mjs',
    'git -C trusted checkout refs/key-scan/head',
    'git -C trusted switch --detach refs/key-scan/head',
    'git -C trusted worktree add ../x refs/key-scan/head',
    'printenv',
    'env | sort',
    'v=FIGMA_FILE_KEY; echo "${!v}"',
    'set -o xtrace',
    'echo "$figma_file_key"',
    'cp "$RUNNER_TEMP/pr-artifact/x.mjs" trusted/library/ui/figma/key-guard.mjs',
    'mv "$RUNNER_TEMP/x" trusted/library/ui/figma/key-guard.mjs',
    'install -m 644 "$RUNNER_TEMP/x" trusted/library/ui/figma/key-guard.mjs',
    'rsync -a "$RUNNER_TEMP/pr-artifact/" trusted/',
    'ln -sf "$RUNNER_TEMP/x" trusted/library/ui/figma/key-guard.mjs',
    'tar -xf "$RUNNER_TEMP/x.tar" -C trusted',
    'tar xf "$RUNNER_TEMP/x.tar"',
    'unzip -o "$RUNNER_TEMP/x.zip"',
    'unzip -o "$RUNNER_TEMP/x.zip" -d library',
    'dd if="$RUNNER_TEMP/x" of=trusted/library/ui/figma/key-guard.mjs',
    "sed -i 's/a/b/' trusted/library/ui/figma/key-guard.mjs",
    'tee library/ui/figma/key-guard.mjs',
    'echo x > trusted/library/ui/figma/key-guard.mjs',
    'echo x >> "trusted/library/ui/figma/key-guard.mjs"',
    'cp x "$GITHUB_WORKSPACE/trusted/library/ui/figma/key-guard.mjs"',
    'cp x "$RUNNER_TEMP/../work/trusted/key-guard.mjs"',
    'cp x /home/runner/work/repo/repo/trusted/key-guard.mjs',
    'cp "$RUNNER_TEMP/x" library/ui/figma/key-guard.mjs',
    'echo x > tooling/web-preview.py',
    "sed -i 's/a/b/' ./tooling/web-preview.py",
    'cp -r "$RUNNER_TEMP/pr-artifact/." .',
    'mv trusted/library/ui/figma/key-guard.mjs "$RUNNER_TEMP/x"',
    'git -C trusted replace abc def',
    'git -C trusted fetch origin "+$HEAD_SHA:refs/replace/abc"',
    'git -C trusted update-ref refs/heads/main "$HEAD_SHA"',
    'git -C trusted symbolic-ref HEAD refs/key-scan/head',
    'git -C trusted config core.hooksPath x',
    'echo x > .git/refs/replace/abc',
    'cp "$RUNNER_TEMP/h" .git/HEAD',
    'cp "$RUNNER_TEMP/x" ./.github/workflows/x.yml',
    'cp "$RUNNER_TEMP/h" trusted/.git/HEAD',
    'tar --extract -f "$RUNNER_TEMP/x.tar" --directory trusted',
    'tar -xf "$RUNNER_TEMP/x.tar" --directory=trusted',
    'tar -xf "$RUNNER_TEMP/x.tar" -Ctrusted',
    'unzip -o "$RUNNER_TEMP/x.zip" -dtrusted',
    'ln -s "$PWD/trusted/.git" "$RUNNER_TEMP/g"',
    'cp --target-directory=trusted/.git "$RUNNER_TEMP/HEAD"',
    'cp -ttrusted/.git "$RUNNER_TEMP/HEAD"',
    'mv -t trusted "$RUNNER_TEMP/x"',
    'cp "$RUNNER_TEMP/x" "$HOME/.local/bin/node"',
    'cp "$RUNNER_TEMP/x" ~/.local/bin/node',
    'echo x > "$TOOLS/node"',
    'sudo cp "$RUNNER_TEMP/x" /usr/local/bin/node',
    'cd trusted/.git && cp "$RUNNER_TEMP/x" HEAD',
    'pushd trusted',
    'popd',
    'git -C trusted fetch origin "$HEAD_SHA" && git -C trusted reset --hard FETCH_HEAD',
    'git -C trusted checkout "$HEAD_SHA"',
    'git -C trusted read-tree -u --reset "$HEAD_SHA"',
    'git -C trusted fetch --update-head-ok origin "+$HEAD_SHA:main"',
    'echo "NODE_OPTIONS=--require ./x.js" >> "$GITHUB_ENV"',
    'echo "$RUNNER_TEMP/pr-artifact" >> $GITHUB_PATH',
    `${COPY_HEAD} 2>/dev/null`,
    `${COPY_HEAD} 2> /dev/null`,
    `${COPY_HEAD} > "$RUNNER_TEMP/log" 2>&1`,
    `${COPY_HEAD} &>/dev/null`,
    `${COPY_HEAD} < /dev/null`,
    `${COPY_HEAD} &`,
    `${COPY_HEAD} & wait`,
    `( ${COPY_HEAD} )`,
    `(${COPY_HEAD})`,
    `{ ${COPY_HEAD}; }`,
    `! ${COPY_HEAD}`,
    `timeout 10 ${COPY_HEAD}`,
    `timeout -s KILL --kill-after 5 10s ${COPY_HEAD}`,
    `env ${COPY_HEAD}`,
    `env -i -u NODE_OPTIONS A=1 ${COPY_HEAD}`,
    `time ${COPY_HEAD}`,
    `time -p ${COPY_HEAD}`,
    `nice -n 5 ${COPY_HEAD}`,
    `sudo -E ${COPY_HEAD}`,
    `A=1 command ${COPY_HEAD}`,
    'env -C trusted cp "$RUNNER_TEMP/h" .git/HEAD',
    'env --chdir=trusted/.git cp "$RUNNER_TEMP/h" HEAD',
    'echo x &>trusted/.git/HEAD',
    'echo x &>> trusted/.git/HEAD',
    'echo x >| trusted/.git/HEAD',
    'echo x >&trusted/.git/HEAD',
    'cat <>trusted/.git/HEAD',
    'curl -sSfLo trusted/.git/config https://example.com/c',
    'curl --output trusted/.git/config https://example.com/c',
    'curl --output=trusted/.git/config https://example.com/c',
    'curl -otrusted/.git/config https://example.com/c',
    'curl -O --output-dir trusted/.git https://example.com/config',
    'curl -sSLO https://example.com/config',
    'wget -O trusted/.git/config https://example.com/c',
    'wget -P trusted/.git https://example.com/config',
    'wget https://example.com/config',
    'gh run download 1 -D trusted/.git/x',
    'gh run download 1 --dir=trusted',
    'gh run download 1 -n x',
    'gh release download v1 -O trusted/.git/config',
    `node -e "require('fs').writeFileSync('trusted/.git/config', '')"`,
    `node --eval "x"`,
    `node -p "x"`,
    `timeout 5 node -e "x"`,
    `node - < "$RUNNER_TEMP/x.js"`,
    `perl -pi -e 's/a/b/' x`,
    `ruby -e 'x'`,
    'git -C trusted lfs install --local',
    'git -C trusted lfs uninstall',
    `git -C trusted fetch origin '+refs/*:refs/*'`,
    'git -C trusted fetch origin "refs/r*:refs/r*"',
    'git -C trusted fetch --no-tags origin +refs/*:refs/*',
    `git -C trusted fetch origin '*:*'`,
    'unzip -l "$RUNNER_TEMP/x.zip"',
    'unzip -p "$RUNNER_TEMP/x.zip" a > "$RUNNER_TEMP/a"',
    'echo "a=b" >> "${GITHUB_OUTPUT}"',
    'cp trusted/x "$RUNNER_TEMP/x" 2>/dev/null',
    'cp trusted/x "$RUNNER_TEMP/x" 2>&1 >&2',
    'timeout 10 cp trusted/x "$RUNNER_TEMP/y" &',
    '( cp trusted/x "$RUNNER_TEMP/y" )',
    'curl -sSfo "$RUNNER_TEMP/x" https://example.com/x',
    'curl -sSf https://example.com/x > "$RUNNER_TEMP/x"',
    'wget -qO "$RUNNER_TEMP/x" https://example.com/x',
    'gh run download 1 -D "$RUNNER_TEMP/x"',
    'gh run view 1 > "$RUNNER_TEMP/x"',
    'env -C "$RUNNER_TEMP" cp a b',
    'node trusted/library/ui/figma/lfs-prefetch.mjs --root trusted',
    'git -C trusted fetch --no-tags origin "+$HEAD_SHA:refs/key-scan/head" "+refs/heads/*:refs/remotes/origin/*"',
    'tar -czf "$RUNNER_TEMP/a.tgz" index.html',
    'tar cf "$RUNNER_TEMP/a.tar" xfiles',
    'tar -xf "$RUNNER_TEMP/x.tar" --directory="$RUNNER_TEMP/out"',
    'unzip -o "$RUNNER_TEMP/x.zip" -d "$RUNNER_TEMP/out"',
    'git -C trusted --work-tree="$RUNNER_TEMP/pr-artifact" add library/ui/figma/key-guard.mjs',
    'git -C trusted -c user.name=x -c user.email=x@example.com commit -m x',
    'echo "a&b" > trusted/.git/HEAD',
    `env -S '${COPY_HEAD}'`,
    'find trusted -name key-guard.mjs -exec cp "$RUNNER_TEMP/x" {} \\;',
    `awk 'BEGIN { system("${COPY_HEAD.replace(/"/g, '')}") }'`,
    `awk 'BEGIN { print "x" > "trusted/.git/HEAD" }'`,
    'patch -p1 -d trusted < "$RUNNER_TEMP/x.patch"',
    'truncate -s 0 trusted/library/ui/figma/key-guard.mjs',
    `git -C trusted -c protocol.ext.allow=always fetch 'ext::sh -c touch% trusted/x' main`,
    'true'
  ])('should reject %s beside the key', (run) => {
    expect(keyReaderViolations(otherStep(run))).toContain('scan: runs a command beside the key')
    expect(keyReaderViolations(rootCheckout(run))).toContain('scan: runs a command beside the key')
  })

  it('should reject a run step with its own working directory or env beside the key', () => {
    const moved = reader((job) =>
      job.steps.splice(KEY, 0, {
        run: 'cp "$RUNNER_TEMP/x" HEAD',
        'working-directory': 'trusted/.git'
      })
    )
    expect(keyReaderViolations(moved)).toContain('scan: runs a command beside the key')
    const hardened = reader((job) =>
      job.steps.splice(KEY, 0, { run: 'true', env: { ...GIT_HARDENING } })
    )
    expect(keyReaderViolations(hardened)).toContain('scan: runs a command beside the key')
  })

  // Every key step the shell parser had to judge: other programs, substitutions, redirects, rewrites of the guard
  // copy in any spelling, copies from another revision or with replace refs on, allowlists or revisions from the
  // scanned head, and options built from variables or ANSI-C quoting. Each differs from the template text.
  it.each([
    [
      OLD_COPY,
      `${OLD_NODE} --root trusted | tee "$RUNNER_TEMP/o.txt" && grep -qx 'check key: ran' "$RUNNER_TEMP/o.txt"`
    ].join('\n'),
    'node "$RUNNER_TEMP/pr-artifact/x.mjs"',
    'python3 trusted/tooling/x.py',
    'node trusted/library/ui/figma/key-guard.mjs $(cat x)',
    'cat /proc/self/environ',
    'node trusted/library/ui/figma/key-guard.mjs > /tmp/out',
    'node trusted/library/ui/figma/key-guard.mjs | tee out.txt',
    'cat .preview/input/x',
    [OLD_COPY, `${OLD_NODE} --root trusted ; node -e "x"`].join('\n'),
    [OLD_COPY, `${OLD_NODE} --root trusted && node -e "x"`].join('\n'),
    [OLD_COPY, `${OLD_NODE} --root trusted || node -e "x"`].join('\n'),
    [OLD_COPY, `${OLD_NODE} --root trusted | node -e "x"`].join('\n'),
    [
      OLD_COPY,
      'if node -e "x"; then grep -q \'a\' "$RUNNER_TEMP/o.txt"; fi',
      `${OLD_NODE} --root trusted`
    ].join('\n'),
    [OLD_COPY, `${OLD_NODE} --root trusted`, '( node -e "x" )'].join('\n'),
    [OLD_COPY, `${OLD_NODE} --root trusted`, 'python3 -c "x"'].join('\n'),
    [OLD_COPY, `${OLD_NODE} --root trusted`, 'sh -c "x"'].join('\n'),
    [OLD_COPY, `${OLD_NODE} --root trusted`, 'eval "x"'].join('\n'),
    [
      OLD_COPY,
      'head -c 999999 "$RUNNER_TEMP/pr-artifact/x.mjs" > "$RUNNER_TEMP/./key-guard.mjs"',
      `${OLD_NODE} --root trusted`
    ].join('\n'),
    [
      OLD_COPY,
      'grep -h \'\' "$RUNNER_TEMP/pr-artifact/x.mjs" >> "$RUNNER_TEMP//key-guard.mjs"',
      `${OLD_NODE} --root trusted`
    ].join('\n'),
    [
      OLD_COPY,
      'cat "$RUNNER_TEMP/x" > "${RUNNER_TEMP}/key-guard.mjs"',
      `${OLD_NODE} --root trusted`
    ].join('\n'),
    [
      OLD_COPY,
      'cat "$RUNNER_TEMP/x" | tee -a "$RUNNER_TEMP/./key-guard.mjs"',
      `${OLD_NODE} --root trusted`
    ].join('\n'),
    [
      OLD_COPY,
      'cat "$RUNNER_TEMP/x" > "$RUNNER_TEMP/key-guard.m"*',
      `${OLD_NODE} --root trusted`
    ].join('\n'),
    [
      OLD_COPY,
      String.raw`cat "$RUNNER_TEMP/x.mjs" > $RUNNER_TEMP/key\-guard.mjs`,
      `${OLD_NODE} --root trusted`
    ].join('\n'),
    [
      OLD_COPY,
      'cat "$RUNNER_TEMP/x.mjs" > "$RUNNER_TEMP/$NAME"',
      `${OLD_NODE} --root trusted`
    ].join('\n'),
    [OLD_COPY.replace(' --no-replace-objects', ''), `${OLD_NODE} --root trusted`].join('\n'),
    [OLD_COPY.replace('$TRUSTED_SHA:', 'HEAD:'), `${OLD_NODE} --root trusted`].join('\n'),
    ['TRUSTED_SHA="$HEAD_SHA"', OLD_COPY, `${OLD_NODE} --root trusted`].join('\n'),
    'node trusted/library/ui/figma/key-guard.mjs --root trusted',
    [`${OLD_NODE}`, OLD_COPY].join('\n'),
    [OLD_COPY, 'cat "$RUNNER_TEMP/x" > "$RUNNER_TEMP/key-guard.mjs"', `${OLD_NODE}`].join('\n'),
    [OLD_COPY.replace('$TRUSTED_SHA:', 'refs/key-scan/head:'), `${OLD_NODE}`].join('\n'),
    [OLD_COPY, `${OLD_NODE}`, 'node "$RUNNER_TEMP/pr-artifact/x.mjs"'].join('\n'),
    'node trusted/library/ui/figma/key-guard.mjs --b ${{ github.event.workflow_run.head_branch }}',
    'node trusted/library/ui/figma/key-guard.mjs | tee "$RUNNER_TEMP/../work/x"',
    'PATH="$RUNNER_TEMP/pr-artifact:$PATH"\nnode trusted/library/ui/figma/key-guard.mjs',
    [
      'extra=(--allowlist-from "$HEAD_SHA")',
      OLD_COPY,
      `${OLD_NODE} --root trusted "\${extra[@]}"`
    ].join('\n'),
    ...[
      '--allowlist-from "$TRUSTED_SHA"',
      '--allowlist-from "${TRUSTED_SHA}" --revision "$TRUSTED_SHA"',
      '--revision $TRUSTED_SHA',
      '',
      '--allowlist-from "$HEAD_SHA"',
      '--allowlist-from HEAD',
      '--allowlist-from "$BASE"',
      '--allowlist-"from" "$HEAD_SHA"',
      '--allowlist-from',
      '--revision "$HEAD_SHA"',
      '"--$OPTION" "$HEAD_SHA"'
    ].map(oldGuardRun),
    // The template itself, changed by one word.
    SCAN.run.replace('--allowlist-from "$TRUSTED_SHA"', '--allowlist-from "$HEAD_SHA"'),
    SCAN.run.replace(
      '--allowlist-from "$TRUSTED_SHA"',
      String.raw`$'\055\055allowlist-from' "$HEAD_SHA"`
    ),
    SCAN.run.replace('--revision "$TRUSTED_SHA"', '--revision "$HEAD_SHA"'),
    SCAN.run.replace('"$TRUSTED_SHA:library', '"HEAD:library'),
    SCAN.run.replace(' --no-replace-objects', ''),
    SCAN.run.replace('"+refs/key-scan/*:refs/key-scan/*"', '"+refs/*:refs/*"'),
    SCAN.run.replace('--range "$range"', '--range "$range" --path .'),
    `${SCAN.run}\nnode "$RUNNER_TEMP/pr-artifact/x.mjs"`,
    `node -e "x"\n${SCAN.run}`,
    SCAN.run.replace(
      'set -o pipefail',
      'set -o pipefail; cp "$RUNNER_TEMP/x" "$RUNNER_TEMP/key-guard.mjs"'
    )
  ])('should reject the key step %#, which is no key step template', (run) => {
    expect(keyReaderViolations(keyStep(run))).toContain(
      'scan: key step does not run a key step template'
    )
  })

  it.each([
    ['shell', { shell: 'python {0}' }],
    ['working-directory', { 'working-directory': '.preview/input' }],
    ['continue-on-error', { 'continue-on-error': true }]
  ] as [string, Partial<WorkflowStep>][])('should reject a key step that sets %s', (key, extra) => {
    expect(keyReaderViolations(keyStep(SCAN.run, extra))).toContain(`scan: key step sets ${key}`)
  })

  // The key step's env holds exactly the key, the pinned commit, the three hardening values and the template's data
  // variables, each from its allowlisted expression.
  it.each([
    'GIT_NO_REPLACE_OBJECTS',
    'GIT_CONFIG_NOSYSTEM',
    'GIT_CONFIG_GLOBAL',
    'TRUSTED_SHA',
    'BASE'
  ])('should reject a key step env without %s', (name) => {
    const dropped = reader((job) => delete job.steps[KEY]!.env![name])
    expect(keyReaderViolations(dropped)).toContain(`scan: key step env lacks ${name}`)
  })

  it.each([
    ['GIT_NO_REPLACE_OBJECTS', '0'],
    ['GIT_CONFIG_NOSYSTEM', ''],
    ['GIT_CONFIG_GLOBAL', '$RUNNER_TEMP/pr-artifact/c'],
    ['TRUSTED_SHA', '${{ steps.trigger.outputs.head }}'],
    ['TRUSTED_SHA', '${{ github.event.workflow_run.head_sha }}'],
    ['HEAD_SHA', '${{ github.event.workflow_run.head_sha }}'],
    ['BASE', '${{ github.event.workflow_run.head_branch }}'],
    ['ARTIFACT_REQUIRED', 'true'],
    ['FIGMA_FILE_KEY', '${{ secrets.FIGMA_FILE_KEY || github.event.workflow_run.head_branch }}']
  ])('should reject a key step env that sets %s to %j', (name, value) => {
    const changed = reader((job) => (job.steps[KEY]!.env![name] = value))
    expect(keyReaderViolations(changed)).toContain(`scan: key step env ${name} is ${value}`)
  })

  it.each([
    'NODE_OPTIONS',
    'PATH',
    'HOME',
    'XDG_CONFIG_HOME',
    'GIT_DIR',
    'GIT_EXEC_PATH',
    'GIT_CONFIG_SYSTEM',
    'GIT_CONFIG_PARAMETERS',
    'GIT_CONFIG_COUNT',
    'GIT_CONFIG_KEY_0',
    'GIT_CONFIG_VALUE_0',
    'GIT_CONFIG',
    'OPTION'
  ])('should reject %s on the key step, and any env on the job or workflow', (name) => {
    const value = '$RUNNER_TEMP/pr-artifact/c'
    expect(keyReaderViolations(reader((job) => (job.steps[KEY]!.env![name] = value)))).toContain(
      `scan: key step env sets ${name}`
    )
    if (name === 'OPTION') return
    expect(keyReaderViolations(reader((job) => (job.env = { [name]: value })))).toContain(
      'scan: sets job env'
    )
    expect(
      keyReaderViolations(reader((_, workflow) => (workflow.env = { [name]: value })))
    ).toContain('sets workflow env')
  })

  it('should reject job or workflow env that no loader list names, even with the git hardening values', () => {
    for (const env of [{ NODE_EXTRA_CA_CERTS: '/tmp/ca.pem' }, { ...GIT_HARDENING }]) {
      expect(keyReaderViolations(reader((job) => (job.env = env)))).toContain('scan: sets job env')
      expect(keyReaderViolations(reader((_, workflow) => (workflow.env = env)))).toContain(
        'sets workflow env'
      )
    }
  })

  it.each([
    ['a third-party action', { uses: 'someone/action@main' }, 'scan: uses someone/action@main'],
    ['a local action', { uses: './.github/actions/x' }, 'scan: uses ./.github/actions/x'],
    ['a docker action', { uses: 'docker://alpine:3' }, 'scan: uses docker://alpine:3'],
    [
      'a look-alike checkout',
      { uses: `actions/checkout-pr@${PINNED}` },
      `scan: uses actions/checkout-pr@${PINNED}`
    ],
    [
      'a trusted action at a sha it is not pinned to',
      { uses: `actions/checkout@${'f'.repeat(40)}` },
      `scan: uses actions/checkout@${'f'.repeat(40)}`
    ],
    [
      'an action pinned by tag',
      { uses: 'actions/setup-node@v7' },
      'scan: uses actions/setup-node@v7'
    ],
    [
      'the publisher action',
      { uses: `decentraland/oddish-action@${PINNED}` },
      `scan: uses decentraland/oddish-action@${PINNED}`
    ],
    [
      'setup-node with a cache',
      { uses: SETUP_NODE, with: { 'node-version': 24, cache: 'npm' } },
      'scan: actions/setup-node sets cache'
    ],
    [
      'an action step with an env',
      { uses: SETUP_NODE, env: { NODE_OPTIONS: '--require ./x.js' } },
      'scan: actions/setup-node step sets env'
    ],
    [
      'an action step that maps the key',
      { uses: SETUP_NODE, env: { K: '${{ secrets.FIGMA_FILE_KEY }}' } },
      'scan: key in an action step'
    ],
    [
      'an artifact over the trusted checkout',
      {
        uses: DOWNLOAD,
        with: { name: 'x', path: 'trusted/library/ui/figma' }
      },
      'scan: downloads an artifact into trusted/library/ui/figma'
    ],
    [
      'an artifact into the workspace root',
      { uses: DOWNLOAD, with: { name: 'x' } },
      'scan: downloads an artifact into the workspace'
    ],
    [
      'an artifact into .github',
      { uses: DOWNLOAD, with: { name: 'x', path: '.github/actions' } },
      'scan: downloads an artifact into .github/actions'
    ],
    [
      'an artifact out of runner.temp',
      {
        uses: DOWNLOAD,
        with: { name: 'x', path: '${{ runner.temp }}/../work/trusted' }
      },
      'scan: downloads an artifact into ${{ runner.temp }}/../work/trusted'
    ],
    [
      'an artifact from another repository',
      {
        uses: DOWNLOAD,
        with: { name: 'x', path: '${{ runner.temp }}/pr-artifact', repository: 'outside/fork' }
      },
      'scan: actions/download-artifact from outside/fork'
    ]
  ] as [string, WorkflowStep, string][])('should reject %s in a key job', (_, step, problem) => {
    expect(keyReaderViolations(reader((job) => job.steps.splice(KEY, 0, step)))).toContain(problem)
  })

  it.each([
    [
      'a checkout of another repository',
      reader(
        (job) =>
          (job.steps[0]!.with!.repository =
            '${{ github.event.workflow_run.head_repository.full_name }}')
      ),
      'scan: actions/checkout from ${{ github.event.workflow_run.head_repository.full_name }}'
    ],
    [
      'a checkout of the scanned head',
      reader((job) => (job.steps[0]!.with!.ref = '${{ github.event.workflow_run.head_sha }}')),
      'scan: checkout of ${{ github.event.workflow_run.head_sha }}'
    ],
    [
      'a checkout with its own token',
      reader((job) => (job.steps[0]!.with!.token = '${{ secrets.OTHER }}')),
      'scan: actions/checkout sets token'
    ],
    [
      'a checkout from another server',
      reader((job) => (job.steps[0]!.with!['github-server-url'] = 'https://example.com')),
      'scan: actions/checkout sets github-server-url'
    ],
    [
      'a checkout that persists credentials',
      reader((job) => delete job.steps[0]!.with!['persist-credentials']),
      'scan: checkout persists credentials'
    ],
    [
      'a checkout into a parent path',
      reader((job) => (job.steps[0]!.with!.path = '../trusted')),
      'scan: the key step needs one checkout at trusted'
    ],
    [
      'a second checkout',
      reader((job) =>
        job.steps.splice(1, 0, { ...job.steps[0]!, with: { ...job.steps[0]!.with, path: 'other' } })
      ),
      'scan: the key step needs one checkout at trusted'
    ],
    [
      'a root checkout under the trusted/ template',
      reader((job) => delete job.steps[0]!.with!.path),
      'scan: the key step needs one checkout at trusted'
    ],
    [
      'a trusted/ checkout under the root template',
      rootReader((job) => (job.steps[0]!.with!.path = 'trusted')),
      'scan: the key step needs one checkout at the workspace root'
    ],
    [
      'the root template downloading into runner.temp',
      rootReader((job) => (job.steps[2]!.with!.path = SCAN.downloads[1])),
      `scan: downloads an artifact into ${SCAN.downloads[1]}`
    ],
    [
      'an action after the key step',
      reader((job) => job.steps.push({ uses: SETUP_NODE, with: { 'node-version': 24 } })),
      'scan: runs an action after the key step'
    ],
    [
      'a second key step',
      reader((job) => job.steps.splice(KEY, 0, { ...job.steps[KEY]! })),
      'scan: maps the key into 2 steps'
    ],
    [
      'a changed report',
      reader((job) => (job.steps[KEY + 1]!.run = `${REPORT_TEMPLATE.run}\ncat "$RUNNER_TEMP/x"`)),
      'scan: runs a command beside the key'
    ],
    [
      'a report that is not last',
      reader((job) => job.steps.push({ ...job.steps[KEY + 1]! })),
      'scan: runs a command beside the key'
    ],
    [
      'a report with another token',
      reader((job) => (job.steps[KEY + 1]!.env!.GH_TOKEN = '${{ secrets.OTHER }}')),
      'scan: report step env GH_TOKEN is ${{ secrets.OTHER }}'
    ],
    [
      'a report that reads the key',
      reader((job) => (job.steps[KEY + 1]!.env!.K = '${{ secrets.FIGMA_FILE_KEY }}')),
      'scan: maps the key into 2 steps'
    ],
    [
      'a report after a key step not named scan',
      reader((job) => (job.steps[KEY]!.id = 'other')),
      'scan: report reads no key step id scan'
    ],
    [
      'a self-hosted runner',
      reader((job) => (job['runs-on'] = 'self-hosted')),
      'scan: runs on "self-hosted"'
    ],
    [
      'a lower-case key in a run',
      otherStep('echo ${{ secrets.figma_file_key }}'),
      'scan: key outside a step env'
    ],
    [
      'a spaced secret reference',
      otherStep('echo ${{ secrets . FIGMA_FILE_KEY }}'),
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
    ]
  ] as [string, Workflow, string][])('should reject %s', (_, workflow, problem) => {
    expect(keyReaderViolations(workflow)).toContain(problem)
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
      'a spaced secret name',
      {
        on: 'workflow_run',
        jobs: { a: { steps: [{ env: { K: '${{ secrets . FIGMA_FILE_KEY }}' } }] } }
      }
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

  it('should leave the jobs of a reader workflow that hold no key to their own rules', () => {
    const split = reader((_, workflow) => {
      workflow.jobs.prepare = {
        'runs-on': 'ubuntu-26.04',
        steps: [{ run: 'cp x trusted/.git/HEAD' }]
      }
    })
    expect(keyReaderViolations(split)).toEqual([])
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
      expect.arrayContaining([
        'reads the whole secrets context',
        'scan: key step does not run a key step template'
      ])
    )
    const indexed = reader((job) => {
      job.steps[KEY]!.env!.FIGMA_FILE_KEY = "${{ secrets['FIGMA_FILE_KEY'] }}"
    })
    expect(keyReaderViolations(indexed)).toEqual(
      expect.arrayContaining([
        'reads the whole secrets context',
        `scan: key step env FIGMA_FILE_KEY is \${{ secrets['FIGMA_FILE_KEY'] }}`
      ])
    )
  })
})

// Every module a source loads: static imports, re-exports and literal dynamic imports, plus any load whose
// target is not a literal (require, createRequire, import of an expression), which counts as external.
function moduleLoads(source: string): string[] {
  const literal =
    /^\s*(?:import|export)\b[^'"]*?\bfrom\s*['"]([^'"]+)['"]|^\s*import\s*['"]([^'"]+)['"]|\bimport\s*\(\s*['"]([^'"]+)['"]\s*\)/gm
  const loads = [...source.matchAll(literal)].map((match) => match[1] ?? match[2] ?? match[3]!)
  for (const opaque of source.matchAll(
    /\brequire\s*\(|\bcreateRequire\b|\bimport\s*\(\s*(?!['"][^'"]+['"]\s*\))/g
  ))
    loads.push(opaque[0])
  return loads
}
const externalLoads = (source: string) =>
  moduleLoads(source).filter((load) => !load.startsWith('node:'))

describe('ui-library-key-scan.yml', () => {
  const workflow = load('ui-library-key-scan.yml')
  const prepare = stepsOf(workflow.jobs.prepare)
  const scanJob = stepsOf(workflow.jobs['key-scan'])
  const allSteps = [...prepare, ...scanJob]
  const runs = allSteps.flatMap((step) => (step.run ? [step.run] : []))
  const [first] = prepare
  const keyStep = scanJob.find((step) => JSON.stringify(step).includes(SECRET))!

  it('should run only from workflow_run, with the least permissions each job needs', () => {
    expect(triggers(workflow)).toEqual(['workflow_run'])
    expect(workflow.permissions).toEqual({})
    for (const job of Object.values(workflow.jobs))
      expect(job.permissions).toEqual({ actions: 'read', checks: 'write', contents: 'read' })
  })

  it('should hold the key in the key-scan job only, which needs prepare and declares no environment', () => {
    expect(Object.keys(workflow.jobs)).toEqual(['prepare', 'key-scan'])
    expect(isKeyReaderJob(workflow.jobs.prepare!)).toBe(false)
    const job = workflow.jobs['key-scan']!
    expect(job.environment).toBeUndefined()
    expect(job.needs).toBe('prepare')
    expect(job.if).toBe("needs.prepare.outputs.scan == 'true'")
    expect(workflow.jobs.prepare!.outputs).toEqual({
      head: '${{ steps.trigger.outputs.head }}',
      base: '${{ steps.trigger.outputs.base }}',
      scan: '${{ steps.trigger.outputs.scan }}',
      reason: '${{ steps.trigger.outputs.reason }}',
      artifact_required: '${{ steps.inputs.outputs.artifact_required }}'
    })
  })

  it('should run the scan template, character for character, with the report template after it', () => {
    const [scanTemplate] = KEY_STEP_TEMPLATES
    expect(keyStep.run!.trim()).toBe(scanTemplate!.run)
    expect(keyStep.id).toBe('scan')
    expect(scanJob.at(-1)!.run!.trim()).toBe(REPORT_TEMPLATE.run)
    expect(scanJob.filter((step) => step.run !== undefined)).toHaveLength(2)
    expect(keyStep.env).toMatchObject({ TRUSTED_SHA: '${{ github.sha }}', ...GIT_HARDENING })
  })

  it('should hand the scan its commits and LFS objects from prepare, and the Review build from the triggering run', () => {
    const downloads = scanJob.filter((step) => step.uses?.startsWith('actions/download-artifact@'))
    expect(downloads.map((step) => step.with)).toEqual([
      { name: 'key-scan-input', path: '${{ runner.temp }}/key-scan-input' },
      {
        name: 'ui-review-${{ needs.prepare.outputs.head }}',
        path: '${{ runner.temp }}/pr-artifact',
        'run-id': '${{ github.event.workflow_run.id }}',
        'github-token': '${{ github.token }}'
      }
    ])
    const upload = prepare.find((step) => step.uses?.startsWith('actions/upload-artifact@'))!
    expect(upload.with).toMatchObject({
      name: 'key-scan-input',
      path: '${{ runner.temp }}/key-scan-input',
      'if-no-files-found': 'error'
    })
    const inputs = prepare.find((step) => step.id === 'inputs')!
    expect(prepare.indexOf(upload)).toBeGreaterThan(
      prepare.findIndex((step) => step.id === 'artifact_check')
    )
    expect(inputs.run).toContain(
      'git -C trusted bundle create "$out/commits.bundle" "${refs[@]}" --not "$main^@"'
    )
    expect(inputs.run).toContain('cp -R trusted/.git/lfs/objects/. "$out/lfs/"')
  })

  it('should count the Review artifacts with a gh call gh accepts', () => {
    const check = prepare.find((step) => step.id === 'artifact_check')!
    // gh rejects --slurp together with --jq before calling the API, which failed every artifact check.
    expect(check.run).not.toMatch(/--slurp/)
    expect(check.run).toContain('--paginate')
    expect(check.run).toMatch(/\| \.id" \| wc -l/)
    expect(check.run).toMatch(/^set -o pipefail$/m)
  })

  it('should check out only the default branch, into trusted/, in each job', () => {
    for (const steps of [prepare, scanJob]) {
      const checkouts = steps.filter((step) => step.uses?.startsWith('actions/checkout@'))
      expect(checkouts).toHaveLength(1)
      expect(checkouts[0]!.with).toMatchObject({
        ref: '${{ github.event.repository.default_branch }}',
        path: 'trusted',
        'fetch-depth': 0
      })
    }
  })

  it('should execute nothing but the trusted copy of the key guard', () => {
    for (const run of runs) {
      expect(run, run).not.toMatch(/\b(pnpm|npm|npx|yarn|corepack)\b/)
      // Every script is main's: from trusted/, or the key guard blob committed there, written out fresh.
      for (const [, script] of run.matchAll(/\bnode\s+(\S+)/g))
        expect(script!.startsWith('trusted/') || script === `"${GUARD_COPY}"`, run).toBe(true)
      expect(run, run).not.toMatch(/\b(bash|sh|source)\s+\S/)
    }
  })

  it('should give the secret to the key guard step only, never at job or workflow level', () => {
    expect(JSON.stringify(workflow.env ?? {})).not.toContain(SECRET)
    for (const job of Object.values(workflow.jobs))
      expect(JSON.stringify(job.env ?? {})).not.toContain(SECRET)
    expect(allSteps.filter((step) => JSON.stringify(step).includes(SECRET))).toEqual([keyStep])
    expect(JSON.stringify(workflow.jobs.prepare)).not.toContain('secrets.')
  })

  it('should run a key guard that is one self-contained file, so its committed blob is the whole program', () => {
    const source = readFileSync(path.join(repoRoot, 'library/ui/figma/key-guard.mjs'), 'utf8')
    expect(moduleLoads(source).length).toBeGreaterThan(0)
    expect(externalLoads(source)).toEqual([])
  })

  it.each([
    "import { x } from './helper.mjs'",
    "import './side-effect.mjs'",
    "export { x } from './helper.mjs'",
    'export * from "./helper.mjs"',
    "export * as helper from './helper.mjs'",
    "const helper = await import('./helper.mjs')",
    'const helper = await import(name)',
    "const helper = require('./helper.cjs')",
    'const load = createRequire(import.meta.url)'
  ])('should find that a guard with %s is not one self-contained file', (line) => {
    expect(externalLoads(`import { readFileSync } from 'node:fs'\n${line}\n`)).not.toEqual([])
  })

  it('should validate the triggering run before anything else, requiring a successful run', () => {
    expect(first!.id).toBe('trigger')
    expect(first!.env).toMatchObject({ CONCLUSION: '${{ github.event.workflow_run.conclusion }}' })
    expect(first!.run).toContain('[ "$CONCLUSION" = success ]')
    expect(first!.run).toContain('[ "$HEAD_REPO" = "$REPOSITORY" ]')
    expect(first!.run).toContain(
      '[[ "$HEAD_SHA" =~ ^[0-9a-f]{40}$ && "$RUN_ID" =~ ^[0-9]+$ ]] || exit 1'
    )
    expect(JSON.stringify(first)).not.toContain('secrets.')
  })

  it('should pin the triggering workflow by file path and id, never by display name', () => {
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
    expect(first!.run).toContain('[ "$head" = "$HEAD_SHA" ]')
    expect(first!.run).toContain('[ "$(jq \'length\' <<< "$PULL_REQUESTS")" = 1 ]')
    const artifact = prepare.find((step) => step.run?.includes('/artifacts'))!
    expect(artifact.run).toContain('ui-review-$HEAD_SHA')
    expect(artifact.run).toContain('actions/runs/$RUN_ID/artifacts')
    expect(artifact.run).toContain('.workflow_run.id == $RUN_ID')
    expect(artifact.run).toContain('.workflow_run.head_sha == \\"$HEAD_SHA\\"')
  })

  it('should read no Git configuration but its own, and keep the checkouts credential-free', () => {
    expect(workflow.env?.GIT_CONFIG_GLOBAL).toBeUndefined()
    for (const step of allSteps.filter(
      (step) => step.run?.includes('git -C trusted') || step.run?.includes('node trusted/')
    ))
      expect(step.env).toMatchObject({ GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null' })
    for (const checkout of allSteps.filter((step) => step.uses?.startsWith('actions/checkout@')))
      expect(checkout.with).toMatchObject({
        'persist-credentials': false,
        lfs: false,
        'set-safe-directory': false
      })
  })

  it('should prefetch every LFS object the scan reads, by oid, from the fixed repository endpoint', () => {
    const lfs = prepare.find((step) => step.run?.includes('lfs-prefetch.mjs'))!
    expect(lfs.run).toContain(`node "${GUARD_COPY}" --root trusted --no-worktree --range "$range"`)
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
    expect(JSON.stringify(scanJob)).not.toMatch(/READ_TOKEN|LFS_AUTHORIZATION/)
    expect(keyStep.run).toContain('--lfs-store "$RUNNER_TEMP/key-scan-input/lfs"')
  })

  it('should build the check summary from the scanner stdout alone', () => {
    expect(keyStep.run).toContain('| tee "$RUNNER_TEMP/key-scan.txt"')
    expect(keyStep.run).not.toMatch(/2>&1|&>|2>\s*"?\$RUNNER_TEMP\/key-scan/)
    const report = scanJob.at(-1)!
    expect(report.run).toContain('head -c 60000 "$RUNNER_TEMP/key-scan.txt"')
    expect(report.run!.match(/key-scan\.txt/g)).toHaveLength(1)
    expect(JSON.stringify(report)).not.toContain(SECRET)
  })

  it('should scan the artifact through the limited, link-refusing enumerator', () => {
    expect(keyStep.run).toContain('--artifact "$RUNNER_TEMP/pr-artifact"')
    expect(keyStep.run).not.toContain('--path')
  })

  it('should never let a skipped scan, a fork run included, conclude a passing check', () => {
    const report = prepare.at(-1)!
    expect(first!.run).not.toMatch(/neutral|skipped|conclusion=success/)
    expect(report.run).toMatch(/^conclusion=failure$/m)
    expect(report.run).not.toMatch(/conclusion=success|neutral|skipped/)
    expect(report.if).toBe(
      "always() && steps.trigger.outputs.head != '' && (steps.trigger.outputs.scan != 'true' || failure() || cancelled())"
    )
  })

  it('should report a failed check that says the key check could not run on a fork run, without the key', () => {
    const report = prepare.at(-1)!
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
        { encoding: 'utf8', env: { ...env, SKIP_REASON: output.reason } }
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
    const script = stepsOf(load('ui-library-key-scan.yml').jobs.prepare)[0]!.run!

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
    const all = Object.values(ui.jobs).flatMap(stepsOf)
    for (const checkout of all.filter((step) => step.uses?.includes('actions/checkout')))
      expect(checkout.with).toMatchObject({ 'set-safe-directory': false })
    for (const step of all.filter(
      (step) => step.run?.includes('git -C trusted') || step.run?.includes('node trusted/')
    ))
      expect(step.env).toMatchObject({ GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null' })
  })
})
