import { createHash } from 'node:crypto'
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { ShopCart, theme } from '@dcl/one-ui/foundations'
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { INTER_FILES, SHARED_ALIASES, SHARED_FONTS, SHARED_TREES } from '../../vite.shared'

// The shared design system (library/ui) as the HUD compiles it: through vite.shared.ts's aliases, with its Inter
// faces taken from this app's @fontsource-variable/inter.
const typography = resolve(__dirname, '../../../../../../library/ui/foundations/typography')
const pinned = JSON.parse(readFileSync(join(typography, 'font-source.json'), 'utf8')) as {
  package: string
  version: string
  files: Record<string, string>
}
const find = SHARED_FONTS.find as RegExp

function sources(path: string): string[] {
  if (statSync(path).isFile()) return [path]
  return readdirSync(path, { withFileTypes: true }).flatMap((entry) =>
    entry.name === 'node_modules' ? [] : sources(join(path, entry.name))
  )
}

describe('the shared UI foundations in the HUD', () => {
  it('compile from the public foundations entry and render with this app’s React', () => {
    expect(theme.palette.primary.primary).toBe('var(--dcl-primary-primary)')
    const { getByRole } = render(<ShopCart role="img" aria-label="Cart" size={24} />)
    expect(getByRole('img', { name: 'Cart' }).getAttribute('width')).toBe('24')
  })

  it('take Inter from this app’s package, at the bytes library/ui pins', () => {
    const installed = JSON.parse(readFileSync(join(INTER_FILES, '../package.json'), 'utf8')) as { name: string; version: string }
    expect(`${installed.name}@${installed.version}`).toBe(`${pinned.package}@${pinned.version}`)
    for (const [name, sha256] of Object.entries(pinned.files)) {
      expect(createHash('sha256').update(readFileSync(join(INTER_FILES, name))).digest('hex'), name).toBe(sha256)
    }
  })

  it('reach the app and test builds through the shared aliases', () => {
    expect(SHARED_ALIASES).toContain(SHARED_FONTS)
    for (const config of ['vite.config.ts', 'vitest.config.ts']) {
      expect(readFileSync(resolve(__dirname, '../..', config), 'utf8'), config).toContain('alias: SHARED_ALIASES')
    }
  })

  it('redirect every face inter.css registers', () => {
    const urls = [...readFileSync(join(typography, 'inter.css'), 'utf8').matchAll(/url\(([^)]+)\)/g)].map((m) => m[1])
    expect(urls.sort()).toEqual(Object.keys(pinned.files).map((name) => `./fonts/${name}`).sort())
    for (const url of urls) expect(url).toMatch(find)
  })

  it('redirect no other source: inter.css is the only one naming those files', () => {
    const anywhere = new RegExp(find.source.replace(/^\^|\$$/g, ''))
    const naming = [resolve(__dirname, '..'), ...SHARED_TREES]
      .flatMap(sources)
      .filter((path) => /\.(css|tsx?)$/.test(path) && path !== __filename)
      .filter((path) => anywhere.test(readFileSync(path, 'utf8')))
    expect(naming).toEqual([join(typography, 'inter.css')])
  })
})
