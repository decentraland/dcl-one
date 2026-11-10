import { createHash } from 'node:crypto'
import { execFileSync, spawnSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { chmod, link, mkdir, mkdtemp, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { parse } from 'yaml'
import {
  enumerateArtifact,
  listLfsObjects,
  main,
  redact,
  runGuard,
  scanBuffer
} from './key-guard.mjs'

const FAKE_KEY = 'FAKEKEY000example000000'
// Built by concatenation so the tree never holds a literal Figma file URL (the guard's own `url`).
const FILE_URL = 'https://www.figma' + '.com/design/AbC123/Design?node-id=1-2'

let root: string

const git = (...args: string[]) =>
  execFileSync('git', args, { cwd: root, stdio: 'pipe' }).toString().trim()

async function put(file: string, content: string) {
  await mkdir(path.dirname(path.join(root, file)), { recursive: true })
  await writeFile(path.join(root, file), content)
}

function commit(message: string) {
  git('add', '-A')
  git(
    '-c',
    'user.email=dev@example.com',
    '-c',
    'user.name=Dev',
    '-c',
    'commit.gpgsign=false',
    'commit',
    '-qm',
    message
  )
}

function guard(options: {
  range?: string | null
  worktree?: boolean
  paths?: string[]
  trees?: { rev: string; prefix: string }[]
  artifacts?: string[]
  artifactLimits?: { files: number; totalBytes: number; fileBytes: number; depth: number }
  allowlistRev?: string
  key?: string
}) {
  return runGuard({
    root,
    range: options.range === undefined ? 'base..HEAD' : options.range,
    worktree: options.worktree ?? true,
    paths: options.paths ?? [],
    trees: options.trees,
    artifacts: options.artifacts,
    artifactLimits: options.artifactLimits,
    allowlistRev: options.allowlistRev,
    key: options.key
  })
}

beforeEach(async () => {
  root = await mkdtemp(path.join(tmpdir(), 'key-guard-'))
  git('init', '-q', '-b', 'main')
  git('config', 'user.name', 'Fixture Dev')
  git('config', 'user.email', 'fixture@example.com')
  await put('README.md', 'clean\n')
  commit('base')
  git('tag', 'base')
})

afterEach(async () => {
  await rm(root, { recursive: true, force: true })
})

// Every form is built by concatenation, like FILE_URL, so the tree never holds a literal match.
const HOST = 'figma' + '.com'
describe('when matching Figma URLs for the url check', () => {
  it.each([
    ['a design link', `https://www.${HOST}/design/AbC123/Name`],
    ['an uppercase host', `https://${HOST.toUpperCase()}/design/AbC123`],
    ['a slides link', `https://${HOST}/slides/AbC123/Deck`],
    ['a make link', `https://${HOST}/make/AbC123/App`],
    ['a site link', `https://${HOST}/site/AbC123`],
    ['a deck link', `https://${HOST}/deck/AbC123`],
    [
      'a percent-encoded embed link',
      `https://embed.${HOST}/?url=https%3A%2F%2Fwww.${HOST}%2Fdesign%2FAbC123`
    ],
    ['a lowercase percent-encoded link', `${HOST}%2fproto%2fAbC123`],
    ['a JSON-escaped link', `"https:\\/\\/www.${HOST}\\/design\\/AbC123"`],
    ['a unicode JSON separator', `${HOST}\\u002fdesign\\u002fAbC123`],
    ['a doubly escaped link', `${HOST}\\\\/board\\\\/AbC123`]
  ])('should flag %s', (_, text) => {
    expect(scanBuffer(Buffer.from(text))).toEqual(['url'])
  })

  it.each([
    ['ordinary text', 'Designs live in Figma; ask the owner for access.'],
    ['the bare host', `See ${HOST} for pricing.`],
    ['a single path segment', `https://${HOST}/pricing`],
    ['a host that only ends in figma.com', `https://not${HOST}/design/AbC123`],
    ['another site with the same path', 'https://example.com/design/AbC123/Name'],
    ['a figma package name', '@figma/code-connect/figma-types'],
    ['a REST API path', `https://api.${HOST}/v1/files/AbC123/nodes`]
  ])('should not flag %s', (_, text) => {
    expect(scanBuffer(Buffer.from(text))).toEqual([])
  })

  it('should honour an allowlisted id in any of these forms', () => {
    const text = `${HOST}%2Fdesign%2FAbC123 and https://${HOST.toUpperCase()}/file/AbC123`
    expect(scanBuffer(Buffer.from(text), undefined, new Set(['AbC123']))).toEqual([])
  })
})

describe('figma-key-guard', () => {
  describe('when nothing carries the key or a Figma file URL', () => {
    it('should pass and say which checks ran', () => {
      const { findings, lines } = guard({ key: FAKE_KEY })

      expect(findings).toEqual([])
      expect(lines).toContain('check key: ran')
      expect(lines.at(-1)).toBe('result: PASS')
    })
  })

  describe('when the key is not configured', () => {
    it('should report the key check as skipped, never as passed', () => {
      const { lines } = guard({})

      expect(lines).toContain('check key: SKIPPED (FIGMA_FILE_KEY not set)')
    })
  })

  describe('when a working-tree file holds the key and another a Figma file URL', () => {
    it('should report path and check ID only', async () => {
      await put('src/leak.ts', `const k = '${FAKE_KEY}'\n`)
      await put('docs/link.md', `see ${FILE_URL}\n`)

      const { findings, lines } = guard({ key: FAKE_KEY, range: null })

      expect(findings).toEqual(
        expect.arrayContaining([
          { check: 'key', location: 'src/leak.ts' },
          { check: 'url', location: 'docs/link.md' }
        ])
      )
      expect(lines.join('\n')).not.toContain(FAKE_KEY)
      expect(lines.join('\n')).not.toContain('AbC123')
    })
  })

  describe('when a commit added the key and a later commit removed it', () => {
    it('should still fail on the commit that carried it', async () => {
      await put('config.json', `{"fileKey":"${FAKE_KEY}"}\n`)
      commit('add key')
      const leaking = git('rev-parse', '--short=12', 'HEAD')
      await put('config.json', '{}\n')
      commit('remove key')

      const { findings, lines } = guard({ key: FAKE_KEY, worktree: false })

      expect(findings).toEqual([{ check: 'key', location: `${leaking}:config.json` }])
      expect(lines.join('\n')).not.toContain(FAKE_KEY)
    })
  })

  describe('when a generated artifact directory holds the key', () => {
    it('should find it there although the directory is ignored by git', async () => {
      await put('.gitignore', 'review-dist\n')
      await put('review-dist/report.json', `{"source":"${FAKE_KEY}"}\n`)

      const { findings } = guard({ key: FAKE_KEY, range: null, paths: ['review-dist'] })

      expect(findings).toEqual([{ check: 'key', location: 'review-dist/report.json' }])
    })
  })

  describe('when a scanned file is an unmaterialized Git LFS pointer', () => {
    it('should fail with the lfs check instead of scanning the pointer', async () => {
      await put(
        'figma/snapshot.json',
        'version https://git-lfs.github.com/spec/v1\noid sha256:00\nsize 1\n'
      )

      const { findings } = guard({ key: FAKE_KEY, range: null })

      expect(findings).toEqual([{ check: 'lfs', location: 'figma/snapshot.json' }])
    })
  })

  describe('when a public file id is allowlisted for the url check in one path', () => {
    const otherUrl = 'https://www.figma' + '.com/design/ZzOther9/Design?node-id=3-4'

    beforeEach(async () => {
      await put(
        'library/ui/figma/key-guard-allow.json',
        JSON.stringify([
          { path: 'docs/public.md', check: 'url', id: 'AbC123', reason: 'public file' },
          { path: 'docs/unpinned.md', check: 'url', reason: 'no id: never honored' },
          { path: 'docs/keyed.md', check: 'key', id: 'KeyedId1', reason: 'never honored' }
        ])
      )
    })

    it('should accept that file id in that path only', async () => {
      await put('docs/public.md', `see ${FILE_URL}\n`)
      await put('docs/other.md', `see ${FILE_URL}\n`)

      const { findings } = guard({ key: FAKE_KEY, range: null })

      expect(findings).toEqual([{ check: 'url', location: 'docs/other.md' }])
    })

    it('should fail a different Figma URL added to the allowlisted file', async () => {
      await put('docs/public.md', `see ${FILE_URL}\nand ${otherUrl}\n`)

      const { findings, lines } = guard({ key: FAKE_KEY, range: null })

      expect(findings).toEqual([{ check: 'url', location: 'docs/public.md' }])
      expect(lines.join('\n')).not.toContain('ZzOther9')
    })

    it('should fail a different Figma URL in the allowlisted file with no key configured', async () => {
      await put('docs/public.md', `see ${FILE_URL}\nand ${otherUrl}\n`)

      const { findings, lines } = guard({ range: null })

      expect(lines).toContain('check key: SKIPPED (FIGMA_FILE_KEY not set)')
      expect(findings).toEqual([{ check: 'url', location: 'docs/public.md' }])
    })

    it('should ignore an entry that pins no file id', async () => {
      await put('docs/unpinned.md', `see ${FILE_URL}\n`)

      const { findings } = guard({ key: FAKE_KEY, range: null })

      expect(findings).toEqual([{ check: 'url', location: 'docs/unpinned.md' }])
    })

    it('should still fail the key check, which cannot be allowlisted', async () => {
      await put('docs/public.md', `key ${FAKE_KEY}\n`)
      await put('docs/keyed.md', `key ${FAKE_KEY}\n`)

      const { findings } = guard({ key: FAKE_KEY, range: null })

      expect(findings).toEqual(
        expect.arrayContaining([
          { check: 'key', location: 'docs/public.md' },
          { check: 'key', location: 'docs/keyed.md' }
        ])
      )
    })
  })
})

// A trusted copy of the guard scans a pull request as data: its commits sit in the object store under a
// private ref, nothing of it is checked out, and its own allowlist is read from that ref.
describe('when scanning another revision from the object store', () => {
  let leaking: string

  beforeEach(async () => {
    git('checkout', '-q', '-b', 'pr')
    await put('library/ui/data.json', `{"file": "${FAKE_KEY}"}\n`)
    await put('docs/outside.md', `key ${FAKE_KEY}\n`)
    commit('pr adds the key')
    leaking = git('rev-parse', '--short=12', 'HEAD')
    git('checkout', '-q', 'main')
  })

  it('should fail on a blob under the scanned path, naming revision and path only', () => {
    const { findings, lines } = guard({
      worktree: false,
      range: null,
      trees: [{ rev: 'pr', prefix: 'library/ui' }],
      key: FAKE_KEY
    })

    expect(findings).toEqual([{ check: 'key', location: `${leaking}:library/ui/data.json` }])
    expect(lines).toContain(`tree pr:library/ui (${leaking}): 1 files`)
    expect(lines.join('\n')).not.toContain(FAKE_KEY)
  })

  it('should read nothing from the working tree, which still holds the base', async () => {
    await put('library/ui/local.json', `${FAKE_KEY}\n`)
    git('checkout', '-q', '-b', 'clean', 'base')

    const { findings } = guard({
      worktree: false,
      range: null,
      trees: [{ rev: 'clean', prefix: 'library/ui' }],
      key: FAKE_KEY
    })

    expect(findings).toEqual([])
  })

  it('should take the url allowlist from the scanned revision when asked to', async () => {
    git('checkout', '-q', 'pr')
    await put('docs/public.md', `see ${FILE_URL}\n`)
    await put(
      'library/ui/figma/key-guard-allow.json',
      JSON.stringify([
        { path: 'docs/public.md', check: 'url', id: 'AbC123', reason: 'public file' }
      ])
    )
    commit('pr allowlists a public file')
    git('checkout', '-q', 'main')
    const scan = (allowlistRev?: string) =>
      guard({
        worktree: false,
        range: null,
        trees: [{ rev: 'pr', prefix: 'docs/public.md' }],
        allowlistRev
      })

    expect(scan().findings.map((finding) => finding.check)).toEqual(['url'])
    expect(scan('pr').findings).toEqual([])
  })

  it('should reject a --tree value without a path', () => {
    expect(main(['--root', root, '--no-range', '--no-worktree', '--tree', 'pr'])).toBe(2)
  })
})

// The unpacked Review artifact is adversarial input: the enumerator reads it without following a link and
// fails closed on anything unexpected.
describe('when enumerating an unpacked artifact', () => {
  const limits = { files: 5, totalBytes: 1000, fileBytes: 100, depth: 3 }
  let outside: string

  beforeEach(async () => {
    outside = await mkdtemp(path.join(tmpdir(), 'key-guard-outside-'))
    await writeFile(path.join(outside, 'secret.txt'), `${FAKE_KEY}\n`)
    await put('artifact/index.html', '<p>ok</p>')
    await put('artifact/assets/app.js', 'console.log(1)')
  })

  afterEach(async () => {
    await rm(outside, { recursive: true, force: true })
  })

  const enumerate = (overrides = {}) =>
    enumerateArtifact(path.join(root, 'artifact'), { ...limits, ...overrides })

  it('should list a valid tree with its relative paths and no problem', () => {
    const { files, problems } = enumerate()

    expect(files.map((file) => file.path).sort()).toEqual(['assets/app.js', 'index.html'])
    expect(problems).toEqual([])
  })

  it('should reject a symlink to a file without following it', async () => {
    await symlink(path.join(outside, 'secret.txt'), path.join(root, 'artifact/link.txt'))

    const { files, problems } = enumerate()

    expect(problems).toEqual([{ path: 'link.txt', reason: 'symlink' }])
    expect(files.map((file) => file.path)).not.toContain('link.txt')
  })

  it('should reject a symlinked directory that escapes the artifact', async () => {
    await symlink(outside, path.join(root, 'artifact/assets/out'))

    const { files, problems } = enumerate()

    expect(problems).toEqual([{ path: path.join('assets', 'out'), reason: 'symlink' }])
    expect(files.map((file) => file.path).join()).not.toContain('secret.txt')
  })

  it('should reject a hard link', async () => {
    await link(path.join(outside, 'secret.txt'), path.join(root, 'artifact/hard.txt'))

    expect(enumerate().problems).toEqual([{ path: 'hard.txt', reason: 'hard link' }])
  })

  it('should reject a file over the per-file limit', async () => {
    await put('artifact/big.bin', 'x'.repeat(101))

    expect(enumerate().problems).toEqual([{ path: 'big.bin', reason: 'file too large' }])
  })

  it('should stop listing past the file count limit and say so once', async () => {
    for (const name of ['a', 'b', 'c', 'd', 'e']) await put(`artifact/${name}.txt`, name)

    const { files, problems } = enumerate()

    expect(files).toHaveLength(5)
    expect(problems.map((problem) => problem.reason)).toEqual(['too many files'])
  })

  it('should reject a tree that is too large in total', async () => {
    for (const name of ['a', 'b', 'c']) await put(`artifact/${name}.txt`, 'x'.repeat(100))

    expect(enumerate({ totalBytes: 250 }).problems.map((problem) => problem.reason)).toEqual([
      'too large in total'
    ])
  })

  it('should reject a tree that is too deep', async () => {
    await put('artifact/a/b/c/d/deep.txt', 'x')

    expect(enumerate().problems).toEqual([
      { path: path.join('a', 'b', 'c', 'd'), reason: 'too deep' }
    ])
  })

  it('should fail the guard on a problem and never read through a link', async () => {
    await symlink(path.join(outside, 'secret.txt'), path.join(root, 'artifact/link.txt'))

    const { findings, lines } = guard({
      worktree: false,
      range: null,
      artifacts: ['artifact'],
      artifactLimits: limits,
      key: FAKE_KEY
    })

    expect(findings).toEqual([{ check: 'artifact', location: 'link.txt (symlink)' }])
    expect(lines.join('\n')).not.toContain(FAKE_KEY)
  })

  it('should still find the key in a regular file of the artifact', async () => {
    await put('artifact/assets/leak.js', `const k = '${FAKE_KEY}'`)

    const { findings } = guard({
      worktree: false,
      range: null,
      artifacts: ['artifact'],
      artifactLimits: limits,
      key: FAKE_KEY
    })

    expect(findings).toEqual([{ check: 'key', location: path.join('assets', 'leak.js') }])
  })
})

// The scanner runs from main over a pull request's data, and the pull request chooses its file names. A path
// named after the key must never reach the report, the check summary (the scanner's stdout) or the log.
describe('when an untrusted path carries the private key', () => {
  const SCRIPT = path.resolve(import.meta.dirname, 'key-guard.mjs')

  // The scanner as the workflow runs it, with the key in its environment. Both streams are returned whole.
  function scanner(...args: string[]) {
    const result = spawnSync(process.execPath, [SCRIPT, '--root', root, ...args], {
      env: { ...process.env, FIGMA_FILE_KEY: FAKE_KEY },
      encoding: 'utf8'
    })
    return { status: result.status, stdout: result.stdout, stderr: result.stderr }
  }

  const expectNoKey = (result: { stdout: string; stderr: string }) => {
    expect(result.stdout).not.toContain(FAKE_KEY)
    expect(result.stderr).not.toContain(FAKE_KEY)
    expect(`${result.stdout}${result.stderr}`.toLowerCase()).not.toContain(FAKE_KEY.toLowerCase())
  }

  it('should redact a key-named path and content found in a tree read from the object store', async () => {
    git('checkout', '-q', '-b', 'pr')
    await put(`library/ui/${FAKE_KEY}.json`, `{"key": "${FAKE_KEY}"}\n`)
    commit('pr adds a key-named file')
    git('checkout', '-q', 'main')

    const result = scanner('--no-worktree', '--no-range', '--tree', 'pr:library/ui')

    expect(result.status).toBe(1)
    expect(result.stdout).toMatch(/^FAIL key \w{12}:library\/ui\/<DCL_FIGMA_FILE>\.json$/m)
    expectNoKey(result)
  })

  it('should redact an artifact file name, a link named after the key, and the content it holds', async () => {
    await put(`artifact/${FAKE_KEY}.js`, `const k = '${FAKE_KEY}'`)
    await symlink(root, path.join(root, `artifact/${FAKE_KEY}-link`))

    const result = scanner('--no-worktree', '--no-range', '--artifact', 'artifact')

    expect(result.status).toBe(1)
    expect(result.stdout).toContain('FAIL key <DCL_FIGMA_FILE>.js')
    expect(result.stdout).toContain('FAIL artifact <DCL_FIGMA_FILE>-link (symlink)')
    expectNoKey(result)
  })

  it('should redact a path in a commit of the scanned range', async () => {
    await put(`docs/${FAKE_KEY}.md`, `key ${FAKE_KEY}\n`)
    commit('touch a key-named path')

    const result = scanner('--no-worktree', '--range', 'base..HEAD')

    expect(result.status).toBe(1)
    expect(result.stdout).toMatch(/^FAIL key \w{12}:docs\/<DCL_FIGMA_FILE>\.md$/m)
    expectNoKey(result)
  })

  it('should redact the path an exception would echo', () => {
    const result = scanner('--no-worktree', '--no-range', '--artifact', `${FAKE_KEY}-missing`)

    expect(result.status).toBe(2)
    expect(result.stderr).toContain('<DCL_FIGMA_FILE>-missing')
    expectNoKey(result)
  })

  it('should redact the revision a failing git command would quote', () => {
    const result = scanner('--no-worktree', '--no-range', '--tree', `${FAKE_KEY}:library/ui`)

    expect(result.status).toBe(2)
    expectNoKey(result)
  })

  it('should capture child-process stderr when an allowlist revision contains the key', () => {
    const result = scanner('--no-worktree', '--no-range', '--allowlist-from', FAKE_KEY)

    expect(result.status).toBe(0)
    expectNoKey(result)
    expect(result.stderr).not.toContain('fatal:')
  })

  // The workflow runs a copy of the guard read from TRUSTED_SHA; the summary names that commit, not HEAD.
  it('should name the revision it was given instead of the checkout HEAD', async () => {
    const pinned = git('rev-parse', 'base')
    await put('later.md', 'later\n')
    commit('later')
    const result = scanner('--no-worktree', '--no-range', '--revision', 'base')

    expect(result.status, result.stderr).toBe(0)
    expect(result.stdout.split('\n')[0]).toBe(`figma-key-guard at ${pinned}`)
    expect(result.stdout).not.toContain(git('rev-parse', 'HEAD'))
  })

  it('should name the checkout HEAD without --revision', () => {
    const result = scanner('--no-worktree', '--no-range')

    expect(result.stdout.split('\n')[0]).toBe(`figma-key-guard at ${git('rev-parse', 'HEAD')}`)
  })

  it.each([
    [['--revision'], '--revision takes a commit'],
    [['--revision', '--no-range'], '--revision takes a commit'],
    [['--revision', ''], '--revision takes a commit'],
    [['--revision', 'no-such-commit'], 'figma-key-guard: Command failed']
  ])('should refuse to run with %j', (args, message) => {
    const result = scanner('--no-worktree', '--no-range', ...args)

    expect(result.status).toBe(2)
    expect(result.stdout).not.toContain('check key: ran')
    expect(result.stderr).toContain(message)
  })

  it('should redact a revision that carries the key', () => {
    const result = scanner('--no-worktree', '--no-range', '--revision', FAKE_KEY)

    expect(result.status).toBe(2)
    expectNoKey(result)
  })

  it('should redact an argument it rejects', () => {
    const result = scanner(`--${FAKE_KEY}`)

    expect(result.status).toBe(2)
    expect(result.stderr).toContain('unknown argument: --<DCL_FIGMA_FILE>')
    expectNoKey(result)
  })

  it('should redact the key in any letter case, several times, and leave other text alone', () => {
    expect(redact(`a/${FAKE_KEY}/b/${FAKE_KEY.toLowerCase()}.json`, FAKE_KEY)).toBe(
      'a/<DCL_FIGMA_FILE>/b/<DCL_FIGMA_FILE>.json'
    )
    expect(redact('plain/path.json', FAKE_KEY)).toBe('plain/path.json')
    expect(redact('no key set', undefined)).toBe('no key set')
  })

  it('should return findings and report lines that never carry the key', async () => {
    await put(`docs/${FAKE_KEY}.md`, `key ${FAKE_KEY}\n`)

    const { findings, lines } = guard({ key: FAKE_KEY, range: null })

    expect(JSON.stringify(findings)).not.toContain(FAKE_KEY)
    expect(lines.join('\n')).not.toContain(FAKE_KEY)
  })

  it('should print only through the one redacting output function', async () => {
    const { readFile } = await import('node:fs/promises')
    const source = await readFile(SCRIPT, 'utf8')
    const outside = source.replace(/\/\/ output:begin[\s\S]*?\/\/ output:end/, '')

    expect(source).toContain('// output:begin')
    expect(outside).not.toMatch(/console\.|process\.(stdout|stderr)\.write|\.write\(/)
  })
})

// Exercise the actual trusted workflow scripts with GitHub controller metadata supplied as data.
describe('when selecting a main push range', () => {
  const workflow = () =>
    parse(
      readFileSync(
        path.resolve(import.meta.dirname, '../../../.github/workflows/ui-library-key-scan.yml'),
        'utf8'
      )
    ) as { jobs: Record<string, { steps: { run?: string; id?: string }[] }> }

  type Job = { run_id: number; head_sha: string; conclusion: string; name: string }

  // The jobs endpoint returns JSON and the workflow filters it with --jq: the mock applies that exact filter with jq,
  // so the trust conditions (run id, head, conclusion, name prefix) run in the test.
  async function boundary(head: string, name: string, decoys: Job[] = []) {
    const output = path.join(root, 'workflow-output')
    await writeFile(output, '')
    const genuine = name
      ? name
          .split('\n')
          .map((line) => ({ run_id: 123, head_sha: head, conclusion: 'success', name: line }))
      : []
    const mock = `gh() {
      local filter='' previous=''
      for argument in "$@"; do
        [ "$previous" = --jq ] && filter="$argument"
        previous="$argument"
      done
      case "$*" in
        *actions/workflows/ui-library.yml*) echo 42 ;;
        */jobs*) printf '%s' "$JOBS_JSON" | jq -r "$filter" ;;
        */artifacts*) echo 1 ;;
        *) return 1 ;;
      esac
    }\n`
    const result = spawnSync('bash', ['-e', '-c', mock + workflow().jobs.prepare!.steps[0]!.run!], {
      cwd: root,
      encoding: 'utf8',
      env: {
        ...process.env,
        WORKFLOW_PATH: '.github/workflows/ui-library.yml',
        WORKFLOW_ID: '42',
        REPOSITORY: 'example/repository',
        HEAD_REPO: 'example/repository',
        RUN_ID: '123',
        EVENT: 'push',
        CONCLUSION: 'success',
        HEAD_SHA: head,
        HEAD_BRANCH: 'main',
        DEFAULT_BRANCH: 'main',
        JOBS_JSON: JSON.stringify({ jobs: [...decoys, ...genuine] }),
        GITHUB_OUTPUT: output
      }
    })
    const text = readFileSync(output, 'utf8')
    return { ...result, output: text, base: /^base=(.*)$/m.exec(text)?.[1] }
  }

  it('should cover an add-then-delete two-commit push from its actual boundary', async () => {
    const before = git('rev-parse', 'HEAD')
    await put('library/ui/temporary.txt', FAKE_KEY)
    commit('introduce synthetic key')
    const leaked = git('rev-parse', '--short=12', 'HEAD')
    await rm(path.join(root, 'library/ui/temporary.txt'))
    await put('library/ui/clean.txt', 'clean tip\n')
    commit('remove synthetic key')
    const head = git('rev-parse', 'HEAD')
    const result = await boundary(head, `UI key push boundary ${before}..${head}`)

    expect(result.status, result.stderr).toBe(0)
    expect(result.base).toBe(before)
    const { findings } = guard({ worktree: false, range: `${result.base}..${head}`, key: FAKE_KEY })
    expect(findings).toContainEqual({
      check: 'key',
      location: `${leaked}:library/ui/temporary.txt`
    })
  })

  it.each(['missing', 'malformed', 'wrong head', 'duplicate'])(
    'should fail closed for %s push metadata',
    async (kind) => {
      const head = git('rev-parse', 'HEAD')
      const valid = `UI key push boundary ${head}..${head}`
      const name =
        kind === 'missing'
          ? ''
          : kind === 'malformed'
            ? `UI key push boundary invalid..${head}`
            : kind === 'wrong head'
              ? `UI key push boundary ${head}..${'f'.repeat(40)}`
              : `${valid}\n${valid}`
      const result = await boundary(head, name)

      expect(result.status).not.toBe(0)
      expect(result.output).not.toContain('scan=true')
    }
  )

  describe('when the jobs list holds boundary-named jobs the trust filter must reject', () => {
    const decoysFor = (head: string, other: string): Job[] => [
      {
        run_id: 999,
        head_sha: head,
        conclusion: 'success',
        name: `UI key push boundary ${other}..${head}`
      },
      {
        run_id: 123,
        head_sha: other,
        conclusion: 'success',
        name: `UI key push boundary ${other}..${head}`
      },
      {
        run_id: 123,
        head_sha: head,
        conclusion: 'failure',
        name: `UI key push boundary ${other}..${head}`
      },
      {
        run_id: 123,
        head_sha: head,
        conclusion: 'success',
        name: `Decoy UI key push boundary ${other}..${head}`
      }
    ]

    it('should fail closed when only another run, another head, a failed job or an unprefixed name carries one', async () => {
      const head = git('rev-parse', 'HEAD')
      const result = await boundary(head, '', decoysFor(head, 'a'.repeat(40)))

      expect(result.status).not.toBe(0)
      expect(result.output).not.toContain('scan=true')
    })

    it('should take the base from the one trusted job and ignore the decoys', async () => {
      const before = git('rev-parse', 'HEAD')
      await put('library/ui/clean.txt', 'clean tip\n')
      commit('advance the pushed branch')
      const head = git('rev-parse', 'HEAD')
      const result = await boundary(
        head,
        `UI key push boundary ${before}..${head}`,
        decoysFor(head, 'a'.repeat(40))
      )

      expect(result.status, result.stderr).toBe(0)
      expect(result.base).toBe(before)
    })
  })

  it('should include the root commit when the first push has no previous head', async () => {
    await put('README.md', FAKE_KEY)
    git('add', '-A')
    git(
      '-c',
      'user.email=dev@example.com',
      '-c',
      'user.name=Dev',
      '-c',
      'commit.gpgsign=false',
      'commit',
      '--amend',
      '-qm',
      'synthetic key in root'
    )
    const leaked = git('rev-parse', '--short=12', 'HEAD')
    await put('README.md', 'clean\n')
    commit('remove key from tip')
    const head = git('rev-parse', 'HEAD')
    const result = await boundary(head, `UI key push boundary ${'0'.repeat(40)}..${head}`)
    expect(result.status, result.stderr).toBe(0)
    const scan = workflow().jobs['key-scan']!.steps.find((step) => step.id === 'scan')!.run!
    const args = path.join(root, 'scanner-args')
    const selected = spawnSync(
      'bash',
      [
        '-e',
        '-c',
        // This repository stands in for trusted/, which already holds the commits: neither the step's fresh copy
        // of the guard nor its bundle fetch is needed here.
        `git() { case " $* " in *' cat-file '*|*' fetch '*) return 0;; esac; command git "$@"; }\n` +
          `node() { printf '%s\\n' "$@" > "$ARGS"; echo 'check key: ran'; }\n` +
          scan
      ],
      {
        cwd: root,
        encoding: 'utf8',
        env: {
          ...process.env,
          BASE: result.base!,
          HEAD_SHA: head,
          RUNNER_TEMP: root,
          ARGS: args,
          FIGMA_FILE_KEY: FAKE_KEY
        }
      }
    )
    expect(selected.status, selected.stderr).toBe(0)
    const argv = readFileSync(args, 'utf8').trim().split('\n')
    const range = argv[argv.indexOf('--range') + 1]!
    expect(range).toBe(head)
    expect(guard({ worktree: false, range, key: FAKE_KEY }).findings).toContainEqual({
      check: 'key',
      location: `${leaked}:README.md`
    })
  })
})

describe('when a merge includes upstream changes', () => {
  it('should scan a conflicting merge resolution without relisting clean upstream merges', async () => {
    await put('conflict.txt', 'base\n')
    commit('shared')
    git('tag', 'shared')
    git('checkout', '-qb', 'upstream')
    await put('conflict.txt', 'upstream\n')
    await put('upstream-only.txt', 'clean')
    commit('upstream')
    git('checkout', 'main')
    await put('conflict.txt', 'branch\n')
    commit('branch')
    const conflict = spawnSync(
      'git',
      ['-c', 'commit.gpgsign=false', 'merge', '--no-ff', 'upstream'],
      { cwd: root, encoding: 'utf8' }
    )
    expect(conflict.status).toBe(1)
    expect(conflict.stdout).toContain('CONFLICT')
    expect(git('rev-parse', 'MERGE_HEAD')).toBe(git('rev-parse', 'upstream'))
    await put('conflict.txt', FAKE_KEY)
    commit('merge resolution')
    expect(git('show', '-s', '--format=%P', 'HEAD').split(' ')).toHaveLength(2)
    const merged = git('rev-parse', '--short=12', 'HEAD')
    expect(
      guard({ range: 'upstream..HEAD', worktree: false, key: FAKE_KEY }).findings
    ).toContainEqual({ check: 'key', location: `${merged}:conflict.txt` })
    await put('conflict.txt', 'clean\n')
    commit('clear')
    git('checkout', 'upstream')
    await put('only-main.txt', 'clean')
    commit('more upstream')
    git('checkout', 'main')
    git(
      '-c',
      'user.name=Dev',
      '-c',
      'user.email=dev@example.com',
      '-c',
      'commit.gpgsign=false',
      'merge',
      '--no-ff',
      'upstream',
      '-m',
      'clean merge'
    )
    expect(
      guard({ range: 'HEAD^1..HEAD', worktree: false, key: FAKE_KEY }).lines.join('\n')
    ).toMatch(/1 blobs/)
  })
})
describe('when build output holds an LFS pointer', () => {
  it('should fail closed without attempting any LFS materialization', async () => {
    await put(
      'artifact/pointer',
      'version https://git-lfs.github.com/spec/v1\noid sha256:' + 'f'.repeat(64) + '\nsize 1\n'
    )
    expect(
      guard({ range: null, worktree: false, artifacts: ['artifact'], key: FAKE_KEY }).findings
    ).toEqual([{ check: 'lfs', location: 'pointer' }])
  })
})

describe('when an artifact or a range contains a pointer whose object is not local', () => {
  it('should fail without invoking the LFS client or downloading content', async () => {
    const actualGit = execFileSync('which', ['git'], { encoding: 'utf8' }).trim()
    const bin = path.join(root, 'bin')
    await mkdir(bin)
    const marker = path.join(root, 'download-called')
    const wrapper = path.join(bin, 'git')
    await writeFile(
      wrapper,
      '#!' +
        process.execPath +
        '\n' +
        `const {execFileSync}=require('node:child_process');const {writeFileSync}=require('node:fs');if(process.argv[2]==='lfs'){writeFileSync(${JSON.stringify(marker)},'called');process.exit(97)}process.stdout.write(execFileSync(${JSON.stringify(actualGit)},process.argv.slice(2)));`
    )
    await chmod(wrapper, 0o755)
    await put(
      'outside/data',
      'version https://git-lfs.github.com/spec/v1\noid sha256:' + 'f'.repeat(64) + '\nsize 1\n'
    )
    commit('pointer')
    const previous = process.env.PATH
    process.env.PATH = bin + path.delimiter + previous
    try {
      expect(
        guard({ range: 'base..HEAD', worktree: false, artifacts: ['outside'] }).findings.map(
          (item) => item.check
        )
      ).toEqual(['lfs', 'lfs'])
      expect(() => readFileSync(marker)).toThrow()
    } finally {
      process.env.PATH = previous
    }
  })
})

describe('when a historical UI pointer has been prefetched', () => {
  it('should read its verified local content and preserve leak detection', async () => {
    const content = Buffer.from(FAKE_KEY)
    const oid = createHash('sha256').update(content).digest('hex')
    await put(`.git/lfs/objects/${oid.slice(0, 2)}/${oid.slice(2, 4)}/${oid}`, FAKE_KEY)
    await put(
      'library/ui/input.json',
      `version https://git-lfs.github.com/spec/v1\noid sha256:${oid}\nsize ${content.length}\n`
    )
    commit('prefetched pointer')
    expect(guard({ worktree: false, key: FAKE_KEY }).findings).toContainEqual({
      check: 'key',
      location: `${git('rev-parse', '--short=12', 'HEAD')}:library/ui/input.json`
    })
  })
})

// A pointer whose payload is stored in the local LFS store, as lfs-prefetch.mjs leaves it.
const POINTER_PREFIX = 'version https://git-lfs.github.com/spec/v1\noid sha256:'
async function storeObject(content: string) {
  const oid = createHash('sha256').update(content).digest('hex')
  await put(`.git/lfs/objects/${oid.slice(0, 2)}/${oid.slice(2, 4)}/${oid}`, content)
  return `${POINTER_PREFIX}${oid}\nsize ${Buffer.byteLength(content)}\n`
}
const short = (rev: string) => git('rev-parse', '--short=12', rev)

describe('when the range holds an LFS pointer outside library/ui', () => {
  it('should pass a clean payload read from the local store', async () => {
    await put('apps/sites/public/clean.bin', await storeObject('clean payload'))
    commit('clean pointer')
    expect(guard({ worktree: false, key: FAKE_KEY }).findings).toEqual([])
  })

  it('should fail a payload that holds the key', async () => {
    await put('apps/sites/public/leak.bin', await storeObject(`prefix ${FAKE_KEY} suffix`))
    commit('leaking pointer')
    expect(guard({ worktree: false, key: FAKE_KEY }).findings).toEqual([
      { check: 'key', location: `${short('HEAD')}:apps/sites/public/leak.bin` }
    ])
  })

  it('should fail a payload that holds a Figma URL', async () => {
    await put('apps/sites/public/link.bin', await storeObject(FILE_URL))
    commit('linking pointer')
    expect(guard({ worktree: false }).findings).toEqual([
      { check: 'url', location: `${short('HEAD')}:apps/sites/public/link.bin` }
    ])
  })

  it('should still scan a leaking payload that a later commit of the range deletes', async () => {
    await put('apps/sites/public/transient.bin', await storeObject(FAKE_KEY))
    commit('add leaking pointer')
    const added = short('HEAD')
    await rm(path.join(root, 'apps/sites/public/transient.bin'))
    commit('delete it')
    expect(guard({ worktree: false, key: FAKE_KEY }).findings).toEqual([
      { check: 'key', location: `${added}:apps/sites/public/transient.bin` }
    ])
  })

  it('should fail a valid pointer whose own bytes carry the key or a Figma URL after the pointer lines', async () => {
    await put(
      'apps/sites/public/trailer.bin',
      `${await storeObject('clean payload')}${FAKE_KEY}\n${FILE_URL}\n`
    )
    commit('pointer with a trailer')
    expect(
      guard({ worktree: false, key: FAKE_KEY })
        .findings.map((finding) => finding.check)
        .sort()
    ).toEqual(['key', 'url'])
  })

  it('should fail a pointer whose object is missing from the local store', async () => {
    await put('apps/sites/public/absent.bin', `${POINTER_PREFIX}${'a'.repeat(64)}\nsize 4\n`)
    commit('missing object')
    expect(guard({ worktree: false, key: FAKE_KEY }).findings).toEqual([
      { check: 'lfs', location: `${short('HEAD')}:apps/sites/public/absent.bin` }
    ])
  })

  it('should fail a stored object whose bytes do not match its pointer', async () => {
    const pointer = await storeObject('original')
    const oid = /sha256:([0-9a-f]+)/.exec(pointer)![1]!
    await put(`.git/lfs/objects/${oid.slice(0, 2)}/${oid.slice(2, 4)}/${oid}`, 'tampered')
    await put('apps/sites/public/tampered.bin', pointer)
    commit('tampered object')
    expect(guard({ worktree: false }).findings).toEqual([
      { check: 'lfs', location: `${short('HEAD')}:apps/sites/public/tampered.bin` }
    ])
  })
})

describe('when listing the LFS objects a scan will read', () => {
  it('should list each pointer of the range, deleted ones included, and of the trees, once', async () => {
    const outside = await storeObject('outside')
    const ui = await storeObject('ui')
    await put('apps/sites/public/a.bin', outside)
    await put('apps/sites/public/b.bin', outside)
    await put('library/ui/c.bin', ui)
    await put('apps/sites/plain.txt', 'not a pointer')
    commit('pointers')
    await rm(path.join(root, 'apps/sites/public/a.bin'))
    await rm(path.join(root, 'apps/sites/public/b.bin'))
    commit('delete the outside ones')
    const oid = (pointer: string) => /sha256:([0-9a-f]+)/.exec(pointer)![1]!
    expect(
      listLfsObjects({ root, range: 'base..HEAD', trees: [{ rev: 'HEAD', prefix: 'library/ui' }] })
        .map((item) => item.oid)
        .sort()
    ).toEqual([oid(outside), oid(ui)].sort())
  })

  it('should list nothing for a range without pointers', async () => {
    await put('apps/sites/plain.txt', 'not a pointer')
    commit('plain')
    expect(listLfsObjects({ root, range: 'base..HEAD' })).toEqual([])
  })

  it('should print the objects as JSON lines, with no path, from --lfs-objects', async () => {
    const pointer = await storeObject('listed')
    await put(`apps/${FAKE_KEY}/named.bin`, pointer)
    commit('pointer under a key-named folder')
    const result = spawnSync(
      process.execPath,
      [
        path.join(import.meta.dirname, 'key-guard.mjs'),
        '--root',
        root,
        '--range',
        'base..HEAD',
        '--no-worktree',
        '--lfs-objects'
      ],
      { encoding: 'utf8', env: { ...process.env, FIGMA_FILE_KEY: FAKE_KEY } }
    )
    expect(result.status).toBe(0)
    expect(result.stdout).not.toContain(FAKE_KEY)
    expect(
      result.stdout
        .trim()
        .split('\n')
        .map((line) => JSON.parse(line))
    ).toEqual([{ oid: /sha256:([0-9a-f]+)/.exec(pointer)![1], size: Buffer.byteLength('listed') }])
  })
})

// The key scan job gets the prefetched objects as downloaded data, outside the checkout's own store.
describe('when the LFS objects come from a separate store', () => {
  // Inside .git only so that commit's git add leaves it out; it is not the checkout's own store.
  const store = () => path.join(root, '.git/downloaded-lfs')
  async function putObject(directory: string, content: string, stored = content) {
    const oid = createHash('sha256').update(content).digest('hex')
    const file = path.join(directory, oid.slice(0, 2), oid.slice(2, 4), oid)
    await mkdir(path.dirname(file), { recursive: true })
    await writeFile(file, stored)
    return `${POINTER_PREFIX}${oid}\nsize ${Buffer.byteLength(content)}\n`
  }
  const run = (...args: string[]) =>
    spawnSync(
      process.execPath,
      [
        path.join(import.meta.dirname, 'key-guard.mjs'),
        '--root',
        root,
        '--range',
        'base..HEAD',
        '--no-worktree',
        ...args
      ],
      { encoding: 'utf8', env: { ...process.env, FIGMA_FILE_KEY: FAKE_KEY } }
    )

  it('should read a pointer payload from --lfs-store and find the key in it', async () => {
    await put('apps/sites/public/leak.bin', await putObject(store(), `x ${FAKE_KEY} x`))
    commit('pointer stored elsewhere')
    const result = run('--lfs-store', store())
    expect(result.status).toBe(1)
    expect(result.stdout).toContain(`FAIL key ${short('HEAD')}:apps/sites/public/leak.bin`)
    expect(result.stdout).not.toContain(FAKE_KEY)
  })

  it('should ignore the checkout store when --lfs-store names another one', async () => {
    const pointer = await storeObject(FAKE_KEY)
    await put('apps/sites/public/local.bin', pointer)
    commit('pointer stored only in the checkout')
    const result = run('--lfs-store', store())
    expect(result.stdout).toContain(`FAIL lfs ${short('HEAD')}:apps/sites/public/local.bin`)
    expect(result.stdout).not.toContain('FAIL key')
  })

  it('should reject an object in --lfs-store whose bytes do not match its pointer', async () => {
    await put('apps/sites/public/swapped.bin', await putObject(store(), FAKE_KEY, 'clean bytes'))
    commit('swapped object')
    const result = run('--lfs-store', store())
    expect(result.stdout).toContain(`FAIL lfs ${short('HEAD')}:apps/sites/public/swapped.bin`)
    expect(result.status).toBe(1)
  })

  it('should fail with no directory after --lfs-store', () => {
    const result = run('--lfs-store')
    expect(result.status).toBe(2)
    expect(result.stderr).toContain('--lfs-store takes a directory')
  })

  it('should not take the next option as the --lfs-store directory', () => {
    const result = run('--lfs-store', '--range', 'HEAD')
    expect(result.status).toBe(2)
    expect(result.stderr).toContain('--lfs-store takes a directory')
  })
})
