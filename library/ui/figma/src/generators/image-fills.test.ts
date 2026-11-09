import { describe, expect, it } from 'vitest'
import { componentSpecs, projectRoot, snapshotSource, testConfig } from '../../test/snapshot.js'
import { configSchema, loadConfig } from '../config.js'
import { externalizeSvgs, generateComponents } from './components.js'
import { IMAGE_PLACEHOLDER, replaceRasterFills } from './image-fills.js'
import { generateAll } from './index.js'

const context = {}
const svgUrl = (svg: string) => `url("data:image/svg+xml,${encodeURIComponent(svg)}")`
const raster = (size: number) =>
  `<svg xmlns="http://www.w3.org/2000/svg"><image width="4" height="4" href="data:image/png;base64,${'A'.repeat(size)}"/></svg>`
const vector = (size: number) =>
  `<svg xmlns="http://www.w3.org/2000/svg">${'<path d="M0 0h20v20H0z"/>'.repeat(Math.ceil(size / 25))}</svg>`
const placeholderCss = (css: string) => {
  const assets = new Map<string, string>()
  return { css: externalizeSvgs(replaceRasterFills(css).css, assets), assets }
}

describe('the imageFills config', () => {
  const minimal = { outDir: 'out', pages: {} }

  it('should default to assets when omitted or undefined', () => {
    expect(configSchema.parse(minimal).imageFills).toBe('assets')
    expect(configSchema.parse({ ...minimal, imageFills: undefined }).imageFills).toBe('assets')
  })

  it('should accept placeholder', () => {
    expect(configSchema.parse({ ...minimal, imageFills: 'placeholder' }).imageFills).toBe(
      'placeholder'
    )
  })

  it.each([['none'], [''], [null], ['Placeholder'], [true]])('should reject %j', (value) => {
    expect(() => configSchema.parse({ ...minimal, imageFills: value })).toThrow()
  })

  // This tree publishes no Figma image exports, so its config selects the placeholder.
  it('should set the repository config to the placeholder', async () => {
    expect((await loadConfig(projectRoot)).imageFills).toBe('placeholder')
  })
})

