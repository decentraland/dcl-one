import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  symlinkSync,
  writeFileSync
} from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { inspectPublicOutput } from '../public-output.mjs'

describe('when inspecting independently pinned public outputs and inputs', () => {
  const scope = 'library/ui/components'
  const file = 'Nav/Nav.tsx'
  const input = 'library/ui/figma/review/runtime.ts'
  const hash = (bytes: string | Buffer) => createHash('sha256').update(bytes).digest('hex')
  let root: string
  let table: Record<string, { sha256: string; origin: Record<string, string> }>
  let inputFiles: Map<string, { sha256: string }>
  let result: ReturnType<typeof inspectPublicOutput>
  let git: (...args: string[]) => string
  let put: (relative: string, bytes: string) => void
  let stageBlob: (relative: string, bytes: string, mode?: string) => void
  let inspect: () => ReturnType<typeof inspectPublicOutput>

  beforeEach(() => {
    root = mkdtempSync(path.join(tmpdir(), 'public-output-'))
    git = (...args) =>
      execFileSync('git', ['-C', root, ...args], {
        encoding: 'utf8',
        env: { ...process.env, GIT_INDEX_FILE: path.join(root, '.git/index') }
      }).trim()
    git('init', '--quiet')
    put = (relative, bytes) => {
      mkdirSync(path.dirname(path.join(root, relative)), { recursive: true })
      writeFileSync(path.join(root, relative), bytes)
    }
    stageBlob = (relative, bytes, mode = '100644') => {
      git(
        'update-index',
        '--add',
        '--cacheinfo',
        `${mode},${execFileSync('git', ['-C', root, 'hash-object', '-w', '--stdin'], { input: bytes }).toString().trim()},${relative}`
      )
    }
    put(scope + '/' + file, 'export const Nav = 1')
    put(input, 'runtime source')
    git('add', '.')
    table = {
      [file]: {
        sha256: hash('export const Nav = 1'),
        origin: { kind: 'runtime', path: '../../never-read' }
      }
    }
    inputFiles = new Map([[input, { sha256: hash('runtime source') }]])
    inspect = () => {
      // Every inspector Git read must use this isolated fixture index too.
      const previous = process.env.GIT_INDEX_FILE
      process.env.GIT_INDEX_FILE = path.join(root, '.git/index')
      try {
        return inspectPublicOutput(root, { scope, table, inputFiles })
      } finally {
        if (previous === undefined) delete process.env.GIT_INDEX_FILE
        else process.env.GIT_INDEX_FILE = previous
      }
    }
  })

  afterEach(() => {
    rmSync(root, { recursive: true, force: true })
  })

  it('should return verified output and input identities without interpreting origin paths', () => {
    result = inspect()
    expect(result.problems).toEqual([])
    expect(result.actual).toEqual([file])
    expect(result.outputs.get(file)).toEqual({
      sha256: table[file]!.sha256,
      indexBlobSha: git('rev-parse', ':' + scope + '/' + file),
      origin: table[file]!.origin
    })
    expect(result.inputs.get(input)).toEqual({
      sha256: hash('runtime source'),
      indexBlobSha: git('rev-parse', ':' + input)
    })
    expect(result.outputs.get(file)!.sha256).not.toBe(result.outputs.get(file)!.indexBlobSha)
  })

  describe.each([
    '../outside',
    '/absolute',
    'C:/drive',
    'Nav\\bad',
    'Nav//bad',
    'Nav/../bad',
    'Nav/NUL.ts',
    'Nav/name.',
    'Nav/name ',
    'Nav/a\nb'
  ])('when a table contains the unsafe path %j', (unsafe) => {
    beforeEach(() => {
      table[unsafe] = table[file]!
    })
    it('should reject it without inspecting that path', () => {
      expect(inspect().problems).toContain('unsafe public file path')
    })
  })

  it('should reject an absent file table', () => {
    expect(inspectPublicOutput(root, { scope, table: {} }).problems).toContain(
      'public output has no pinned file table'
    )
  })
  it('should reject unsupported scopes', () => {
    expect(inspectPublicOutput(root, { scope: '../outside', table }).problems).toContain(
      'unsupported public output scope'
    )
  })
  it('should reject unstaged materialized changes', () => {
    put(scope + '/' + file, 'changed')
    result = inspect()
    expect(result.problems).toContain('changed public file: ' + file)
    expect(result.problems).toContain('public index bytes differ: ' + file)
    expect(result.outputs.has(file)).toBe(false)
  })
  it('should reject a changed index even when materialized bytes match the pin', () => {
    stageBlob(scope + '/' + file, 'other staged bytes')
    expect(inspect().problems).toContain('public index bytes differ: ' + file)
  })
  it('should reject missing materialized output', () => {
    rmSync(path.join(root, scope, file))
    expect(inspect().problems).toContain('missing public file: ' + file)
  })
  it('should reject untracked pinned output', () => {
    git('rm', '--cached', scope + '/' + file)
    expect(inspect().problems).toContain('untracked public file: ' + file)
  })
  it('should reject extra untracked output', () => {
    put(scope + '/extra.ts', 'extra')
    expect(inspect().problems).toContain('extra public file: extra.ts')
  })
  it('should reject an indexed extra even when absent from disk', () => {
    stageBlob(scope + '/extra.ts', 'extra')
    expect(inspect().problems).toContain('extra indexed public file: extra.ts')
  })
  it('should reject case-insensitive output collisions', () => {
    table['nav/nav.tsx'] = table[file]!
    expect(inspect().problems).toContain('non-portable public file collision: nav/nav.tsx')
  })
  it('should reject symlinked output files', () => {
    rmSync(path.join(root, scope, file))
    symlinkSync(path.join(root, input), path.join(root, scope, file))
    expect(inspect().problems).toContain('public output contains a symlink: ' + file)
  })
  it('should reject symlinked scope ancestors', () => {
    renameSync(path.join(root, 'library'), path.join(root, 'elsewhere'))
    symlinkSync('elsewhere', path.join(root, 'library'))
    expect(inspect().problems).toContain('public output directory is missing or unsafe')
  })
  it('should reject a symlinked repository root', () => {
    symlinkSync(root, path.join(root, 'repo-link'))
    expect(inspectPublicOutput(path.join(root, 'repo-link'), { scope, table }).problems).toContain(
      'public output directory is missing or unsafe'
    )
  })
  it('should reject non-regular output entries', () => {
    execFileSync('mkfifo', [path.join(root, scope, 'pipe')])
    expect(inspect().problems).toContain('public output contains a special file: pipe')
  })
  it('should reject symlink index mode even when disk contains regular bytes', () => {
    stageBlob(scope + '/' + file, 'somewhere', '120000')
    expect(inspect().problems).toContain('unsafe index mode for public file: ' + file)
  })
  it('should reject unmerged index entries', () => {
    git('rm', '--cached', scope + '/' + file)
    execFileSync('git', ['-C', root, 'update-index', '--index-info'], {
      input: `100644 ${git('rev-parse', ':' + input)} 1\t${scope}/${file}\n100644 ${git('rev-parse', ':' + input)} 2\t${scope}/${file}\n`,
      env: { ...process.env, GIT_INDEX_FILE: path.join(root, '.git/index') }
    })
    expect(inspect().problems).toContain('unmerged public file: ' + file)
  })
  it('should reject materialized pointer payloads', () => {
    put(
      scope + '/' + file,
      `version https://git-lfs.github.com/spec/v1\noid sha256:${table[file]!.sha256}\nsize 20\n`
    )
    expect(inspect().problems).toContain('unmaterialized public file: ' + file)
  })
  it('should reject CRLF pointer payloads even when both bytes and pin match', () => {
    put(
      scope + '/' + file,
      'version https://git-lfs.github.com/spec/v1\r\noid sha256:' +
        'a'.repeat(64) +
        '\r\nsize 1\r\n'
    )
    git('add', scope + '/' + file)
    table[file]!.sha256 = hash(readFileSync(path.join(root, scope, file)))
    expect(inspect().problems).toContain('unmaterialized public file: ' + file)
  })
  it('should reject indexed pointers for components', () => {
    stageBlob(
      scope + '/' + file,
      `version https://git-lfs.github.com/spec/v1\noid sha256:${table[file]!.sha256}\nsize 20\n`
    )
    expect(inspect().problems).toContain('public index bytes differ: ' + file)
  })
  it('should reject components LFS exception requests', () => {
    expect(
      inspectPublicOutput(root, { scope, table, lfsFiles: new Set([file]) }).problems
    ).toContain('components do not permit LFS exceptions')
  })
  describe.each(['../outside', 'library/ui/figma/NUL.ts', 'library/ui/figma/x\ny'])(
    'when an input path is unsafe: %j',
    (unsafe) => {
      beforeEach(() => {
        inputFiles.set(unsafe, { sha256: hash('outside') })
      })
      it('should reject the input without following it', () => {
        expect(inspect().problems).toContain('unsafe or unpinned public input: ' + unsafe)
      })
    }
  )
  it('should reject malformed input digests', () => {
    inputFiles.set(input, { sha256: 'not a digest' })
    expect(inspect().problems).toContain('unsafe or unpinned public input: ' + input)
  })
  it('should reject changed materialized inputs', () => {
    put(input, 'changed source')
    result = inspect()
    expect(result.problems).toContain('changed public input: ' + input)
    expect(result.inputs.has(input)).toBe(false)
  })
  it('should reject changed input index bytes', () => {
    stageBlob(input, 'changed source')
    expect(inspect().problems).toContain('input index bytes differ: ' + input)
  })
  it('should reject missing inputs', () => {
    rmSync(path.join(root, input))
    expect(inspect().problems).toContain('missing public input: ' + input)
  })
  it('should reject symlinked input ancestors', () => {
    renameSync(path.join(root, 'library/ui/figma'), path.join(root, 'runtime-elsewhere'))
    symlinkSync(path.join(root, 'runtime-elsewhere'), path.join(root, 'library/ui/figma'))
    expect(inspect().problems).toContain('symlink public input: ' + input)
  })
  it('should reject an input whose pin matches bytes but index contains a pointer', () => {
    stageBlob(
      input,
      `version https://git-lfs.github.com/spec/v1\noid sha256:${hash('runtime source')}\nsize 14\n`
    )
    expect(inspect().problems).toContain('input index bytes differ: ' + input)
  })
  it('should not alter index bytes or materialized output on refusal', () => {
    put(input, 'changed source')
    inspect()
    expect(readFileSync(path.join(root, scope, file), 'utf8')).toBe('export const Nav = 1')
    expect(git('show', ':' + input)).toBe('runtime source')
  })
})
