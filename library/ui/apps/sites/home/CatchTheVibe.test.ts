import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

// Vitest runs from library/ui (its config's root). jsdom applies no :hover, so the hover contract is read from
// the stylesheet; the browser check in docs/ui-standards.md proves the rendered result.
const css = readFileSync(join(process.cwd(), 'apps/sites/home/CatchTheVibe.module.css'), 'utf8')

describe('CatchTheVibe HANG OUT NOW hover', () => {
  it('should keep the primary fill on hover', () => {
    expect(css).toMatch(
      /\.hangOut:hover:not\(\[data-disabled\], \[data-loading\]\) \{\s*background-color: var\(--dcl-primary-primary\);/
    )
  })

  it('should hide the outer ring at rest', () => {
    expect(css).toMatch(
      /\.hangOut::before \{[^}]*inset: 0;[^}]*border: 4px solid var\(--dcl-neutrals-white\);[^}]*opacity: 0;[^}]*pointer-events: none;/
    )
  })

  it('should grow the ring out of the button on pointer hover', () => {
    expect(css).toMatch(
      /@media \(hover: hover\) and \(pointer: fine\) \{\s*\.hangOut:hover:not\(\[data-disabled\], \[data-loading\]\)::before \{\s*inset: -7\.5px;\s*opacity: 1;/
    )
  })

  it('should not grow the ring of a disabled or loading button', () => {
    expect(css).not.toMatch(/\.hangOut:hover::before/)
  })

  it('should drop the ring transition under reduced motion', () => {
    expect(css).toMatch(
      /@media \(prefers-reduced-motion: reduce\) \{[^}]*\.hangOut::before \{\s*transition: none;/
    )
  })
})