describe('replaceRasterFills', () => {
  describe('when a background embeds a raster', () => {
    it.each([
      ['a large SVG with a raster image', svgUrl(raster(6000))],
      ['a small SVG with a raster image', svgUrl(raster(10))],
      ['an xlink raster image', svgUrl(raster(10).replace(' href=', ' xlink:href='))],
      [
        'a base64 SVG with a raster image',
        `url("data:image/svg+xml;base64,${Buffer.from(raster(10)).toString('base64')}")`
      ],
      ['a direct PNG', 'url("data:image/png;base64,iVBORw0KGgo=")'],
      ['a direct JPEG', "url('data:image/jpeg;base64,/9j/4AAQ')"],
      ['a direct GIF', 'url(data:image/gif;base64,R0lGODlh)'],
      ['a direct WebP', 'url( "data:image/webp;base64,UklGRg==" )']
    ])('should replace %s with the placeholder and publish no asset', (_, url) => {
      const { css, assets } = placeholderCss(
        `.a {\n  background: ${url} center / contain no-repeat;\n}\n`
      )

      expect(css).toBe(`.a {\n  background: ${IMAGE_PLACEHOLDER} center / contain no-repeat;\n}\n`)
      expect(assets.size).toBe(0)
    })

    it('should replace a background-image and count every replacement', () => {
      const result = replaceRasterFills(
        `.a { background-image: ${svgUrl(raster(10))}; }\n.b { background: ${svgUrl(raster(10))}; }\n`
      )

      expect(result).toEqual({
        css: `.a { background-image: ${IMAGE_PLACEHOLDER}; }\n.b { background: ${IMAGE_PLACEHOLDER}; }\n`,
        replaced: 2
      })
    })

    it('should replace only the raster layer of a multi-layer background', () => {
      const small = svgUrl(vector(100))
      const css = `.a { background: ${svgUrl(raster(10))} 0 0 / 10px 20px repeat-x, ${small} center / cover no-repeat, linear-gradient(red, blue); }\n`

      expect(replaceRasterFills(css).css).toBe(
        `.a { background: ${IMAGE_PLACEHOLDER} 0 0 / 10px 20px repeat-x, ${small} center / cover no-repeat, linear-gradient(red, blue); }\n`
      )
    })

    it('should not mistake a nested selector block for a declaration', () => {
      const css = `.a {\n  color: red;\n  &::after {\n    background: ${svgUrl(raster(10))};\n  }\n}\n`

      expect(replaceRasterFills(css).css).toBe(
        `.a {\n  color: red;\n  &::after {\n    background: ${IMAGE_PLACEHOLDER};\n  }\n}\n`
      )
    })
  })

  describe('when nothing embeds a raster', () => {
    it.each([
      ['a small vector SVG', `.a { background: ${svgUrl(vector(100))}; }\n`],
      ['a large vector SVG', `.a { background: ${svgUrl(vector(6000))}; }\n`],
      [
        'an image nesting a vector SVG',
        `.a { background: ${svgUrl(`<svg xmlns="http://www.w3.org/2000/svg"><image width="48" height="64" href='data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%2F%3E'/></svg>`)}; }\n`
      ],
      [
        'a raster named only in a comment',
        '.a { color: red; /* url("data:image/png;base64,AA") */ }\n'
      ],
      ['a raster named only in a string', '.a { content: "url(data:image/png;base64,AA)"; }\n'],
      ['an external URL', '.a { background: url("https://example.com/a.png"); }\n'],
      [
        'raster text in an SVG id',
        `.a { background: ${svgUrl('<svg xmlns="http://www.w3.org/2000/svg"><g id="data:image/png;base64,AA"/></svg>')}; }\n`
      ]
    ])('should leave %s unchanged', (_, css) => {
      expect(replaceRasterFills(css)).toEqual({ css, replaced: 0 })
    })

    it('should leave an image nesting a base64 vector SVG unchanged', () => {
      const nested = Buffer.from(vector(100)).toString('base64')
      const css = `.a { background: ${svgUrl(`<svg xmlns="http://www.w3.org/2000/svg"><image href="data:image/svg+xml;base64,${nested}"/></svg>`)}; }\n`

      expect(replaceRasterFills(css)).toEqual({ css, replaced: 0 })
    })

    it('should preserve escapes inside comments and ordinary strings', () => {
      const css = '.a { color: red; /* \\75rl(x) */ }\n.b::before { content: "\\201C"; }\n'

      expect(replaceRasterFills(css)).toEqual({ css, replaced: 0 })
    })

    it('should still publish a large vector SVG as an asset', () => {
      const { css, assets } = placeholderCss(`.a { background: ${svgUrl(vector(6000))}; }\n`)

      expect(assets.size).toBe(1)
      expect(css).toMatch(/^\.a \{ background: url\("\.\.\/assets\/[0-9a-f]{16}\.svg"\); \}\n$/)
    })

    it('should leave a vector mask, including a conic mask, unchanged', () => {
      const css = `.a { mask-image: ${svgUrl(vector(100))}; mask: ${svgUrl(vector(6000))} 0 0 / 100% 100% no-repeat; }\n`

      expect(replaceRasterFills(css)).toEqual({ css, replaced: 0 })
    })
  })

  describe('when a raster cannot be replaced safely', () => {
    it.each([
      ['small', raster(10)],
      ['large', raster(6000)]
    ])('should refuse an unsafe %s raster SVG', (_, svg) => {
      const unsafe = svg.replace('</svg>', '<script>alert(1)</script></svg>')

      expect(() => replaceRasterFills(`.a { background: ${svgUrl(unsafe)}; }`)).toThrow(
        'Refusing to replace an unsafe SVG image fill: element <script> is not allowed'
      )
    })

    it('should refuse a raster hidden in an SVG comment', () => {
      const svg = '<svg xmlns="http://www.w3.org/2000/svg"><!-- data:image/png;base64,AA --></svg>'

      expect(() => replaceRasterFills(`.a { background: ${svgUrl(svg)}; }`)).toThrow(
        'Refusing to replace an unsafe SVG image fill:'
      )
    })

    it('should refuse a raster two nested SVGs deep', () => {
      const inner = encodeURIComponent(raster(10))
      const middle = encodeURIComponent(
        `<svg xmlns="http://www.w3.org/2000/svg"><image href="data:image/svg+xml,${inner}"/></svg>`
      )
      const svg = `<svg xmlns="http://www.w3.org/2000/svg"><image href="data:image/svg+xml,${middle}"/></svg>`

      expect(() => replaceRasterFills(`.a { background: ${svgUrl(svg)}; }`)).toThrow(
        'Refusing to replace an unsafe SVG image fill:'
      )
    })

    it('should refuse a nested raster that follows a parenthesis in the payload', () => {
      const inner = encodeURIComponent(
        raster(10).replace('<image', '<rect fill="url(#p)"/><pattern id="p"/><image')
      )
      const svg = `<svg xmlns="http://www.w3.org/2000/svg"><image href="data:image/svg+xml,${inner}"/></svg>`

      expect(() => replaceRasterFills(`.a { background: ${svgUrl(svg)}; }`)).toThrow(
        'Refusing to replace an unsafe SVG image fill:'
      )
    })

    it('should refuse a raster inside a nested base64 SVG', () => {
      const nested = Buffer.from(raster(10)).toString('base64')
      const svg = `<svg xmlns="http://www.w3.org/2000/svg"><image href="data:image/svg+xml;base64,${nested}"/></svg>`

      expect(() => replaceRasterFills(`.a { background: ${svgUrl(svg)}; }`)).toThrow(
        'Refusing to replace an unsafe SVG image fill:'
      )
    })

    it.each([
      ['a malformed percent-encoded', 'data:image/svg+xml,%E0%A4%A'],
      ['a malformed base64', 'data:image/svg+xml;base64,@@not-base64@@'],
      ['a truncated base64', 'data:image/svg+xml;base64,PHN2Zz'],
      ['an unquoted-context', 'x data:image/svg+xml,%3Csvg%2F%3E']
    ])('should refuse %s nested SVG payload', (_, href) => {
      const svg = `<svg xmlns="http://www.w3.org/2000/svg"><image href="${href}"/></svg>`

      expect(() => replaceRasterFills(`.a { background: ${svgUrl(svg)}; }`)).toThrow(
        'Refusing to classify a malformed SVG payload'
      )
    })

    it.each([
      ['background', '.a { background: \\75rl("data:image/png;base64,AA"); }'],
      ['background-image', '.a { background-image: \\55 RL(data:image/png;base64,AA); }'],
      ['mask-image', '.a { mask-image: u\\72l("data:image/png;base64,AA"); }']
    ])('should refuse an escaped url() spelling in %s', (_, css) => {
      expect(() => replaceRasterFills(css)).toThrow('Refusing to classify an escaped CSS token')
    })

    it.each([['image-set'], ['-webkit-image-set'], ['IMAGE-SET']])(
      'should replace raster image source strings in %s',
      (name) => {
        const css = `.a { background-image: ${name}("data:image/png;base64,AA" 1x, 'data:image/svg+xml,${encodeURIComponent(raster(10))}' 2x, "data:image/svg+xml,${encodeURIComponent(vector(100))}" 3x); }\n`

        expect(replaceRasterFills(css)).toEqual({
          css: `.a { background-image: ${name}(${IMAGE_PLACEHOLDER} 1x, ${IMAGE_PLACEHOLDER} 2x, "data:image/svg+xml,${encodeURIComponent(vector(100))}" 3x); }\n`,
          replaced: 2
        })
      }
    )

    it.each([
      ['image-set', '.a { background-image: image-set("data:image\\2f png;base64,AA" 1x); }'],
      [
        '-webkit-image-set',
        ".a { background: -webkit-image-set('d\\61ta:image/png;base64,AA' 1x); }"
      ]
    ])('should refuse an escaped image source string in %s', (_, css) => {
      expect(() => replaceRasterFills(css)).toThrow('Refusing to classify an escaped image source')
    })

    it('should refuse a raster image source string outside a background', () => {
      expect(() =>
        replaceRasterFills('.a { mask-image: image-set("data:image/png;base64,AA" 1x); }')
      ).toThrow('Placeholder image fills only replace backgrounds, refusing a raster in mask-image')
    })

    it.each([
      ['raster text', '.a::before { content: "data:image/png;base64,AA"; }\n'],
      ['escaped raster text', '.a::before { content: "data:image\\2f png;base64,AA"; }\n'],
      ['an escaped comment', '.a { color: red; /* image-set("data:image\\2f png" 1x) */ }\n'],
      ['a font format string', '@font-face { src: url(a.woff2) format("woff2"); }\n']
    ])('should leave ordinary strings with %s unchanged', (_, css) => {
      expect(replaceRasterFills(css)).toEqual({ css, replaced: 0 })
    })

    it('should refuse a raster inside a nested SVG', () => {
      const inner = encodeURIComponent(raster(10))
      const svg = `<svg xmlns="http://www.w3.org/2000/svg"><image href="data:image/svg+xml,${inner}"/></svg>`

      expect(() => replaceRasterFills(`.a { background: ${svgUrl(svg)}; }`)).toThrow(
        'Refusing to replace an unsafe SVG image fill:'
      )
    })

    it.each([
      ['mask-image', `.a { mask-image: ${svgUrl(raster(10))}; }`],
      ['mask', `.a { mask: url("data:image/png;base64,AA") 0 0 / 1px 1px; }`],
      ['border-image', `.a { border-image: url("data:image/png;base64,AA") 1; }`]
    ])('should refuse a raster in %s instead of substituting it', (property, css) => {
      expect(() => replaceRasterFills(css)).toThrow(
        `Placeholder image fills only replace backgrounds, refusing a raster in ${property}`
      )
    })

    it.each([
      ['malformed encoding', '.a { background: url("data:image/svg+xml,%E0%A4%A"); }'],
      ['an escaped URL', '.a { background: url("data:image\\2f png;base64,AA"); }'],
      ['an unterminated URL', '.a { background: url("data:image/png;base64,AA'],
      ['an unterminated string', '.a { content: "abc'],
      ['an unterminated comment', '.a { color: red; /* url(data:image/png;base64,AA)']
    ])('should refuse %s', (_, css) => {
      expect(() => replaceRasterFills(css)).toThrow()
    })
  })
})

