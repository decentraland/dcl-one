import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { snapshotDir, snapshotSource, testConfig } from '../../test/snapshot.js'
import type { ComponentSpec, ResponsiveConfig } from '../ir/components.js'
import type { RawComponentPage } from '../ir/raw-components.js'
import { buildComponentSpecs } from './component-spec.js'

const NAV_BREAKPOINTS = { mobile: 'mobile', desktop: 'desktop', largeDesktop: 'largeDesktop' }

// Builds the synthetic Demo Nav with the given responsive config, against the synthetic Layout breakpoints
// (mobile 0, tablet 768, desktop 1024, large-desktop 1536).
async function demoNav(
  responsive: ResponsiveConfig | undefined,
  edit?: (data: RawComponentPage) => void
): Promise<ComponentSpec> {
  const config = await testConfig()
  const data = JSON.parse(
    await readFile(path.join(snapshotDir, 'components/Demo Nav.json'), 'utf8')
  ) as RawComponentPage
  edit?.(data)
  const components = { ...config.components, DemoNav: { ...config.components.DemoNav, responsive } }
  const specs = buildComponentSpecs({
    pages: [{ name: 'Demo Nav', category: 'Navigation', data }],
    config: { ...config, components },
    collections: await snapshotSource().getTokenCollections(),
    textStyles: [],
    icons: new Map()
  })
  return specs.find((spec) => spec.name === 'DemoNav')!
}

const build = (breakpoints: Record<string, string>, prop = 'viewport') =>
  demoNav({ prop, breakpoints })

describe('resolveResponsive', () => {
  describe('when every option of the axis names a foundations breakpoint', () => {
    it('should resolve the options in ascending order of minimum width, the narrowest at 0', async () => {
      const spec = await build(NAV_BREAKPOINTS)

      expect(spec.responsive).toEqual({
        prop: 'viewport',
        breakpoints: [
          { value: 'mobile', min: 0 },
          { value: 'desktop', min: 1024 },
          { value: 'largeDesktop', min: 1536 }
        ]
      })
    })
  })

  describe('when the component configures no responsive axis', () => {
    it('should leave the spec without one', async () => {
      expect((await demoNav(undefined)).responsive).toBeUndefined()
    })
  })

  describe('when the configured prop is not a variant prop', () => {
    it('should fail generation for an unknown prop', async () => {
      await expect(build(NAV_BREAKPOINTS, 'size')).rejects.toThrow(
        'DemoNav responsive: size is not a variant prop'
      )
    })

    it('should fail generation for a boolean prop', async () => {
      await expect(build({ false: 'mobile', true: 'desktop' }, 'signedIn')).rejects.toThrow(
        'DemoNav responsive: signedIn is not a variant prop'
      )
    })
  })

  describe('when the breakpoints do not cover exactly the options of the prop', () => {
    it('should fail generation for an option with no breakpoint', async () => {
      await expect(build({ mobile: 'mobile', desktop: 'desktop' })).rejects.toThrow(
        'DemoNav responsive: option largeDesktop names no breakpoint'
      )
    })

    it('should fail generation for a breakpoint keyed by a value the prop does not have', async () => {
      await expect(build({ ...NAV_BREAKPOINTS, tablet: 'tablet' })).rejects.toThrow(
        'DemoNav responsive: tablet is not an option of viewport'
      )
    })
  })

  describe('when an option names a breakpoint the foundations do not define', () => {
    it('should fail generation and list the known breakpoints', async () => {
      await expect(build({ ...NAV_BREAKPOINTS, desktop: 'laptop' })).rejects.toThrow(
        'DemoNav responsive: desktop names the unknown breakpoint laptop (foundations: mobile, tablet, desktop, largeDesktop)'
      )
    })
  })

  describe('when two options name the same breakpoint', () => {
    it('should fail generation, since one of them could never show', async () => {
      await expect(build({ ...NAV_BREAKPOINTS, largeDesktop: 'desktop' })).rejects.toThrow(
        'share the minimum width 1024px'
      )
    })
  })

  describe('when the narrowest option does not start at 0', () => {
    it('should fail generation, since narrower widths would show no view', async () => {
      await expect(build({ ...NAV_BREAKPOINTS, mobile: 'tablet' })).rejects.toThrow(
        'DemoNav responsive: the narrowest option mobile must start at 0px, not 768px'
      )
    })
  })

  describe('when an option lacks a combination of the other axes that another option draws', () => {
    it('should fail generation instead of typing the prop optional for an undrawn view', async () => {
      const dropped = 'viewport=mobile, signed-in=true, balances=credits, theme=dark'
      const responsive = { prop: 'viewport', breakpoints: NAV_BREAKPOINTS }

      await expect(
        demoNav(responsive, (data) => {
          const nav = data.components.find((component) => component.name === 'DemoNav')!
          nav.others = nav.others.filter((variant) => variant.name !== dropped)
        })
      ).rejects.toThrow('DemoNav responsive: viewport=mobile has no view for')
    })
  })
})
