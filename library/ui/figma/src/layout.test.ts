import { readdirSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { UPSTREAM_LAYOUT } from './layout.js'

// The ported sources address library/ui paths only (layout.ts). A repo-root path of figma-design-system's
// layout would point outside this tree. Upstream package names may still appear in prose and as the
// Review's served folder (/dcl-ui-review/..., public/dcl-ui-review), never at the start of a path.
// Every code and config file of the library is scanned, not only the figma sources.
const library = path.resolve(import.meta.dirname, '../..')
const SOURCE = /\.(ts|tsx|mts|mjs|cjs|js|json|html|css|md|yaml|yml)$/
// Folders that are not code or config, one reason each.
const SKIPPED_DIRS = new Map([
  ['node_modules', 'installed dependencies'],
  ['review-dist', 'the Review build output, gitignored'],
  ['figma/figma-snapshot', 'capture data, not code: Figma metadata, upstream-origin names allowed'],
  [
    'figma/test/fixtures/figma-snapshot',
    'frozen capture data for the tests, same as figma-snapshot'
  ],
  ['figma/review/public', 'the served Review evidence (images and their records), not code']
])
// Files exempt as a whole, one reason each.
const SKIPPED_FILES = new Map([
  ['figma/src/layout.ts', 'defines the upstream-to-library map, so it names the upstream roots'],
  ['figma/src/layout.test.ts', 'this gate, which names the upstream roots to find them'],
  ['upstream.lock.json', 'the provenance record, which names upstream paths by design']
])
const REPO_ROOT_PATH = /(?<![/\w-])(dcl-mcp-figma|dcl-ui-review|component-library)\//
// The Review serves itself from a folder named dcl-ui-review (figma/review/dcl-ui-review, and
// review-dist/dcl-ui-review once built): a folder inside the Review, not a repository root.
const SERVED = [
  "'dcl-ui-review/source-signature.json'",
  "'dcl-ui-review/assets'",
  "'dcl-ui-review/index.html'"
]

const scanned = (relative: string) =>
  SOURCE.test(relative) &&
  !SKIPPED_FILES.has(relative) &&
  ![...SKIPPED_DIRS.keys()].some(
    (dir) => relative.startsWith(`${dir}/`) || relative.split('/').includes(dir)
  )

function sources(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const full = path.join(dir, entry.name)
    if (entry.isDirectory()) return entry.name === '.git' ? [] : sources(full)
    return scanned(path.relative(library, full)) ? [full] : []
  })
}

const offenders = (files: { file: string; text: string }[]) =>
  files
    .filter(
      ({ file, text }) =>
        scanned(path.relative(library, file)) &&
        REPO_ROOT_PATH.test(SERVED.reduce((rest, literal) => rest.split(literal).join(''), text))
    )
    .map(({ file }) => path.relative(library, file))

describe('the ported layout', () => {
  const files = sources(library).map((file) => ({ file, text: readFileSync(file, 'utf8') }))

  it('should leave no repo-root path of the upstream layout in any code or config file', () => {
    expect(offenders(files)).toEqual([])
  })

  it('should map every upstream root to a library/ui path', () => {
    expect(UPSTREAM_LAYOUT.every(([, ours]) => !REPO_ROOT_PATH.test(`${ours}/`))).toBe(true)
  })

  it('should fail on an upstream repo-root path (control)', () => {
    const planted = {
      file: path.join(library, 'figma/src/x.ts'),
      text: "const p = 'dcl-mcp-figma/figma-snapshot'"
    }
    expect(offenders([...files, planted])).toEqual(['figma/src/x.ts'])
  })

  // One planted literal per code or config root, so each is shown to be scanned.
  const ROOTS = [
    'figma/test/x.ts',
    'figma/capture/x.figma.js',
    'figma/scripts/x.mjs',
    'figma/agent-kit/x.mjs',
    'figma/agent-kit/skills/x/SKILL.md',
    'figma/figma.config.json',
    'figma/tsconfig.json',
    'figma/key-guard.mjs',
    'figma/key-guard-allow.json',
    'figma/image-metadata-allow.json',
    'figma/review/src/x.tsx',
    'figma/review/src/generated/x.tsx',
    'figma/review/scripts/x.mjs',
    'figma/review/test/x.ts',
    'figma/review/dcl-ui-review/index.html',
    'figma/review/review.vite.config.ts',
    'figma/review/tsconfig.json',
    'components/x.tsx',
    'index.ts',
    'package.json',
    'vitest.config.ts',
    'tsconfig.json',
    'eslint.config.js',
    'pnpm-workspace.yaml'
  ]
  it.each(ROOTS)('should scan %s (control)', (relative) => {
    const planted = {
      file: path.join(library, relative),
      text: "import x from 'component-library/src/index.ts'"
    }
    expect(offenders([planted])).toEqual([relative])
  })

  it('should keep the served Review folder and the skipped data out of the gate', () => {
    const literal = "'dcl-mcp-figma/x'"
    expect(
      offenders([
        {
          file: path.join(library, 'figma/review/review.vite.config.ts'),
          text: "'dcl-ui-review/assets'"
        },
        { file: path.join(library, 'figma/figma-snapshot/x.json'), text: literal },
        { file: path.join(library, 'figma/review/public/dcl-ui-review/x.json'), text: literal }
      ])
    ).toEqual([])
  })
})
