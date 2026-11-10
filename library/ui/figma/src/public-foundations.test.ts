import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { publicFoundationProblems } from '../public-foundations.mjs'

describe('when public foundation provenance is checked', () => {
  let root: string
  let lock: {
    sources: { repository: string; snapshot: { revision: string } }[]
    publicFoundations?: {
      files: Record<string, { sha256: string; origin: Record<string, string | undefined> }>
    }
  }
  const digest = (value: string | Buffer) => createHash('sha256').update(value).digest('hex')
  const font = { package: '@fontsource-variable/inter', version: '5.3.0', license: 'OFL-1.1' }
  const relative = 'library/ui/foundations/'
  let put: (file: string, content: string) => Promise<void>
  let git: (...args: string[]) => Buffer

  beforeEach(async () => {
    root = await mkdtemp(path.join(tmpdir(), 'public-foundations-'))
    git = (...args) => execFileSync('git', ['-C', root, ...args], { stdio: 'pipe' })
    git('init', '--quiet')
    put = async (file, content) => {
      await mkdir(path.dirname(path.join(root, relative, file)), { recursive: true })
      await writeFile(path.join(root, relative, file), content)
    }
    const fonts = Object.fromEntries(
      Array.from({ length: 7 }, (_, i) => [`font${i}.woff2`, digest('font' + i)])
    )
    await put(
      'typography/font-source.json',
      JSON.stringify({
        ...font,
        files: fonts,
        licenseFile: 'fonts/OFL.txt',
        licenseSha256: digest('license')
      })
    )
    await put('typography/fonts/OFL.txt', 'license')
    await put('colors/colors.ts', 'export const palette = {}')
    await put(
      'source.json',
      JSON.stringify({
        output: [{ path: 'colors/colors.ts', sha256: digest('export const palette = {}') }]
      })
    )
    await put('foundations.css', '@layer dcl.reset, dcl.foundations, dcl.components;')
    for (let i = 0; i < 7; i++) await put(`typography/fonts/font${i}.woff2`, 'font' + i)
    git('add', '.')
    const files: Record<string, { sha256: string; origin: Record<string, string | undefined> }> = {}
    for (const file of git('ls-files').toString().trim().split('\n')) {
      const name = file.slice(relative.length)
      const bytes = await readFile(path.join(root, file))
      const origin =
        name.endsWith('.woff2') || name.endsWith('OFL.txt')
          ? {
              kind: 'npm',
              ...font,
              path: name.endsWith('OFL.txt') ? 'LICENSE' : 'files/' + path.basename(name)
            }
          : name === 'colors/colors.ts'
            ? { kind: 'generated', revision: 'a'.repeat(40) }
            : { kind: 'source', repository: 'https://github.com/decentraland/dcl-one' }
      files[name] = { sha256: digest(bytes), origin }
      if (name.endsWith('.woff2')) {
        const pointer = `version https://git-lfs.github.com/spec/v1\noid sha256:${digest(bytes)}\nsize ${bytes.length}\n`
        const oid = execFileSync('git', ['-C', root, 'hash-object', '-w', '--stdin'], {
          input: pointer
        })
          .toString()
          .trim()
        git('update-index', '--cacheinfo', '100644,' + oid + ',' + file)
      }
    }
    lock = {
      sources: [
        {
          repository: 'https://github.com/decentraland/figma-design-system',
          snapshot: { revision: 'a'.repeat(40) }
        }
      ],
      publicFoundations: { files }
    }
  })

  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })

  it('should accept materialized bytes whose LFS identities and origins match', () => {
    expect(publicFoundationProblems(root, lock)).toEqual([])
  })
  it('should reject an absent public table', () => {
    delete lock.publicFoundations
    expect(publicFoundationProblems(root, lock)).toContain(
      'public foundations have no pinned file table'
    )
  })
  it('should reject changed bytes before they are staged', async () => {
    await put('colors/colors.ts', 'changed')
    expect(publicFoundationProblems(root, lock)).toContain('changed public file: colors/colors.ts')
  })
  it('should reject a missing file', async () => {
    await rm(path.join(root, relative, 'colors/colors.ts'))
    expect(publicFoundationProblems(root, lock)).toContain('missing public file: colors/colors.ts')
  })
  it('should reject an extra untracked file', async () => {
    await put('extra.ts', '')
    expect(publicFoundationProblems(root, lock)).toContain('extra public file: extra.ts')
  })
  it('should reject an unmaterialized LFS pointer', async () => {
    await put(
      'typography/fonts/font0.woff2',
      git('show', ':' + relative + 'typography/fonts/font0.woff2').toString()
    )
    expect(publicFoundationProblems(root, lock)).toContain(
      'unmaterialized public file: typography/fonts/font0.woff2'
    )
  })
  it('should reject a different LFS object identity', () => {
    const file = relative + 'typography/fonts/font0.woff2'
    git(
      'update-index',
      '--cacheinfo',
      '100644,' +
        git('rev-parse', ':' + relative + 'colors/colors.ts')
          .toString()
          .trim() +
        ',' +
        file
    )
    expect(publicFoundationProblems(root, lock)).toContain(
      'font LFS identity mismatch: typography/fonts/font0.woff2'
    )
  })
  it('should reject omitting a declared font from both the table and directory', async () => {
    delete lock.publicFoundations!.files['typography/fonts/font0.woff2']
    await rm(path.join(root, relative, 'typography/fonts/font0.woff2'))
    expect(publicFoundationProblems(root, lock)).toContain('font manifest differs: font0.woff2')
  })

  it('should reject a wrong npm origin', () => {
    lock.publicFoundations!.files['typography/fonts/font0.woff2']!.origin.version = '0.0.0'
    expect(publicFoundationProblems(root, lock)).toContain(
      'wrong npm origin: typography/fonts/font0.woff2'
    )
  })
  it('should reject a different generated origin', () => {
    lock.publicFoundations!.files['colors/colors.ts']!.origin.revision = 'b'.repeat(40)
    expect(publicFoundationProblems(root, lock)).toContain(
      'wrong generated origin: colors/colors.ts'
    )
  })
  it('should reject labeling generated code as handwritten source', () => {
    lock.publicFoundations!.files['colors/colors.ts']!.origin = {
      kind: 'source',
      repository: 'https://github.com/decentraland/dcl-one'
    }
    expect(publicFoundationProblems(root, lock)).toContain(
      'generated origin classification differs: colors/colors.ts'
    )
  })
  it('should reject a malformed staged manifest without throwing', async () => {
    await put('source.json', 'not-json')
    git('add', relative + 'source.json')
    lock.publicFoundations!.files['source.json']!.sha256 = digest('not-json')
    expect(publicFoundationProblems(root, lock)).toContain('foundation manifests are malformed')
  })

  it('should reject symlinked foundation manifests without reading their target', async () => {
    await rm(path.join(root, relative, 'source.json'))
    await symlink(
      path.join(root, relative, 'typography/font-source.json'),
      path.join(root, relative, 'source.json')
    )
    expect(publicFoundationProblems(root, lock)).toContain(
      'public output contains a symlink: source.json'
    )
  })

  it('should reject a pointer with a correct digest but wrong byte size', () => {
    git(
      'update-index',
      '--cacheinfo',
      '100644,' +
        execFileSync('git', ['-C', root, 'hash-object', '-w', '--stdin'], {
          input: `version https://git-lfs.github.com/spec/v1\noid sha256:${digest('font0')}\nsize 999\n`
        })
          .toString()
          .trim() +
        ',' +
        relative +
        'typography/fonts/font0.woff2'
    )
    expect(publicFoundationProblems(root, lock)).toContain(
      'font LFS identity mismatch: typography/fonts/font0.woff2'
    )
  })

  it('should reject a non-font index pointer even when its LFS digest matches', () => {
    git(
      'update-index',
      '--cacheinfo',
      '100644,' +
        execFileSync('git', ['-C', root, 'hash-object', '-w', '--stdin'], {
          input: `version https://git-lfs.github.com/spec/v1\noid sha256:${digest('export const palette = {}')}\nsize 25\n`
        })
          .toString()
          .trim() +
        ',' +
        relative +
        'colors/colors.ts'
    )
    expect(publicFoundationProblems(root, lock)).toContain(
      'public index bytes differ: colors/colors.ts'
    )
  })
})
