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

  describe('and the outputs and inputs match their pins', () => {
    beforeEach(() => {
      result = inspect()
    })

    it('should report no problems', () => {
      expect(result.problems).toEqual([])
    })

    it('should list the pinned output as the actual output', () => {
      expect(result.actual).toEqual([file])
    })

    it('should return the verified output identity without interpreting origin paths', () => {
      expect(result.outputs.get(file)).toEqual({
        sha256: table[file]!.sha256,
        indexBlobSha: git('rev-parse', ':' + scope + '/' + file),
        origin: table[file]!.origin
      })
      expect(result.outputs.get(file)!.sha256).not.toBe(result.outputs.get(file)!.indexBlobSha)
    })

    it('should return the verified input identity', () => {
      expect(result.inputs.get(input)).toEqual({
        sha256: hash('runtime source'),
        indexBlobSha: git('rev-parse', ':' + input)
      })
    })
  })

  describe.each([
    'library/ui/figma/component-generation.mts',
    'library/ui/figma/component-generation.MTS',
    'library/ui/figma/source-identity.cts',
    'library/ui/figma/src/generators/components.tsx'
  ])('and an executable override exists at %s', (shadow) => {
    beforeEach(() => {
      put(shadow, 'export const shadow = true')
    })

    it('should reject an untracked implementation outside the declared read-set', () => {
      expect(inspect().problems).toEqual(['untracked component implementation: ' + shadow])
    })

    describe('and Git ignores it', () => {
      beforeEach(() => {
        put('.gitignore', shadow + '\n')
      })

      it('should still reject the implementation', () => {
        expect(inspect().problems).toEqual(['untracked component implementation: ' + shadow])
      })
    })
  })

  describe('and a root alternative is tracked', () => {
    const shadow = 'library/ui/figma/component-generation.mts'

    beforeEach(() => {
      put(shadow, 'export const shadow = true')
      git('add', '--', shadow)
    })

    it('should inventory it independently from supplied input pins', () => {
      expect(inspect().indexPaths).toContain(shadow)
    })
  })

  describe('and a source directory is a recursive symlink', () => {
    beforeEach(() => {
      const target = path.join(root, 'outside-source')
      mkdirSync(target)
      writeFileSync(path.join(target, 'shadow.mts'), 'export const hidden = true')
      mkdirSync(path.join(root, 'library/ui/figma/src'), { recursive: true })
      symlinkSync(target, path.join(root, 'library/ui/figma/src/external'))
    })

    it('should reject the symlink without following it', () => {
      expect(inspect().problems).toEqual([
        'symlink component implementation: library/ui/figma/src/external'
      ])
    })
  })

  describe('and an ignored gallery build exists', () => {
    beforeEach(() => {
      put('library/ui/figma/review/review-dist/assets/gallery.js', 'compiled gallery')
    })

    it('should not treat it as executable component source', () => {
      expect(inspect().problems).toEqual([])
    })
  })

  describe.each([
    'library/ui/figma/package.json',
    'library/ui/figma/PACKAGE.JSON',
    'library/ui/figma/jsconfig.json',
    'library/ui/figma/src/nested/package.json',
    'library/ui/figma/src/test/fixtures/package.json',
    'library/ui/figma/src/__tests__/tsconfig.paths.json',
    'library/ui/figma/review/package.json'
  ])('and an unapproved resolver control exists at %s', (control) => {
    beforeEach(() => {
      put(
        control,
        JSON.stringify({ name: 'postcss', exports: { '.': './audit-dependency/postcss.mjs' } })
      )
      put(
        'library/ui/figma/audit-dependency/postcss.mjs',
        'export default () => ({ auditDependencyShadow: true })'
      )
    })

    it('should reject the resolver redirect without evaluating its non-source target', () => {
      expect(inspect().problems).toContain('unexpected component resolver control: ' + control)
    })

    describe('and it is staged and supplied a pin', () => {
      beforeEach(() => {
        inputFiles.set(control, { sha256: hash(readFileSync(path.join(root, control))) })
        git('add', '--', control)
      })

      it('should still reject the control', () => {
        expect(inspect().problems).toContain('unexpected component resolver control: ' + control)
      })
    })
  })

  describe.each([
    'library/ui/figma/node_modules',
    'library/ui/figma/src/NODE_MODULES',
    'library/ui/figma/src/generators/node_modules',
    'library/ui/figma/src/test/fixtures/node_modules',
    'library/ui/figma/review/node_modules'
  ])('and an empty nearer dependency directory exists at %s', (directory) => {
    beforeEach(() => {
      mkdirSync(path.join(root, directory), { recursive: true })
    })

    it('should reject the alternate package lookup location', () => {
      expect(inspect().problems).toContain('unexpected component resolver control: ' + directory)
    })
  })

  describe('and a normally excluded source fixture directory holds a special entry', () => {
    beforeEach(() => {
      mkdirSync(path.join(root, 'library/ui/figma/src/test/fixtures'), { recursive: true })
      execFileSync('mkfifo', [path.join(root, 'library/ui/figma/src/test/fixtures/pipe')])
    })

    it('should reject the special entry', () => {
      expect(inspect().problems).toContain(
        'unsafe component resolver entry: library/ui/figma/src/test/fixtures/pipe'
      )
    })
  })

  describe('and the root Figma tsconfig is tracked', () => {
    beforeEach(() => {
      put('library/ui/figma/tsconfig.json', '{"compilerOptions":{}}')
      git('add', '.')
    })

    describe('and it is pinned', () => {
      beforeEach(() => {
        inputFiles.set('library/ui/figma/tsconfig.json', {
          sha256: hash('{"compilerOptions":{}}')
        })
      })

      it('should permit the existing pinned root Figma tsconfig control', () => {
        expect(inspect().problems).toEqual([])
      })
    })

    describe('and it is not pinned', () => {
      it('should reject it as an unexpected resolver control', () => {
        expect(inspect().problems).toContain(
          'unexpected component resolver control: library/ui/figma/tsconfig.json'
        )
      })
    })
  })

  describe('and a locked non-source helper has a tracked executable sibling', () => {
    describe('and the sibling is not in the read-set', () => {
      beforeEach(() => {
        put('library/ui/figma/review/shadow.mjs', 'export const shadow = true')
        git('add', '.')
      })

      it('should reject the unpinned helper implementation', () => {
        expect(inspect().problems).toContain(
          'unpinned component helper implementation: library/ui/figma/review/shadow.mjs'
        )
      })
    })

    describe('and the sibling is included in the read-set', () => {
      beforeEach(() => {
        put('library/ui/figma/review/helper.mjs', 'export const helper = true')
        inputFiles.set('library/ui/figma/review/helper.mjs', {
          sha256: hash('export const helper = true')
        })
        git('add', '.')
      })

      it('should permit the sibling', () => {
        expect(inspect().problems).toEqual([])
      })
    })
  })

  describe('and the fixed review runtime is pinned beside a gallery config', () => {
    beforeEach(() => {
      const runtime = 'library/ui/figma/review/src/components/figma-view-state.ts'
      inputFiles.delete(input)
      put(runtime, 'runtime bytes')
      inputFiles.set(runtime, { sha256: hash('runtime bytes') })
      put('library/ui/figma/review/review.vite.config.ts', 'gallery config')
      git('add', '.')
    })

    it('should treat the runtime as byte input without admitting gallery implementations', () => {
      expect(inspect().problems).toEqual([])
    })
  })

  describe('and a producer is pinned', () => {
    beforeEach(() => {
      put('library/ui/figma/src/install-components.ts', 'producer bytes')
      inputFiles.set('library/ui/figma/src/install-components.ts', {
        sha256: hash('producer bytes')
      })
      git('add', '.')
    })

    it('should validate producer identities without requiring an installed dependency tree', () => {
      expect(inspect().problems).toEqual([])
    })
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
  ])('and a table contains the unsafe path %j', (unsafe) => {
    beforeEach(() => {
      table[unsafe] = table[file]!
    })

    it('should reject it without inspecting that path', () => {
      expect(inspect().problems).toContain('unsafe public file path')
    })
  })

  describe('and the file table is absent', () => {
    it('should reject the output', () => {
      expect(inspectPublicOutput(root, { scope, table: {} }).problems).toContain(
        'public output has no pinned file table'
      )
    })
  })

  describe('and the scope is unsupported', () => {
    it('should reject the scope', () => {
      expect(inspectPublicOutput(root, { scope: '../outside', table }).problems).toContain(
        'unsupported public output scope'
      )
    })
  })

  describe('and the materialized output changed without staging', () => {
    beforeEach(() => {
      put(scope + '/' + file, 'changed')
      result = inspect()
    })

    it('should report the changed public file', () => {
      expect(result.problems).toContain('changed public file: ' + file)
    })

    it('should report that the index bytes differ', () => {
      expect(result.problems).toContain('public index bytes differ: ' + file)
    })

    it('should not return the output identity', () => {
      expect(result.outputs.has(file)).toBe(false)
    })
  })

  describe('and the index changed while materialized bytes match the pin', () => {
    beforeEach(() => {
      stageBlob(scope + '/' + file, 'other staged bytes')
    })

    it('should reject the changed index', () => {
      expect(inspect().problems).toContain('public index bytes differ: ' + file)
    })
  })

  describe('and the materialized output is missing', () => {
    beforeEach(() => {
      rmSync(path.join(root, scope, file))
    })

    it('should reject the missing output', () => {
      expect(inspect().problems).toContain('missing public file: ' + file)
    })
  })

  describe('and the pinned output is untracked', () => {
    beforeEach(() => {
      git('rm', '--cached', scope + '/' + file)
    })

    it('should reject the untracked output', () => {
      expect(inspect().problems).toContain('untracked public file: ' + file)
    })
  })

  describe('and an extra untracked output exists', () => {
    beforeEach(() => {
      put(scope + '/extra.ts', 'extra')
    })

    it('should reject the extra output', () => {
      expect(inspect().problems).toContain('extra public file: extra.ts')
    })
  })

  describe('and an extra output is indexed but absent from disk', () => {
    beforeEach(() => {
      stageBlob(scope + '/extra.ts', 'extra')
    })

    it('should reject the indexed extra', () => {
      expect(inspect().problems).toContain('extra indexed public file: extra.ts')
    })
  })

  describe('and the table has a case-insensitive output collision', () => {
    beforeEach(() => {
      table['nav/nav.tsx'] = table[file]!
    })

    it('should reject the collision', () => {
      expect(inspect().problems).toContain('non-portable public file collision: nav/nav.tsx')
    })
  })

  describe('and an output file is a symlink', () => {
    beforeEach(() => {
      rmSync(path.join(root, scope, file))
      symlinkSync(path.join(root, input), path.join(root, scope, file))
    })

    it('should reject the symlinked output file', () => {
      expect(inspect().problems).toContain('public output contains a symlink: ' + file)
    })
  })

  describe('and a scope ancestor is a symlink', () => {
    beforeEach(() => {
      renameSync(path.join(root, 'library'), path.join(root, 'elsewhere'))
      symlinkSync('elsewhere', path.join(root, 'library'))
    })

    it('should reject the output directory as unsafe', () => {
      expect(inspect().problems).toContain('public output directory is missing or unsafe')
    })
  })

  describe('and the repository root is a symlink', () => {
    let link: string

    beforeEach(() => {
      link = path.join(root, 'repo-link')
      symlinkSync(root, link)
    })

    it('should reject the output directory as unsafe', () => {
      expect(inspectPublicOutput(link, { scope, table }).problems).toContain(
        'public output directory is missing or unsafe'
      )
    })
  })

  describe('and the output holds a non-regular entry', () => {
    beforeEach(() => {
      execFileSync('mkfifo', [path.join(root, scope, 'pipe')])
    })

    it('should reject the special file', () => {
      expect(inspect().problems).toContain('public output contains a special file: pipe')
    })
  })

  describe('and the index records a symlink mode while disk holds regular bytes', () => {
    beforeEach(() => {
      stageBlob(scope + '/' + file, 'somewhere', '120000')
    })

    it('should reject the unsafe index mode', () => {
      expect(inspect().problems).toContain('unsafe index mode for public file: ' + file)
    })
  })

  describe('and the index holds unmerged entries', () => {
    beforeEach(() => {
      git('rm', '--cached', scope + '/' + file)
      execFileSync('git', ['-C', root, 'update-index', '--index-info'], {
        input: `100644 ${git('rev-parse', ':' + input)} 1\t${scope}/${file}\n100644 ${git('rev-parse', ':' + input)} 2\t${scope}/${file}\n`,
        env: { ...process.env, GIT_INDEX_FILE: path.join(root, '.git/index') }
      })
    })

    it('should reject the unmerged public file', () => {
      expect(inspect().problems).toContain('unmerged public file: ' + file)
    })
  })

  describe('and the materialized output is a pointer payload', () => {
    beforeEach(() => {
      put(
        scope + '/' + file,
        `version https://git-lfs.github.com/spec/v1\noid sha256:${table[file]!.sha256}\nsize 20\n`
      )
    })

    it('should reject the unmaterialized file', () => {
      expect(inspect().problems).toContain('unmaterialized public file: ' + file)
    })
  })

  describe('and the output is a CRLF pointer payload whose bytes and pin match', () => {
    beforeEach(() => {
      put(
        scope + '/' + file,
        'version https://git-lfs.github.com/spec/v1\r\noid sha256:' +
          'a'.repeat(64) +
          '\r\nsize 1\r\n'
      )
      git('add', scope + '/' + file)
      table[file]!.sha256 = hash(readFileSync(path.join(root, scope, file)))
    })

    it('should reject the unmaterialized file', () => {
      expect(inspect().problems).toContain('unmaterialized public file: ' + file)
    })
  })

  describe('and the index holds a pointer for a component', () => {
    beforeEach(() => {
      stageBlob(
        scope + '/' + file,
        `version https://git-lfs.github.com/spec/v1\noid sha256:${table[file]!.sha256}\nsize 20\n`
      )
    })

    it('should reject the differing index bytes', () => {
      expect(inspect().problems).toContain('public index bytes differ: ' + file)
    })
  })

  describe('and an LFS exception is requested for components', () => {
    it('should reject the exception request', () => {
      expect(
        inspectPublicOutput(root, { scope, table, lfsFiles: new Set([file]) }).problems
      ).toContain('components do not permit LFS exceptions')
    })
  })

  describe.each(['../outside', 'library/ui/figma/NUL.ts', 'library/ui/figma/x\ny'])(
    'and an input path is unsafe: %j',
    (unsafe) => {
      beforeEach(() => {
        inputFiles.set(unsafe, { sha256: hash('outside') })
      })

      it('should reject the input without following it', () => {
        expect(inspect().problems).toContain('unsafe or unpinned public input: ' + unsafe)
      })
    }
  )

  describe('and an input digest is malformed', () => {
    beforeEach(() => {
      inputFiles.set(input, { sha256: 'not a digest' })
    })

    it('should reject the input', () => {
      expect(inspect().problems).toContain('unsafe or unpinned public input: ' + input)
    })
  })

  describe('and a materialized input changed', () => {
    beforeEach(() => {
      put(input, 'changed source')
      result = inspect()
    })

    it('should report the changed input', () => {
      expect(result.problems).toContain('changed public input: ' + input)
    })

    it('should not return the input identity', () => {
      expect(result.inputs.has(input)).toBe(false)
    })
  })

  describe('and the input index bytes changed', () => {
    beforeEach(() => {
      stageBlob(input, 'changed source')
    })

    it('should reject the differing input index bytes', () => {
      expect(inspect().problems).toContain('input index bytes differ: ' + input)
    })
  })

  describe('and an input is missing', () => {
    beforeEach(() => {
      rmSync(path.join(root, input))
    })

    it('should reject the missing input', () => {
      expect(inspect().problems).toContain('missing public input: ' + input)
    })
  })

  describe('and an input ancestor is a symlink', () => {
    beforeEach(() => {
      renameSync(path.join(root, 'library/ui/figma'), path.join(root, 'runtime-elsewhere'))
      symlinkSync(path.join(root, 'runtime-elsewhere'), path.join(root, 'library/ui/figma'))
    })

    it('should reject the symlinked input', () => {
      expect(inspect().problems).toContain('symlink public input: ' + input)
    })
  })

  describe('and the input pin matches its bytes but the index holds a pointer', () => {
    beforeEach(() => {
      stageBlob(
        input,
        `version https://git-lfs.github.com/spec/v1\noid sha256:${hash('runtime source')}\nsize 14\n`
      )
    })

    it('should reject the differing input index bytes', () => {
      expect(inspect().problems).toContain('input index bytes differ: ' + input)
    })
  })

  describe('and the inspection refuses a changed input', () => {
    beforeEach(() => {
      put(input, 'changed source')
      inspect()
    })

    it('should not alter the materialized output', () => {
      expect(readFileSync(path.join(root, scope, file), 'utf8')).toBe('export const Nav = 1')
    })

    it('should not alter the index bytes', () => {
      expect(git('show', ':' + input)).toBe('runtime source')
    })
  })
})
