// @vitest-environment node
import '@/data/lib/creators/test/setup'
import { readdirSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

// The only Emotion files left: they style decentraland-ui2's Tooltip and Navbar until the monorepo replaces both.
// Nothing new joins.
const REMAINING = ['src/components/Tooltip/Tooltip.styles.ts', 'src/components/TopNav/TopNav.styles.ts']

const files = (readdirSync('src', { recursive: true }) as string[]).map(path => `src/${path.replace(/\\/g, '/')}`)

describe('CSS Modules migration', () => {
  it('adds no new Emotion styles file', () => {
    expect(files.length).toBeGreaterThan(0)
    const added = files.filter(path => path.endsWith('.styles.ts') && !REMAINING.includes(path))
    expect(added, 'New styles go in a Component.module.css').toEqual([])
  })

  it('lists only styles files that still exist', () => {
    const gone = REMAINING.filter(path => !files.includes(path))
    expect(gone, 'Delete the entries of converted files').toEqual([])
  })

  it('never leaves a converted component with its old styles file', () => {
    const both = files.filter(
      path => path.endsWith('.module.css') && files.includes(path.replace(/\.module\.css$/, '.styles.ts'))
    )
    expect(both, 'Delete the .styles.ts the module replaces').toEqual([])
  })
})
