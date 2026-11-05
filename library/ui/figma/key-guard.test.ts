import { execFileSync } from 'node:child_process'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { runGuard } from './key-guard.mjs'

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
  key?: string
}) {
  return runGuard({
    root,
    range: options.range === undefined ? 'base..HEAD' : options.range,
    worktree: options.worktree ?? true,
    paths: options.paths ?? [],
    key: options.key
  })
}

beforeEach(async () => {
  root = await mkdtemp(path.join(tmpdir(), 'key-guard-'))
  git('init', '-q', '-b', 'main')
  await put('README.md', 'clean\n')
  commit('base')
  git('tag', 'base')
})

afterEach(async () => {
  await rm(root, { recursive: true, force: true })
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
