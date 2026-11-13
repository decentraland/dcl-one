import { generateTokens } from './tokens.js'
import { beforeAll, describe, expect, it } from 'vitest'
import { snapshotSource } from '../../test/snapshot.js'
import { generateAll } from './index.js'
import type { GeneratedFile } from './types.js'

const context = {}

describe('generateTokens', () => {
  let files: Map<string, string>

  beforeAll(async () => {
    const result = await generateAll(snapshotSource(), context)
    files = new Map(result.files.map((file: GeneratedFile) => [file.path, file.content]))
  })

  describe('when placing the foundations in the cascade', () => {
    it('should put the tokens and text styles in the foundations layer, after the full layer order', () => {
      for (const path of ['tokens/tokens.css', 'tokens/text-styles.module.css']) {
        const css = files.get(path)!
        expect(css).toContain(
          '@layer dcl.reset, dcl.foundations, dcl.components;\n\n@layer dcl.foundations {\n'
        )
        expect(css.indexOf('@layer dcl.reset, dcl.foundations, dcl.components;')).toBeLessThan(
          css.indexOf('{')
        )
        expect(css.trimEnd().endsWith('}')).toBe(true)
      }
    })
  })

  describe('when generating the CSS variables', () => {
    it('should keep semantic tokens as references to primitives', () => {
      expect(files.get('tokens/tokens.css')).toContain('--dcl-bg-page: var(--dcl-neutral-dark);')
    })

    it('should render transparent primitives as rgba', () => {
      expect(files.get('tokens/tokens.css')).toContain(
        '--dcl-alpha-white-10: rgba(255, 255, 255, 0.1);'
      )
    })

    it('should start from the mobile layout and only emit what changes at each breakpoint', () => {
      const css = files.get('tokens/tokens.css')!

      expect(css).toContain('--dcl-font-size-heading-xl: 32px;')
      expect(css).toMatch(/@media \(min-width: 768px\) \{[^}]*--dcl-font-size-heading-xl: 60px;/)
      expect(css).not.toMatch(/@media \(min-width: 1536px\)/)
    })

    it('should not expose the breakpoint token as a variable', () => {
      expect(files.get('tokens/tokens.css')).not.toContain('breakpoint-min')
    })
  })

  describe('when generating the TypeScript tokens', () => {
    it('should resolve semantic aliases to raw colors', () => {
      expect(files.get('tokens/tokens.ts')).toMatch(/bg: \{\s+page: '#161518'/)
    })

    it('should expose breakpoints sorted from mobile to large desktop', () => {
      expect(files.get('tokens/tokens.ts')).toMatch(
        /breakpoints = \{\s+mobile: 0,\s+tablet: 768,\s+desktop: 1024,\s+largeDesktop: 1536\s+\}/
      )
    })

    it('should type every text style, including uppercase buttons', () => {
      const tokens = files.get('tokens/tokens.ts')!

      expect(tokens).toMatch(/'button\/md': \{[^}]*textTransform: 'uppercase'/)
      expect(tokens).toMatch(/caption: \{[^}]*lineHeight: 1\.4,/)
    })

    it('should build gradients with the captured angle', () => {
      expect(files.get('tokens/tokens.ts')).toContain(
        'linear-gradient(219deg, #ff2d55 0%, #ffbc5b 100%)'
      )
    })
  })

  describe('when generating the theme', () => {
    it('should only reference CSS variables that tokens.css defines', () => {
      const defined = new Set(
        [...files.get('tokens/tokens.css')!.matchAll(/^\s*(--dcl-[\w-]+):/gm)].map((m) => m[1])
      )
      const referenced = [...files.get('tokens/theme.ts')!.matchAll(/var\((--dcl-[\w-]+)\)/g)].map(
        (m) => m[1]!
      )

      // Every Primitives, Semantic, Spacing and Layout variable but the breakpoint, plus both gradients.
      expect(referenced.length).toBe(29)
      expect(referenced.filter((name) => !defined.has(name))).toEqual([])
    })

    it('should expose responsive layout and font-size references with their real names', () => {
      const theme = files.get('tokens/theme.ts')!

      expect(theme).toContain("marginX: 'var(--dcl-layout-margin-x)'")
      expect(theme).toContain("labelSmCaps: 'var(--dcl-font-size-label-sm-caps)'")
      expect(theme).toContain("sunset: 'var(--dcl-gradient-sunset)'")
    })

    it('should expose CSS variable references only', () => {
      const theme = files.get('tokens/theme.ts')!

      expect(theme).toContain("page: 'var(--dcl-bg-page)'")
      expect(theme).not.toMatch(/#[0-9a-f]{6}/)
    })
  })

  describe('when two tokens would share a CSS variable name', () => {
    it('should fail loudly instead of overwriting one of them', async () => {
      const source = snapshotSource()
      const collections = await source.getTokenCollections()
      const duplicate = {
        ...collections[1]!,
        variables: [...collections[1]!.variables, { ...collections[1]!.variables[0]! }]
      }
      const broken = Object.assign(Object.create(source), {
        getTokenCollections: async () => [
          collections[0]!,
          duplicate,
          collections[2]!,
          collections[3]!
        ]
      })

      await expect(generateAll(broken, context)).rejects.toThrow('CSS variable collision')
    })
  })
})

describe('when a Figma font family contains CSS syntax', () => {
  it('should keep quotes and newlines inside the font family string', async () => {
    const files = generateTokens(
      {
        collections: await snapshotSource().getTokenCollections(),
        gradients: [],
        textStyles: [
          {
            name: 'body',
            fontFamily: "A'; color:red;/*\nB",
            fontStyle: 'Regular',
            fontSize: 16,
            lineHeightPercent: 150,
            letterSpacingPx: 0,
            textCase: 'ORIGINAL'
          }
        ]
      },
      {}
    )
    const css = files.find((file) => file.path === 'tokens/text-styles.module.css')!.content
    expect(css).toContain("font-family: 'A\\27 ; color:red;/*\\a B', sans-serif;")
    expect(css).not.toContain("A'; color:red")
  })
})
