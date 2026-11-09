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
// Actions a key-holding job may use, each pinned by a full commit sha.
const TRUSTED_ACTIONS =
  /^(actions\/checkout|actions\/setup-node|actions\/download-artifact|decentraland\/oddish-action)@[0-9a-f]{40}$/
// The inputs each action may take in a key-holding job; anything else could change where code comes from.
// oddish-action publishes the package and runs after the scan, so its inputs are not constrained here.
const ACTION_INPUTS: Record<string, string[] | undefined> = {
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
  'actions/download-artifact': ['name', 'path', 'run-id', 'github-token', 'repository'],
  'decentraland/oddish-action': undefined
}
const THIS_REPOSITORY = '${{ github.repository }}'
// A downloaded artifact lands under the runner's temporary directory, or in a workspace dot directory that
// holds no code: never over a checkout, .github or .git.
const DATA_PATH = /^(?:\$\{\{ runner\.temp \}\}\/|\.(?!git(?:hub)?(?:\/|$))[\w-]+(?:\/|$))/
const PARENT_SEGMENT = /(?:^|[\s/"'=])\.\.(?:[/\s"']|$)/
// Git commands that rewrite a working tree or the ref it is on, whatever ref they name.
// replace refs, ref updates and config change what HEAD:<path> resolves to without touching the working tree.
const REWRITES_CHECKOUT =
  /\bgit\b[^\n;|&]*\s(?:checkout|switch|reset|restore|worktree|read-tree|checkout-index|update-index|pull|merge|rebase|cherry-pick|am|apply|stash|clean|replace|update-ref|symbolic-ref|config)\b|\bgit\b[^\n;|&]*refs\/replace\b|--update-head-ok/
// Variables that make the shell or node load code a step did not name.
const LOADER_VARIABLES =
  /^(NODE_OPTIONS|NODE_PATH|LD_PRELOAD|LD_LIBRARY_PATH|BASH_ENV|ENV|PATH|SHELLOPTS|GIT_DIR|GIT_WORK_TREE|GIT_OBJECT_DIRECTORY|GIT_ALTERNATE_OBJECT_DIRECTORIES|GIT_REPLACE_REF_BASE|GIT_NAMESPACE|GIT_INDEX_FILE|GIT_CONFIG_PARAMETERS|GIT_CONFIG_COUNT)$/i
// Where a command starts in a shell line.
const START = String.raw`(?:^|[;&|(]|\b(?:then|do|else)\b)\s*`
// Pipes and redirects of the key step may only write into $RUNNER_TEMP.
const TEMP_PATH = String.raw`"?\$\{?RUNNER_TEMP\}?/[^"\s]*"?`
// The key step runs the guard committed at HEAD of the checkout, written to a fresh file by this exact command,
// never the working tree copy, which an earlier step can change. --no-replace-objects keeps refs/replace from
// swapping the blob. key-guard.mjs imports only node: builtins, so that one file is the whole program.
const GUARD_COPY = '$RUNNER_TEMP/key-guard.mjs'
const FRESH_GUARD =
  /^git (?:-C trusted )?--no-replace-objects cat-file blob HEAD:library\/ui\/figma\/key-guard\.mjs > "\$RUNNER_TEMP\/key-guard\.mjs"$/
const FRESH_GUARD_NODE = /^node "\$RUNNER_TEMP\/key-guard\.mjs"(?: [^;&|]*)?$/
// One shell command per entry: lines, lists and pipes split, with the keywords that only steer them removed.
function shellCommands(run: string): string[] {
  return run
    .replace(/\\\n/g, ' ')
    .split(/\n|&&|\|\||[;|]/)
    .map((part) => {
      let command = part.trim()
      for (let previous = ''; previous !== command;) {
        previous = command
        command = command.replace(/^(?:if|then|else|elif|while|until|do|!)(?:\s+|$)/, '').trim()
      }
      return /^(?:fi|done)$/.test(command) ? '' : command
    })
    .filter(Boolean)
}
// A path as the shell sees it: quotes dropped, ${RUNNER_TEMP} spelled $RUNNER_TEMP, /./ and // collapsed.
function normalizePath(token: string): string {
  let path = token.replace(/["']/g, '').replace(/\$\{RUNNER_TEMP\}/g, '$RUNNER_TEMP')
  for (let previous = ''; previous !== path;) {
    previous = path
    path = path.replace(/\/\.(?=\/|$)/g, '').replace(/\/{2,}/g, '/')
  }
  return path
}
// A redirect and its whole target word, quoted and unquoted parts together.
const WORD = String.raw`((?:"[^"]*"|'[^']*'|[^\s;|&"'<>])+)`
const REDIRECT = new RegExp(String.raw`(?<![<&\d])\d?>>?(?!&)\s*${WORD}|<\s*${WORD}`, 'g')
function freshGuardViolations(commands: string[]): string[] {
  const copy = commands.findIndex((command) => FRESH_GUARD.test(command))
  const nodes = commands.flatMap((command, index) => (/^node\b/.test(command) ? [index] : []))
  // Every write that lands on the copy, however the path is spelled; only the copy command may make one.
  const writes = commands.flatMap((command, index) => {
    const targets = [...command.matchAll(REDIRECT)].flatMap(([redirect, target]) =>
      redirect.trimStart().startsWith('<') ? [] : [target!]
    )
    if (/^tee\b/.test(command))
      targets.push(...(command.match(/"[^"]*"|'[^']*'|\S+/g) ?? []).slice(1))
    return targets.some((target) => normalizePath(target) === GUARD_COPY) ? [index] : []
  })
  if (
    copy < 0 ||
    nodes.length === 0 ||
    nodes.some((index) => index < copy || !FRESH_GUARD_NODE.test(commands[index]!)) ||
    writes.length !== 1 ||
    writes[0] !== copy
  )
    return ['does not run a fresh copy of the committed key guard']
  return []
}
const KEY_STEP_COMMANDS = [
  /^set -o pipefail$/,
  /^git (?:-C trusted )?--no-replace-objects cat-file blob HEAD:library\/ui\/figma\/key-guard\.mjs$/,
  FRESH_GUARD_NODE,
  new RegExp(`^tee(?: -a)? ${TEMP_PATH}$`),
  new RegExp(`^grep(?: -[A-Za-z]+)* (?:'[^']*'|"[^"$]*")(?: ${TEMP_PATH})?$`),
  new RegExp(`^head(?: -[cn] \\d+)?(?: ${TEMP_PATH})?$`),
  new RegExp(`^cat(?: ${TEMP_PATH})*$`),
  // Plain assignments and tests, which the scan needs to build its arguments; nothing they hold runs.
  /^[A-Za-z_]\w*=(?:\([^()]*\)|"[^"]*"|'[^']*'|[^\s"'()]*)$/,
  /^\[ [^[\]]* \]$/
]
// Commands that write files, and where. A step before the key step may not write into a checkout (or, for a
// checkout at the workspace root, anywhere in the workspace but a dot directory), because the checkout holds
// the code the key step trusts. tar -x and unzip extract into the working directory unless told otherwise.
const WRITERS = /^(?:cp|mv|install|rsync|ln|dd|tee|sed|tar|unzip)$/
function protectsPath(token: string, checkouts: string[]): boolean {
  const path = token
    .replace(/^["']|["']$/g, '')
    .replace(/^(?:of=|--?[\w-]+=)/, '')
    .replace(/^\.\/+/, '')
  if (path === '' || path.startsWith('-') || path.startsWith('&')) return false
  // The workspace itself holds every checkout, so a recursive copy or an extraction there can reach them.
  if (path === '.' || path === '*') return true
  // A repository's metadata decides what HEAD:<path> is, and .github holds the workflows.
  if (/(?:^|\/)\.git(?:hub)?(?:\/|$)/.test(path)) return true
  if (/GITHUB_WORKSPACE|github\.workspace/.test(path)) return true
  if (/(?:^|\/)\.\.(?:\/|$)/.test(path)) return true
  if (path.startsWith('/')) return !/^\/(?:dev|tmp)\//.test(path)
  if (path.startsWith('$') || path.startsWith('~')) return false
  const root = checkouts.some((checkout) => checkout === '' || checkout === '.')
  if (root) return path === '.' || !path.startsWith('.')
  return ['trusted', 'library', ...checkouts].some(
    (prefix) => path === prefix || path.startsWith(`${prefix}/`)
  )
}
function checkoutWrites(run: string, checkouts: string[]): string[] {
  const problems: string[] = []
  for (const segment of shellCommands(run)) {
    for (const [redirect, target] of segment.matchAll(REDIRECT))
      if (!redirect.trimStart().startsWith('<') && protectsPath(target!, checkouts))
        problems.push(`writes the checkout before the key step: ${segment}`)
    const command = segment
      .replace(/^(?:[A-Za-z_]\w*=\S*\s+)*/, '')
      .replace(/^(?:(?:sudo|command|exec|nohup|xargs)\s+(?:-\S+\s+)*)*/, '')
    const [name = '', ...args] = command.match(/"[^"]*"|'[^']*'|\S+/g) ?? []
    const tool = name.split('/').pop()!
    if (!WRITERS.test(tool)) continue
    if (tool === 'sed' && !args.some((arg) => /^-[A-Za-z]*i|^--in-place/.test(arg))) continue
    // tar extracts with x in its first argument (bundled, dash optional), in a short option bundle, or --extract.
    if (
      tool === 'tar' &&
      !/^-?[A-Za-z]*x[A-Za-z]*$/.test(args[0] ?? '') &&
      !args.some((arg) => /^-[A-Za-z]*x[A-Za-z]*$|^--(?:extract|get)$/.test(arg))
    )
      continue
    let targets = args
    if (tool === 'sed') targets = args.filter((arg) => !arg.startsWith('-')).slice(1)
    // cp, install, rsync and ln write their last operand (or the -t directory); mv also removes its sources.
    if (['cp', 'install', 'rsync', 'ln'].includes(tool)) {
      const operands = args.filter((arg) => !arg.startsWith('-'))
      const directory = args.findIndex((arg) => arg === '-t' || arg === '--target-directory')
      targets = directory >= 0 ? [args[directory + 1] ?? '.'] : operands.slice(-1)
    }
    if (tool === 'dd') targets = args.filter((arg) => arg.startsWith('of='))
    if (tool === 'tar' || tool === 'unzip') {
      // -C DIR, -CDIR, --directory DIR and --directory=DIR for tar; -d DIR and -dDIR for unzip.
      const [short, long] = tool === 'tar' ? ['-C', '--directory'] : ['-d', '--no-long-form']
      targets = []
      for (const [index, arg] of args.entries()) {
        if (arg === short || arg === long) targets.push(args[index + 1] ?? '.')
        else if (arg.startsWith(`${long}=`)) targets.push(arg.slice(long.length + 1))
        else if (arg.startsWith(short) && arg.length > short.length && !/^-[A-Za-z]*x/.test(arg))
          targets.push(arg.slice(short.length))
      }
      if (targets.length === 0) targets.push('.')
    }
    if (targets.some((target) => protectsPath(target, checkouts)))
      problems.push(`writes the checkout before the key step: ${segment}`)
  }
  return problems
}

// What the step that maps the key may run: the commands above, joined by pipes, lists and redirects only.
// Every command is checked, node included, wherever it sits in a line.
function keyStepViolations(run: string): string[] {
  const problems: string[] = []
  if (/\$\(|`|<\(|>\(|&(?!&)|\/proc\b|environ/.test(run.replace(/&&/g, '')))
    problems.push('substitutes, backgrounds or reads the environment')
  // An expression is expanded into the script before the shell parses it, so a branch name could inject code.
  if (run.includes('${{')) problems.push('interpolates an expression')
  if (PARENT_SEGMENT.test(run)) problems.push('uses a parent path')
  const commands = shellCommands(run)
  problems.push(...freshGuardViolations(commands))
  for (const command of commands) {
    let rest = command
    const assigned = /^([A-Za-z_]\w*)=/.exec(rest)?.[1]
    if (assigned && LOADER_VARIABLES.test(assigned)) problems.push(`sets ${assigned}`)
    for (const [redirect, target, source] of command.matchAll(REDIRECT)) {
      const path = target ?? source ?? ''
      if (!new RegExp(`^${TEMP_PATH}$`).test(path.trim()) || /[*?[]/.test(path))
        problems.push(`redirects outside $RUNNER_TEMP: ${redirect.trim()}`)
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
    const keyIndex = jobSteps.findIndex((step) => referencesKey(step.env))
    const checkouts = jobSteps
      .filter((step) => step.uses?.startsWith('actions/checkout@'))
      .map((step) => String(step.with?.path ?? '').replace(/\/+$/, ''))
    for (const [index, step] of jobSteps.entries()) {
      if (keyIndex < 0 || index < keyIndex)
        for (const problem of checkoutWrites(step.run ?? '', checkouts))
          problems.push(`${id}: ${problem}`)
      const { env, ...stepRest } = step
      if (referencesKey(stepRest)) problems.push(`${id}: key outside a step env`)
      for (const name of Object.keys(env ?? {}))
        if (LOADER_VARIABLES.test(name)) problems.push(`${id}: sets ${name}`)
      if (step.uses !== undefined) {
        if (!TRUSTED_ACTIONS.test(step.uses)) problems.push(`${id}: uses ${step.uses}`)
        if (referencesKey(env)) problems.push(`${id}: key in an action step`)
        const action = step.uses.split('@')[0]!
        const inputs = ACTION_INPUTS[action]
        for (const input of Object.keys(step.with ?? {}))
          if (inputs && !inputs.includes(input)) problems.push(`${id}: ${action} sets ${input}`)
        const repository = step.with?.repository
        if (repository !== undefined && repository !== THIS_REPOSITORY)
          problems.push(`${id}: ${action} from ${String(repository)}`)
        if (action === 'actions/checkout') {
          const ref = String(step.with?.ref ?? '')
          if (!TRUSTED_REFS.includes(ref))
            problems.push(`${id}: checkout of ${ref || 'the event ref'}`)
          if (step.with?.['persist-credentials'] !== false)
            problems.push(`${id}: checkout persists credentials`)
          if (PARENT_SEGMENT.test(String(step.with?.path ?? '')))
            problems.push(`${id}: checkout into a parent path`)
        }
        if (action === 'actions/download-artifact') {
          const target = String(step.with?.path ?? '')
          if (!DATA_PATH.test(target) || PARENT_SEGMENT.test(target))
            problems.push(`${id}: downloads an artifact into ${target || 'the workspace'}`)
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
      // The key step's fresh guard copy is the one file node may run from $RUNNER_TEMP.
      const ranFiles = referencesKey(env)
        ? run.replace(/^node "\$RUNNER_TEMP\/key-guard\.mjs"/gm, '')
        : run
      if (
        /\b(node|python[\d.]*)\s+(?:-\S+\s+)*["']?(\$\{?RUNNER_TEMP|(\.\/)?\.preview\b)/.test(
          ranFiles
        )
      )
        problems.push(`${id}: runs a downloaded file`)
      if (REWRITES_CHECKOUT.test(run)) problems.push(`${id}: rewrites a checkout with git`)
      // GITHUB_ENV and GITHUB_PATH reach every later step, around the loader variable rule.
      if (/\bGITHUB_(?:ENV|PATH)\b/.test(run))
        problems.push(`${id}: writes GITHUB_ENV or GITHUB_PATH`)
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
  const FRESH_COPY =
    'git -C trusted --no-replace-objects cat-file blob HEAD:library/ui/figma/key-guard.mjs > "$RUNNER_TEMP/key-guard.mjs"'
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
            FRESH_COPY,
            'node "$RUNNER_TEMP/key-guard.mjs" --root trusted | tee "$RUNNER_TEMP/out.txt"',
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
  // The same reader with its checkout at the workspace root, as the CDN publisher has it.
  const rootCheckout = (run: string) =>
    reader((job) => {
      delete job.steps[0]!.with!.path
      job.steps[1] = { run }
    })
  const keyStep = (run: string, extra: Partial<WorkflowStep> = {}) =>
    reader((job) => Object.assign(job.steps[2]!, { run }, extra))

  it('should accept a key step that copies, runs and checks the guard on one line', () => {
    const line = [
      'node "$RUNNER_TEMP/key-guard.mjs" --root trusted | tee "$RUNNER_TEMP/o.txt"',
      `grep -qx 'check key: ran' "$RUNNER_TEMP/o.txt"`
    ].join(' && ')
    expect(keyReaderViolations(keyStep([FRESH_COPY, line].join('\n')))).toEqual([])
  })

  it.each([
    'tar -czf "$RUNNER_TEMP/a.tgz" index.html',
    'tar cf "$RUNNER_TEMP/a.tar" xfiles',
    'tar -xf "$RUNNER_TEMP/x.tar" --directory="$RUNNER_TEMP/out"',
    'unzip -o "$RUNNER_TEMP/x.zip" -d "$RUNNER_TEMP/out"'
  ])('should not flag %s, which writes only into $RUNNER_TEMP', (run) => {
    expect(keyReaderViolations(otherStep(run))).toEqual([])
    expect(keyReaderViolations(rootCheckout(run))).toEqual([])
  })

  it('should accept a reader that follows every rule', () => {
    expect(keyReaderViolations(reader())).toEqual([])
    expect(keyReaderViolations(otherStep('python3 tooling/web-preview.py resolve'))).toEqual([])
    // Writes into $RUNNER_TEMP, $GITHUB_OUTPUT or a dot directory leave the checkout alone.
    for (const run of [
      'cp trusted/x "$RUNNER_TEMP/x" && echo "a=b" >> "$GITHUB_OUTPUT"',
      'tar -xf "$RUNNER_TEMP/x.tar" -C "$RUNNER_TEMP/out" 2> /dev/null',
      'echo x >&2'
    ])
      expect(keyReaderViolations(otherStep(run))).toEqual([])
    expect(keyReaderViolations(rootCheckout('mkdir -p .preview && cp x .preview/y'))).toEqual([])
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
      'scan: key step does not run a fresh copy of the committed key guard'
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
      'scan: rewrites a checkout with git'
    ],
    [
      'a switch to the scanned head',
      otherStep('git -C trusted switch --detach refs/key-scan/head'),
      'scan: rewrites a checkout with git'
    ],
    [
      'a worktree of the scanned head',
      otherStep('git -C trusted worktree add ../x refs/key-scan/head'),
      'scan: rewrites a checkout with git'
    ],
    ['printenv', otherStep('printenv'), 'scan: prints the key'],
    ['a bare env', otherStep('env | sort'), 'scan: prints the key'],
    ['indirect expansion', otherStep('v=FIGMA_FILE_KEY; echo "${!v}"'), 'scan: prints the key'],
    ['xtrace', otherStep('set -o xtrace'), 'scan: prints the key'],
    ['a lower-case key variable', otherStep('echo "$figma_file_key"'), 'scan: prints the key'],
    [
      'a cp into trusted/',
      otherStep('cp "$RUNNER_TEMP/pr-artifact/x.mjs" trusted/library/ui/figma/key-guard.mjs'),
      'scan: writes the checkout before the key step: cp "$RUNNER_TEMP/pr-artifact/x.mjs" trusted/library/ui/figma/key-guard.mjs'
    ],
    [
      'an mv into trusted/',
      otherStep('mv "$RUNNER_TEMP/x" trusted/library/ui/figma/key-guard.mjs'),
      'scan: writes the checkout before the key step: mv "$RUNNER_TEMP/x" trusted/library/ui/figma/key-guard.mjs'
    ],
    [
      'an install into trusted/',
      otherStep('install -m 644 "$RUNNER_TEMP/x" trusted/library/ui/figma/key-guard.mjs'),
      'scan: writes the checkout before the key step: install -m 644 "$RUNNER_TEMP/x" trusted/library/ui/figma/key-guard.mjs'
    ],
    [
      'an rsync into trusted/',
      otherStep('rsync -a "$RUNNER_TEMP/pr-artifact/" trusted/'),
      'scan: writes the checkout before the key step: rsync -a "$RUNNER_TEMP/pr-artifact/" trusted/'
    ],
    [
      'an ln into trusted/',
      otherStep('ln -sf "$RUNNER_TEMP/x" trusted/library/ui/figma/key-guard.mjs'),
      'scan: writes the checkout before the key step: ln -sf "$RUNNER_TEMP/x" trusted/library/ui/figma/key-guard.mjs'
    ],
    [
      'a tar -x into trusted/',
      otherStep('tar -xf "$RUNNER_TEMP/x.tar" -C trusted'),
      'scan: writes the checkout before the key step: tar -xf "$RUNNER_TEMP/x.tar" -C trusted'
    ],
    [
      'a tar x into the working directory',
      otherStep('tar xf "$RUNNER_TEMP/x.tar"'),
      'scan: writes the checkout before the key step: tar xf "$RUNNER_TEMP/x.tar"'
    ],
    [
      'an unzip into the working directory',
      otherStep('unzip -o "$RUNNER_TEMP/x.zip"'),
      'scan: writes the checkout before the key step: unzip -o "$RUNNER_TEMP/x.zip"'
    ],
    [
      'an unzip into library/',
      otherStep('unzip -o "$RUNNER_TEMP/x.zip" -d library'),
      'scan: writes the checkout before the key step: unzip -o "$RUNNER_TEMP/x.zip" -d library'
    ],
    [
      'a dd into trusted/',
      otherStep('dd if="$RUNNER_TEMP/x" of=trusted/library/ui/figma/key-guard.mjs'),
      'scan: writes the checkout before the key step: dd if="$RUNNER_TEMP/x" of=trusted/library/ui/figma/key-guard.mjs'
    ],
    [
      'a sed -i in trusted/',
      otherStep("sed -i 's/a/b/' trusted/library/ui/figma/key-guard.mjs"),
      "scan: writes the checkout before the key step: sed -i 's/a/b/' trusted/library/ui/figma/key-guard.mjs"
    ],
    [
      'a tee into library/',
      otherStep('tee library/ui/figma/key-guard.mjs'),
      'scan: writes the checkout before the key step: tee library/ui/figma/key-guard.mjs'
    ],
    [
      'a > redirect into trusted/',
      otherStep('echo x > trusted/library/ui/figma/key-guard.mjs'),
      'scan: writes the checkout before the key step: echo x > trusted/library/ui/figma/key-guard.mjs'
    ],
    [
      'a >> redirect into trusted/',
      otherStep('echo x >> "trusted/library/ui/figma/key-guard.mjs"'),
      'scan: writes the checkout before the key step: echo x >> "trusted/library/ui/figma/key-guard.mjs"'
    ],
    [
      'a write through GITHUB_WORKSPACE',
      otherStep('cp x "$GITHUB_WORKSPACE/trusted/library/ui/figma/key-guard.mjs"'),
      'scan: writes the checkout before the key step: cp x "$GITHUB_WORKSPACE/trusted/library/ui/figma/key-guard.mjs"'
    ],
    [
      'a write through a parent path',
      otherStep('cp x "$RUNNER_TEMP/../work/trusted/key-guard.mjs"'),
      'scan: writes the checkout before the key step: cp x "$RUNNER_TEMP/../work/trusted/key-guard.mjs"'
    ],
    [
      'a write to an absolute workspace path',
      otherStep('cp x /home/runner/work/repo/repo/trusted/key-guard.mjs'),
      'scan: writes the checkout before the key step: cp x /home/runner/work/repo/repo/trusted/key-guard.mjs'
    ],
    [
      'a cp into a root checkout',
      rootCheckout('cp "$RUNNER_TEMP/x" library/ui/figma/key-guard.mjs'),
      'scan: writes the checkout before the key step: cp "$RUNNER_TEMP/x" library/ui/figma/key-guard.mjs'
    ],
    [
      'a redirect into a root checkout',
      rootCheckout('echo x > tooling/web-preview.py'),
      'scan: writes the checkout before the key step: echo x > tooling/web-preview.py'
    ],
    [
      'a sed -i in a root checkout',
      rootCheckout("sed -i 's/a/b/' ./tooling/web-preview.py"),
      "scan: writes the checkout before the key step: sed -i 's/a/b/' ./tooling/web-preview.py"
    ],
    [
      'a recursive copy into the workspace',
      otherStep('cp -r "$RUNNER_TEMP/pr-artifact/." .'),
      'scan: writes the checkout before the key step: cp -r "$RUNNER_TEMP/pr-artifact/." .'
    ],
    [
      'an mv out of trusted/',
      otherStep('mv trusted/library/ui/figma/key-guard.mjs "$RUNNER_TEMP/x"'),
      'scan: writes the checkout before the key step: mv trusted/library/ui/figma/key-guard.mjs "$RUNNER_TEMP/x"'
    ],
    [
      'a same-line node after ;',
      keyStep(
        [FRESH_COPY, 'node "$RUNNER_TEMP/key-guard.mjs" --root trusted ; node -e "x"'].join('\n')
      ),
      'scan: key step runs node -e "x"'
    ],
    [
      'a same-line node after &&',
      keyStep(
        [FRESH_COPY, 'node "$RUNNER_TEMP/key-guard.mjs" --root trusted && node -e "x"'].join('\n')
      ),
      'scan: key step runs node -e "x"'
    ],
    [
      'a same-line node after ||',
      keyStep(
        [FRESH_COPY, 'node "$RUNNER_TEMP/key-guard.mjs" --root trusted || node -e "x"'].join('\n')
      ),
      'scan: key step runs node -e "x"'
    ],
    [
      'a same-line node after |',
      keyStep(
        [FRESH_COPY, 'node "$RUNNER_TEMP/key-guard.mjs" --root trusted | node -e "x"'].join('\n')
      ),
      'scan: key step runs node -e "x"'
    ],
    [
      'node inside an if',
      keyStep(
        [
          FRESH_COPY,
          'if node -e "x"; then grep -q \'a\' "$RUNNER_TEMP/o.txt"; fi',
          'node "$RUNNER_TEMP/key-guard.mjs" --root trusted'
        ].join('\n')
      ),
      'scan: key step runs node -e "x"'
    ],
    [
      'node in a subshell',
      keyStep(
        [FRESH_COPY, 'node "$RUNNER_TEMP/key-guard.mjs" --root trusted', '( node -e "x" )'].join(
          '\n'
        )
      ),
      'scan: key step runs ( node -e "x" )'
    ],
    [
      'python in the key step',
      keyStep(
        [FRESH_COPY, 'node "$RUNNER_TEMP/key-guard.mjs" --root trusted', 'python3 -c "x"'].join(
          '\n'
        )
      ),
      'scan: key step runs python3 -c "x"'
    ],
    [
      'sh in the key step',
      keyStep(
        [FRESH_COPY, 'node "$RUNNER_TEMP/key-guard.mjs" --root trusted', 'sh -c "x"'].join('\n')
      ),
      'scan: key step runs sh -c "x"'
    ],
    [
      'eval in the key step',
      keyStep(
        [FRESH_COPY, 'node "$RUNNER_TEMP/key-guard.mjs" --root trusted', 'eval "x"'].join('\n')
      ),
      'scan: key step runs eval "x"'
    ],
    [
      'a dot-segment rewrite of the copy',
      keyStep(
        [
          FRESH_COPY,
          'head -c 999999 "$RUNNER_TEMP/pr-artifact/x.mjs" > "$RUNNER_TEMP/./key-guard.mjs"',
          'node "$RUNNER_TEMP/key-guard.mjs" --root trusted'
        ].join('\n')
      ),
      'scan: key step does not run a fresh copy of the committed key guard'
    ],
    [
      'a double-slash rewrite of the copy',
      keyStep(
        [
          FRESH_COPY,
          'grep -h \'\' "$RUNNER_TEMP/pr-artifact/x.mjs" >> "$RUNNER_TEMP//key-guard.mjs"',
          'node "$RUNNER_TEMP/key-guard.mjs" --root trusted'
        ].join('\n')
      ),
      'scan: key step does not run a fresh copy of the committed key guard'
    ],
    [
      'a braced rewrite of the copy',
      keyStep(
        [
          FRESH_COPY,
          'cat "$RUNNER_TEMP/x" > "${RUNNER_TEMP}/key-guard.mjs"',
          'node "$RUNNER_TEMP/key-guard.mjs" --root trusted'
        ].join('\n')
      ),
      'scan: key step does not run a fresh copy of the committed key guard'
    ],
    [
      'a tee into the copy',
      keyStep(
        [
          FRESH_COPY,
          'cat "$RUNNER_TEMP/x" | tee -a "$RUNNER_TEMP/./key-guard.mjs"',
          'node "$RUNNER_TEMP/key-guard.mjs" --root trusted'
        ].join('\n')
      ),
      'scan: key step does not run a fresh copy of the committed key guard'
    ],
    [
      'a glob onto the copy',
      keyStep(
        [
          FRESH_COPY,
          'cat "$RUNNER_TEMP/x" > "$RUNNER_TEMP/key-guard.m"*',
          'node "$RUNNER_TEMP/key-guard.mjs" --root trusted'
        ].join('\n')
      ),
      'scan: key step redirects outside $RUNNER_TEMP: > "$RUNNER_TEMP/key-guard.m"*'
    ],
    [
      'a copy that honours replace refs',
      keyStep(
        [
          'git -C trusted cat-file blob HEAD:library/ui/figma/key-guard.mjs > "$RUNNER_TEMP/key-guard.mjs"',
          'node "$RUNNER_TEMP/key-guard.mjs" --root trusted'
        ].join('\n')
      ),
      'scan: key step does not run a fresh copy of the committed key guard'
    ],
    [
      'a git directory override on the key step',
      reader((job) => (job.steps[2]!.env!.GIT_DIR = '.preview/git')),
      'scan: sets GIT_DIR'
    ],
    [
      'git replace before the key step',
      otherStep('git -C trusted replace abc def'),
      'scan: rewrites a checkout with git'
    ],
    [
      'a fetch into refs/replace before the key step',
      otherStep('git -C trusted fetch origin "+$HEAD_SHA:refs/replace/abc"'),
      'scan: rewrites a checkout with git'
    ],
    [
      'git update-ref before the key step',
      otherStep('git -C trusted update-ref refs/heads/main "$HEAD_SHA"'),
      'scan: rewrites a checkout with git'
    ],
    [
      'git symbolic-ref before the key step',
      otherStep('git -C trusted symbolic-ref HEAD refs/key-scan/head'),
      'scan: rewrites a checkout with git'
    ],
    [
      'git config before the key step',
      otherStep('git -C trusted config core.hooksPath x'),
      'scan: rewrites a checkout with git'
    ],
    [
      'a redirect into .git of a root checkout',
      rootCheckout('echo x > .git/refs/replace/abc'),
      'scan: writes the checkout before the key step: echo x > .git/refs/replace/abc'
    ],
    [
      'a copy onto .git/HEAD of a root checkout',
      rootCheckout('cp "$RUNNER_TEMP/h" .git/HEAD'),
      'scan: writes the checkout before the key step: cp "$RUNNER_TEMP/h" .git/HEAD'
    ],
    [
      'a write into .github of a root checkout',
      rootCheckout('cp "$RUNNER_TEMP/x" ./.github/workflows/x.yml'),
      'scan: writes the checkout before the key step: cp "$RUNNER_TEMP/x" ./.github/workflows/x.yml'
    ],
    [
      'a copy into trusted/.git',
      otherStep('cp "$RUNNER_TEMP/h" trusted/.git/HEAD'),
      'scan: writes the checkout before the key step: cp "$RUNNER_TEMP/h" trusted/.git/HEAD'
    ],
    [
      'a tar --extract --directory DIR',
      otherStep('tar --extract -f "$RUNNER_TEMP/x.tar" --directory trusted'),
      'scan: writes the checkout before the key step: tar --extract -f "$RUNNER_TEMP/x.tar" --directory trusted'
    ],
    [
      'a tar --directory=DIR',
      otherStep('tar -xf "$RUNNER_TEMP/x.tar" --directory=trusted'),
      'scan: writes the checkout before the key step: tar -xf "$RUNNER_TEMP/x.tar" --directory=trusted'
    ],
    [
      'a tar -CDIR',
      otherStep('tar -xf "$RUNNER_TEMP/x.tar" -Ctrusted'),
      'scan: writes the checkout before the key step: tar -xf "$RUNNER_TEMP/x.tar" -Ctrusted'
    ],
    [
      'an unzip -dDIR',
      otherStep('unzip -o "$RUNNER_TEMP/x.zip" -dtrusted'),
      'scan: writes the checkout before the key step: unzip -o "$RUNNER_TEMP/x.zip" -dtrusted'
    ],
    [
      'a key step that runs the working tree guard',
      keyStep('node trusted/library/ui/figma/key-guard.mjs --root trusted'),
      'scan: key step does not run a fresh copy of the committed key guard'
    ],
    [
      'a key step that copies the guard after running node',
      keyStep(['node "$RUNNER_TEMP/key-guard.mjs"', FRESH_COPY].join('\\n')),
      'scan: key step does not run a fresh copy of the committed key guard'
    ],
    [
      'a key step that rewrites the copy',
      keyStep(
        [
          FRESH_COPY,
          'cat "$RUNNER_TEMP/x" > "$RUNNER_TEMP/key-guard.mjs"',
          'node "$RUNNER_TEMP/key-guard.mjs"'
        ].join('\\n')
      ),
      'scan: key step does not run a fresh copy of the committed key guard'
    ],
    [
      'a key step that copies the guard from another revision',
      keyStep(
        [
          'git -C trusted --no-replace-objects cat-file blob refs/key-scan/head:library/ui/figma/key-guard.mjs > "$RUNNER_TEMP/key-guard.mjs"',
          'node "$RUNNER_TEMP/key-guard.mjs"'
        ].join('\\n')
      ),
      'scan: key step does not run a fresh copy of the committed key guard'
    ],
    [
      'a key step that also runs another script',
      keyStep(
        [
          FRESH_COPY,
          'node "$RUNNER_TEMP/key-guard.mjs"',
          'node "$RUNNER_TEMP/pr-artifact/x.mjs"'
        ].join('\\n')
      ),
      'scan: key step does not run a fresh copy of the committed key guard'
    ],
    [
      'a reset to a fetched head',
      otherStep(
        'git -C trusted fetch origin "$HEAD_SHA" && git -C trusted reset --hard FETCH_HEAD'
      ),
      'scan: rewrites a checkout with git'
    ],
    [
      'a checkout of a bare sha',
      otherStep('git -C trusted checkout "$HEAD_SHA"'),
      'scan: rewrites a checkout with git'
    ],
    [
      'a read-tree into the checkout',
      otherStep('git -C trusted read-tree -u --reset "$HEAD_SHA"'),
      'scan: rewrites a checkout with git'
    ],
    [
      'a fetch into the checked-out branch',
      otherStep('git -C trusted fetch --update-head-ok origin "+$HEAD_SHA:main"'),
      'scan: rewrites a checkout with git'
    ],
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
      'a checkout into a parent path',
      reader((job) => (job.steps[0]!.with!.path = '../trusted')),
      'scan: checkout into a parent path'
    ],
    [
      'setup-node with a cache',
      reader((job) =>
        job.steps.push({
          uses: `actions/setup-node@${PINNED}`,
          with: { 'node-version': 24, cache: 'npm' }
        })
      ),
      'scan: actions/setup-node sets cache'
    ],
    [
      'an artifact over the trusted checkout',
      reader((job) =>
        job.steps.push({
          uses: `actions/download-artifact@${PINNED}`,
          with: { name: 'x', path: 'trusted/library/ui/figma' }
        })
      ),
      'scan: downloads an artifact into trusted/library/ui/figma'
    ],
    [
      'an artifact into the workspace root',
      reader((job) =>
        job.steps.push({ uses: `actions/download-artifact@${PINNED}`, with: { name: 'x' } })
      ),
      'scan: downloads an artifact into the workspace'
    ],
    [
      'an artifact into .github',
      reader((job) =>
        job.steps.push({
          uses: `actions/download-artifact@${PINNED}`,
          with: { name: 'x', path: '.github/actions' }
        })
      ),
      'scan: downloads an artifact into .github/actions'
    ],
    [
      'an artifact out of runner.temp',
      reader((job) =>
        job.steps.push({
          uses: `actions/download-artifact@${PINNED}`,
          with: { name: 'x', path: '${{ runner.temp }}/../work/trusted' }
        })
      ),
      'scan: downloads an artifact into ${{ runner.temp }}/../work/trusted'
    ],
    [
      'an artifact from another repository',
      reader((job) =>
        job.steps.push({
          uses: `actions/download-artifact@${PINNED}`,
          with: { name: 'x', path: '${{ runner.temp }}/x', repository: 'outside/fork' }
        })
      ),
      'scan: actions/download-artifact from outside/fork'
    ],
    [
      'an action step that maps the key',
      reader((job) =>
        job.steps.push({
          uses: `decentraland/oddish-action@${PINNED}`,
          env: { K: '${{ secrets.FIGMA_FILE_KEY }}' }
        })
      ),
      'scan: key in an action step'
    ],
    [
      'a key step that interpolates an expression',
      keyStep(
        'node trusted/library/ui/figma/key-guard.mjs --b ${{ github.event.workflow_run.head_branch }}'
      ),
      'scan: key step interpolates an expression'
    ],
    [
      'a key step that tees into a parent path',
      keyStep('node trusted/library/ui/figma/key-guard.mjs | tee "$RUNNER_TEMP/../work/x"'),
      'scan: key step uses a parent path'
    ],
    [
      'a key step that sets PATH',
      keyStep('PATH="$RUNNER_TEMP/pr-artifact:$PATH"\nnode trusted/library/ui/figma/key-guard.mjs'),
      'scan: key step sets PATH'
    ],
    [
      'a write to GITHUB_ENV',
      otherStep('echo "NODE_OPTIONS=--require ./x.js" >> "$GITHUB_ENV"'),
      'scan: writes GITHUB_ENV or GITHUB_PATH'
    ],
    [
      'a write to GITHUB_PATH',
      otherStep('echo "$RUNNER_TEMP/pr-artifact" >> $GITHUB_PATH'),
      'scan: writes GITHUB_ENV or GITHUB_PATH'
    ],
    [
      'a spaced secret reference',
      otherStep('echo ${{ secrets . FIGMA_FILE_KEY }}'),
      'scan: key outside a step env'
    ]
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
      // Every script is main's: from trusted/, or the key guard blob committed there, written out fresh.
      for (const [, script] of run.matchAll(/\bnode\s+(\S+)/g))
        expect(
          script!.startsWith('trusted/') || script === '"$RUNNER_TEMP/key-guard.mjs"',
          run
        ).toBe(true)
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
    expect(holders[0]!.run).toMatch(
      /^git -C trusted --no-replace-objects cat-file blob HEAD:library\/ui\/figma\/key-guard\.mjs > "\$RUNNER_TEMP\/key-guard\.mjs"\nnode "\$RUNNER_TEMP\/key-guard\.mjs" /m
    )
  })

  it('should run a key guard that is one self-contained file, so its committed blob is the whole program', () => {
    const source = readFileSync(path.join(repoRoot, 'library/ui/figma/key-guard.mjs'), 'utf8')
    const imports = [
      ...source.matchAll(/^\s*import\b[^'"]*['"]([^'"]+)['"]|\bimport\(\s*['"]([^'"]+)['"]/gm)
    ]
    expect(imports.length).toBeGreaterThan(0)
    for (const [, from, dynamic] of imports) expect(from ?? dynamic).toMatch(/^node:/)
    expect(source).not.toMatch(/\brequire\(|\bimport\(\s*[^'"\s]/)
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
