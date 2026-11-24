import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

// Vitest runs from library/ui (its config's root). jsdom applies no :hover, so the hover contract is read from
// the stylesheet; the browser check in docs/ui-standards.md proves the rendered result.
const css = readFileSync(
  join(process.cwd(), 'apps/sites/download/DownloadActions.module.css'),
  'utf8'
)

describe('DownloadActions landing CTA hover', () => {
  it('should draw the white ring outside the button on pointer hover', () => {
    expect(css).toMatch(
      /@media \(hover: hover\) and \(pointer: fine\) \{[^@]*\.sourceAction:hover:not\(:focus-visible\) \{\s*outline-color: var\(--dcl-neutrals-white\);/
    )
  })

  it('should keep the primary fill on hover and press', () => {
    expect(css).toMatch(
      /\.sourceActions \.sourceAction:is\(:hover, :active\):not\(:disabled\) \{\s*background: var\(--dcl-action-primary\);/
    )
  })

  it('should keep the Epic fill white on hover and press', () => {
    expect(css).toMatch(
      /\.sourceActions \.sourceEpic:is\(:hover, :active\):not\(:disabled\) \{\s*background: var\(--dcl-neutrals-white\);/
    )
  })

  it('should drop the ring transition under reduced motion', () => {
    expect(css).toMatch(
      /@media \(prefers-reduced-motion: reduce\) \{\s*\.sourceAction:not\(:focus-visible\) \{\s*transition: none;/
    )
  })
})
