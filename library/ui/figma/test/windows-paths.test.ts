import { readMaterialized } from '../src/lfs.js'
import { execFileSync } from 'node:child_process'
import { readdirSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { encodePageFileName, isWindowsSafeName } from '../src/capture/page-file-name.js'

// The repository is also checked out on Windows, so no library/ui path may hold a name Windows rejects:
// a reserved or control character, a trailing dot or space, or a reserved device stem in any case.
const repoRoot = path.resolve(import.meta.dirname, '../../../..')
const figma = path.resolve(import.meta.dirname, '..')

// Tracked files plus new, not-ignored ones, so the gate also holds before a change is committed.
const libraryPaths = () =>
  execFileSync(
    'git',
    ['ls-files', '-z', '--cached', '--others', '--exclude-standard', 'library/ui'],
    { cwd: repoRoot, encoding: 'utf8' }
  )
    .split('\0')
    .filter(Boolean)

const invalid = (paths: string[]) =>
  paths.filter((file) => !file.split('/').every((segment) => isWindowsSafeName(segment)))

describe('when checking library/ui paths for a Windows checkout', () => {
  it('should hold no path that Windows rejects', () => {
    const paths = libraryPaths()
    // A real listing, not an empty or failed one: the synthetic snapshot alone is dozens of paths.
    expect(paths.length).toBeGreaterThan(300)
    expect(paths).toContain(
      'library/ui/figma/test/fixtures/synthetic-snapshot/components/Demo%3A Legacy Bars.json'
    )
    expect(invalid(paths)).toEqual([])
  })

  // One planted path per rule class (control).
  it.each([
    ['a reserved character', 'library/ui/figma/figma-snapshot/components/Bars: Nav bar.json'],
    ['a control character', 'library/ui/figma/tab\there.json'],
    ['the delete character', 'library/ui/figma/del\u007fete.json'],
    ['a trailing dot in a folder', 'library/ui/figma/trailing./x.json'],
    ['a trailing space in a folder', 'library/ui/figma/trailing /x.json'],
    ['a device stem', 'library/ui/figma/AUX.json'],
    ['a device stem in another case with an extension', 'library/ui/figma/lpt1.notes.json'],
    ['a superscript device stem', 'library/ui/figma/COM².json'],
    [
      'a superscript device stem in another case with an extension',
      'library/ui/figma/lpt³.notes.json'
    ]
  ])('should reject %s', (_, planted) => {
    expect(invalid([...libraryPaths(), planted])).toEqual([planted])
  })
})

type ConfiguredPage = { id: string | null; name: string }
const configuredPages = (file: string): ConfiguredPage[] =>
  Object.values(
    (JSON.parse(readFileSync(file, 'utf8')) as { pages: Record<string, ConfiguredPage> }).pages
  )
const capturesIn = async (directory: string) =>
  Promise.all(
    readdirSync(directory)
      .filter((file) => file.endsWith('.json'))
      .map(async (file) => {
        const { page } = JSON.parse(
          (await readMaterialized(path.join(directory, file))).toString('utf8')
        ) as {
          page: { id: string; name: string }
        }
        return { file, id: page.id, name: page.name }
      })
  )

// Only tracked captures: the synthetic test snapshot. The capture cache (figma-snapshot) is gitignored and is
// never present in CI.
const captures = await capturesIn(path.join(figma, 'test/fixtures/synthetic-snapshot/components'))

describe('when mapping every existing Figma page name to its capture file', () => {
  const pages = [
    ...configuredPages(path.join(figma, 'figma.config.json')),
    ...configuredPages(path.join(figma, 'test/fixtures/figma.config.json'))
  ]

  const names = [...new Set([...pages, ...captures].map((page) => page.name))]
  // A capture is stored under its configured page name; the Figma page name may add a marker (for
  // example a trailing check mark). A legacy capture, configured by name with no id, keeps that name even
  // when its page id is now claimed by a canonical page.
  const configuredName = new Map(
    pages.filter((page) => page.id !== null).map((page) => [page.id!, page.name])
  )

  const legacyNames = new Set(pages.filter((page) => page.id === null).map((page) => page.name))
  const storedName = (capture: { id: string; name: string }) =>
    legacyNames.has(capture.name) ? capture.name : (configuredName.get(capture.id) ?? capture.name)

  it('should give every page its own file name', () => {
    expect(new Set(names.map(encodePageFileName)).size).toBe(names.length)
  })

  it('should change the file name of the two legacy pages named with a colon only', () => {
    expect(names.filter((name) => encodePageFileName(name) !== `${name}.json`)).toEqual([
      'Bars: Nav bar - Tabs bar - Footer',
      'Demo: Legacy Bars'
    ])
    expect(encodePageFileName('Demo: Legacy Bars')).toBe('Demo%3A Legacy Bars.json')
    expect(captures.map((capture) => capture.file)).toContain('Demo%3A Legacy Bars.json')
  })

  it('should store every capture under the encoded name of its page', () => {
    expect(
      captures.filter((capture) => capture.file !== encodePageFileName(storedName(capture)))
    ).toEqual([])
  })
})

// A capture file name is formed only by encodePageFileName; any other interpolated `.json` name under
// figma/src is a second mapper. One exemption, with its reason.
const EXEMPT = new Map([
  ['src/capture/page-file-name.ts', 'the mapper itself'],
  ['src/guard/fs-safe.test.ts', 'a temporary file for the listing test, not a capture']
])
const INTERPOLATED_JSON = /\$\{[^}]+\}\.json/

const sources = (directory: string): string[] =>
  readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const full = path.join(directory, entry.name)
    if (entry.isDirectory()) return sources(full)
    return /\.(ts|mts|tsx)$/.test(entry.name) ? [full] : []
  })

const secondMappers = (files: { file: string; text: string }[]) =>
  files
    .filter(({ file, text }) => !EXEMPT.has(file) && INTERPOLATED_JSON.test(text))
    .map(({ file }) => file)

describe('when forming capture file names in figma/src', () => {
  const files = sources(path.join(figma, 'src')).map((full) => ({
    file: path.relative(figma, full),
    text: readFileSync(full, 'utf8')
  }))

  it('should go through encodePageFileName everywhere', () => {
    expect(secondMappers(files)).toEqual([])
  })

  it('should fail on a hand-built capture name (control)', () => {
    const planted = { file: 'src/x.ts', text: "path.join(dir, 'components', `${name}.json`)" }
    expect(secondMappers([...files, planted])).toEqual(['src/x.ts'])
  })
})