describe('generateComponents with imageFills', () => {
  it('should emit identical files when the mode is omitted or assets', async () => {
    const specs = await componentSpecs()
    const omitted = generateComponents(specs, context)
    const assets = generateComponents(specs, { ...context, imageFills: 'assets' })

    expect(assets.files).toEqual(omitted.files)
    expect(omitted.imageFills).toEqual({ mode: 'assets', replaced: [] })
  })

  it('should replace raster fills, write no raster asset and report the replacements', async () => {
    const specs = await componentSpecs()
    const output = generateComponents(specs, { ...context, imageFills: 'placeholder' })
    const assets = output.files.filter((file) => file.path.startsWith('components/assets/'))
    const css = output.files.filter((file) => file.path.endsWith('.module.css'))

    expect(output.imageFills.mode).toBe('placeholder')
    expect(output.imageFills.replaced.length).toBeGreaterThan(0)
    expect(output.imageFills.replaced.every((entry) => entry.count > 0)).toBe(true)
    expect(assets.filter((file) => /data:image\/(?!svg)/.test(file.content))).toEqual([])
    expect(css.filter((file) => /data:image\/(?!svg)/.test(file.content))).toEqual([])
    expect(css.some((file) => file.content.includes(IMAGE_PLACEHOLDER))).toBe(true)
  })
})

describe('generateAll with imageFills', () => {
  it('should emit byte-identical files for an omitted and an explicit assets mode', async () => {
    const config = { ...(await testConfig()), selection: undefined }
    const omitted: Partial<typeof config> = { ...config }
    delete omitted.imageFills
    const explicit = await generateAll(snapshotSource(), context, {
      ...config,
      imageFills: 'assets'
    })
    const implicit = await generateAll(snapshotSource(), context, omitted as typeof config)

    expect(implicit.files).toEqual(explicit.files)
    expect(explicit.summary.imageFills).toEqual({ mode: 'assets', replaced: [] })
  })

  it('should pass the configured placeholder mode to component generation', async () => {
    const config = { ...(await testConfig()), selection: undefined }
    const result = await generateAll(snapshotSource(), context, {
      ...config,
      imageFills: 'placeholder'
    })

    const components = result.files.filter((file) => file.path.startsWith('components/'))

    expect(result.summary.imageFills.mode).toBe('placeholder')
    expect(result.summary.imageFills.replaced.length).toBeGreaterThan(0)
    expect(components.filter((file) => /data:image\/(?!svg)/.test(file.content))).toEqual([])
  })
})
