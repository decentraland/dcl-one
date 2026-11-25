import { useState, type ReactNode } from 'react'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { Chip as ChipModule } from '../../../apps/hud/design/Chip'
import * as design from '../../../apps/hud/design'
import * as glyphs from '../../../apps/hud/design/Glyphs'
import { SHOWCASE_SECTIONS } from '../../../apps/hud/design/Showcase'
import {
  SHOWCASE_EXCLUSIONS,
  SHOWCASE_NOT_COMPONENTS
} from '../../../apps/hud/design/ShowcaseIndex'
import {
  claimProblems,
  componentExports,
  coverageProblems,
  isComponentExport
} from '../../src/coverage'
import { GlyphGrid } from '../../src/hud/GlyphsPage'
import { stubElementScroll } from './jsdom'

stubElementScroll()

// Every component the HUD's design entry (apps/hud/design/index.ts) and its glyphs export records its name when it
// mounts, so a section's claim is checked against what it really renders through the entry, as the Showcase and the
// HUD's features import it.
const instrumentation = vi.hoisted(() => ({ mounted: new Set<string>() }))
vi.mock('../../../apps/hud/design', async (importOriginal) => {
  const { instrumentComponents } = await import('../instrument')
  return instrumentComponents(await importOriginal<Record<string, unknown>>(), (name) =>
    instrumentation.mounted.add(name)
  )
})
vi.mock('../../../apps/hud/design/Glyphs', async (importOriginal) => {
  const { instrumentComponents } = await import('../instrument')
  return instrumentComponents(await importOriginal<Record<string, unknown>>(), (name) =>
    instrumentation.mounted.add(name)
  )
})

type Section = { name: string; components: readonly string[]; render: () => ReactNode }

// The glyphs are the HUD's icon set, which the gallery's Glyphs page draws, so no section names them.
const components = componentExports(design, glyphs)
const glyphNames = componentExports(glyphs, {})
// Each section as a gallery page: one example, its render.
const pages = SHOWCASE_SECTIONS.map((section) => ({
  name: section.name,
  components: section.components,
  examples: [{ id: section.name, title: section.title, render: section.render }]
}))

// Sections whose named component appears only after a press: the button the test presses first.
const OPENED_BY: Record<string, string> = { AnchoredMenu: 'Open the photo options' }

// A failed claim usually means the Showcase reached the component around the design entry, which this test watches.
const AROUND_THE_ENTRY =
  'Showcase.tsx imports the primitives from the design entry (.), the one this test instruments'

function mountedBy(section: Pick<Section, 'name' | 'render'>): Set<string> {
  instrumentation.mounted.clear()
  // Mounted as an element, as the Showcase and the gallery mount it, so a section's hooks belong to the section.
  const Render = section.render
  const view = render(<Render />)
  const opener = OPENED_BY[section.name]
  if (opener) fireEvent.click(screen.getByRole('button', { name: opener }))
  const mounted = new Set(instrumentation.mounted)
  view.unmount()
  return mounted
}

const missingGlyphs = (mounted: ReadonlySet<string>) =>
  glyphNames.filter((name) => !mounted.has(name))

describe('when the Showcase is checked against the HUD design entry', () => {
  it('should find the named component exports, the glyphs apart, and no function or constant', () => {
    expect(components).toEqual(
      expect.arrayContaining(['Chip', 'Pager', 'PopupHost', 'KeyCap', 'IconButton'])
    )
    for (const name of ['VerifiedBadge', 'showDialog', 'ARM_MS', 'rarityOf'])
      expect(components).not.toContain(name)
    expect(glyphNames).toEqual(expect.arrayContaining(['VerifiedBadge', 'SocialIcon', 'Mark']))
  })

  it('should give every export that is not a component its reason, and list no other', () => {
    const others = Object.keys(design)
      .filter((name) => !isComponentExport(name, (design as Record<string, unknown>)[name]))
      .sort()
    expect(others).toEqual(Object.keys(SHOWCASE_NOT_COMPONENTS).sort())
    for (const reason of Object.values(SHOWCASE_NOT_COMPONENTS)) expect(reason.trim()).not.toBe('')
  })

  it('should name every component the design entry exports in a section, or exclude it with a reason', () => {
    expect(coverageProblems(components, pages, SHOWCASE_EXCLUSIONS)).toEqual([])
  })
})

describe('when a section names the components it renders', () => {
  it.each(SHOWCASE_SECTIONS.map((section) => [section.name, section] as const))(
    'should mount every component the %s section names',
    (_, section) => {
      expect(claimProblems(section, mountedBy(section)), AROUND_THE_ENTRY).toEqual([])
    }
  )

  it('should mount every glyph on the gallery Glyphs page', () => {
    expect(missingGlyphs(mountedBy({ name: 'Glyphs', render: () => <GlyphGrid /> }))).toEqual([])
  })
})

describe('when the Showcase has a gap', () => {
  it('should fail the components of a section it lost', () => {
    const withoutModal = pages.filter((page) => page.name !== 'Modal')
    expect(coverageProblems(components, withoutModal, SHOWCASE_EXCLUSIONS)).toEqual([
      'Modal: no example renders it and no exclusion explains why',
      'ModalActions: no example renders it and no exclusion explains why',
      'ModalShell: no example renders it and no exclusion explains why',
      'ModalTitle: no example renders it and no exclusion explains why'
    ])
  })

  it('should fail a section that draws a placeholder instead of the components it names', () => {
    const placeholder = {
      name: 'BrowsePage',
      components: ['BrowseToolbar', 'BrowsePanel'],
      render: () => <div>Browse page</div>
    }
    expect(claimProblems(placeholder, mountedBy(placeholder))).toEqual([
      'BrowsePage: claims BrowseToolbar, but no example mounts it',
      'BrowsePage: claims BrowsePanel, but no example mounts it'
    ])
  })

  it('should fail a section that names a glyph or a function', () => {
    const page = { ...pages[0]!, components: ['VerifiedBadge', 'showDialog'] }
    expect(coverageProblems(components, [...pages.slice(1), page], SHOWCASE_EXCLUSIONS)).toEqual([
      'Palette: lists VerifiedBadge, which is not a public component export',
      'Palette: lists showDialog, which is not a public component export'
    ])
  })

  it('should mount a section whose render calls a hook, as the hosts do', () => {
    const hooked = {
      name: 'Hooked',
      render: () => {
        const [label] = useState('Entry')
        return <design.Chip label={label} />
      }
    }
    expect(mountedBy(hooked).has('Chip')).toBe(true)
  })

  it('should not count a component rendered around the design entry', () => {
    const around = { name: 'Around', render: () => <ChipModule label="Direct" /> }
    const through = { name: 'Through', render: () => <design.Chip label="Entry" /> }
    expect(mountedBy(around).has('Chip')).toBe(false)
    expect(mountedBy(through).has('Chip')).toBe(true)
  })

  it('should fail a Glyphs page that leaves a glyph out', () => {
    const missing = missingGlyphs(mountedBy({ name: 'Glyphs', render: () => <glyphs.Coin /> }))
    expect(missing).toContain('VerifiedBadge')
    expect(missing).not.toContain('Coin')
  })
})
