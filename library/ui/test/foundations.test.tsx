import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it } from 'vitest'
import { NavbarCredits, iconComponents, iconMeta, theme } from '../index'
import fontSource from '../foundations/typography/font-source.json'

const foundationRoot = path.resolve(import.meta.dirname, '../foundations')
const layerOrder = '@layer dcl.reset, dcl.foundations, dcl.components;'

describe('when a consumer loads shared foundations', () => {
  describe.each([
    'foundations.css',
    'colors/colors.css',
    'spacing/spacing.css',
    'layout/layout.css',
    'typography/inter.css',
    'typography/text-styles.module.css'
  ])('and the stylesheet is %s', (file) => {
    let source: string

    beforeEach(async () => {
      source = await readFile(path.join(foundationRoot, file), 'utf8')
    })

    it('should establish the shared cascade before any import or declaration', () => {
      expect(
        source
          .replace(/\/\*[\s\S]*?\*\//g, '')
          .trimStart()
          .startsWith(layerOrder)
      ).toBe(true)
    })
  })

  describe('and a theme references the loaded CSS variables', () => {
    let declarations: Set<string>
    let references: string[]

    beforeEach(async () => {
      declarations = new Set(
        (
          await Promise.all(
            ['colors/colors.css', 'spacing/spacing.css', 'layout/layout.css'].map((file) =>
              readFile(path.join(foundationRoot, file), 'utf8')
            )
          )
        )
          .join('\n')
          .match(/--dcl-[a-z0-9-]+(?=\s*:)/g)
      )
      references = [...JSON.stringify(theme).matchAll(/var\((--dcl-[a-z0-9-]+)\)/g)].map(
        (match) => match[1]!
      )
    })

    it('should resolve every public theme reference from the shared stylesheets', () => {
      expect(references.filter((reference) => !declarations.has(reference))).toEqual([])
    })
  })

  describe('and an informative icon is rendered', () => {
    beforeEach(() => {
      render(<NavbarCredits role="img" aria-label="Credits" size={24} />)
    })

    it('should preserve accessible SVG attributes and the requested dimensions', () => {
      expect(screen.getByRole('img', { name: 'Credits' })).toHaveAttribute('width', '24')
      expect(screen.getByRole('img', { name: 'Credits' })).toHaveAttribute('height', '24')
    })

    it('should expose every captured icon through the public registry', () => {
      expect(iconMeta.filter((icon) => !Object.hasOwn(iconComponents, icon.component))).toEqual([])
    })
  })

  describe.each(Object.entries(fontSource.files))(
    'and the bundled font is %s',
    (file, pinnedDigest) => {
      let payload: Buffer
      let indexedOid: string | undefined

      beforeEach(async () => {
        payload = await readFile(path.join(foundationRoot, 'typography/fonts', file))
        indexedOid = execFileSync(
          'git',
          ['show', `:library/ui/foundations/typography/fonts/${file}`],
          { encoding: 'utf8' }
        ).match(/^oid sha256:([a-f0-9]{64})$/m)?.[1]
      })

      it('should load a WOFF2 payload rather than an unmaterialized LFS pointer', () => {
        expect(payload.subarray(0, 4).toString()).toBe('wOF2')
      })

      it('should match both the committed LFS identity and the pinned package payload', () => {
        expect(createHash('sha256').update(payload).digest('hex')).toBe(pinnedDigest)
        expect(indexedOid).toBe(pinnedDigest)
      })
    }
  )

  describe('and the bundled font origin is inspected', () => {
    let license: Buffer

    beforeEach(async () => {
      license = await readFile(path.join(foundationRoot, 'typography', fontSource.licenseFile))
    })

    it('should identify the licensed npm source independently from the Figma capture', () => {
      expect({
        package: fontSource.package,
        version: fontSource.version,
        license: fontSource.license
      }).toEqual({ package: '@fontsource-variable/inter', version: '5.3.0', license: 'OFL-1.1' })
    })
    it('should retain the complete pinned font license', () => {
      expect(createHash('sha256').update(license).digest('hex')).toBe(fontSource.licenseSha256)
    })
  })
})
