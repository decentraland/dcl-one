import path from 'node:path'
import { Client } from '@modelcontextprotocol/sdk/client/index.js'
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'

const projectRoot = path.resolve(import.meta.dirname, '..')

type TextResult = { isError?: boolean; content: { type: string; text: string }[] }

describe('figma stdio server', () => {
  let client: Client

  beforeAll(async () => {
    client = new Client({ name: 'test-client', version: '0.0.0' })
    await client.connect(
      new StdioClientTransport({
        command: process.execPath,
        args: ['--import', 'tsx', 'src/server.ts'],
        cwd: projectRoot,
        env: { ...process.env, DCL_FIGMA_CONFIG: 'test/fixtures/figma.config.json' } as Record<
          string,
          string
        >,
        stderr: 'pipe'
      })
    )
  }, 30_000)

  afterAll(async () => {
    await client.close()
  })

  const call = async (name: string, args: Record<string, unknown> = {}) => {
    const result = (await client.callTool({ name, arguments: args })) as TextResult
    return { isError: result.isError ?? false, text: result.content[0]!.text }
  }

  describe('when listing tools', () => {
    it('should expose the maintenance and the resolve tools', async () => {
      const { tools } = await client.listTools()

      expect(tools.map((t) => t.name).sort()).toEqual([
        'figma_check_mappings',
        'figma_diff',
        'figma_export_icons',
        'figma_get_component_spec',
        'figma_get_tokens',
        'figma_list_components',
        'figma_list_pages',
        'figma_resolve_component',
        'figma_screenshot',
        'figma_sync'
      ])
    })
  })

  describe('when calling figma_get_tokens', () => {
    it('should return 83+36+16+26 variables and 32 text styles', async () => {
      const { text } = await call('figma_get_tokens')
      const result = JSON.parse(text)

      expect(result.counts).toMatchObject({
        variables: 161,
        byCollection: { Primitives: 83, Semantic: 36, Spacing: 16, Layout: 26 },
        textStyles: 32
      })
    })

    it('should reject an unknown collection with a readable error', async () => {
      const { isError, text } = await call('figma_get_tokens', { collection: 'Nope' })

      expect(isError).toBe(true)
      expect(text).toContain('Available: Primitives, Semantic, Spacing, Layout')
    })
  })

  describe('when calling figma_list_components', () => {
    it('should exclude disabled pages from generated component discovery', async () => {
      const result = JSON.parse((await call('figma_list_components', { kind: 'components' })).text)
      expect(result.components.length).toBeGreaterThan(0)
      expect(result.components.some((entry: { page: string }) => entry.page === 'Buttons')).toBe(
        false
      )
    })

    it('should only return components that need manual work when asked', async () => {
      const result = JSON.parse(
        (await call('figma_list_components', { kind: 'components', needsManualOnly: true })).text
      )

      expect(result.components.length).toBeGreaterThan(0)
      expect(
        result.components.every((entry: { needsManual: string[] }) => entry.needsManual.length > 0)
      ).toBe(true)
    })

    it('should group icon variants under one component', async () => {
      const result = JSON.parse(
        (await call('figma_list_components', { search: 'navbar/wallet' })).text
      )

      expect(result.components).toEqual([
        expect.objectContaining({
          name: 'navbar/wallet',
          variants: ['active=false', 'active=true']
        })
      ])
    })
  })

  describe('when calling figma_get_component_spec', () => {
    it('should return a ready component and its captured views', async () => {
      const result = JSON.parse(
        (await call('figma_get_component_spec', { name: 'ProgressBar' })).text
      )
      expect(result).toMatchObject({ name: 'ProgressBar', page: 'Progress Bar' })
      expect(Object.keys(result.views).length).toBeGreaterThan(0)
    })

    it('should suggest the available names for an unknown component', async () => {
      const { isError, text } = await call('figma_get_component_spec', { name: 'Nope' })

      expect(isError).toBe(true)
      expect(text).toContain('Available:')
    })
  })

  describe('when calling figma_export_icons', () => {
    it('should return the SVG markup of a named icon', async () => {
      const result = JSON.parse(
        (await call('figma_export_icons', { names: ['general/like'] })).text
      )

      expect(result.icons[0].svg).toContain('<svg')
    })

    it('should refuse to dump hundreds of icons inline', async () => {
      const { isError, text } = await call('figma_export_icons', { category: 'general' })

      expect(isError).toBe(true)
      expect(text).toContain('Narrow the filter')
    })

    it('should refuse an outDir outside the workspace', async () => {
      const { isError, text } = await call('figma_export_icons', {
        names: ['general/like'],
        outDir: '/etc/dcl'
      })

      expect(isError).toBe(true)
      expect(text).toContain('outDir must stay inside')
    })
  })

  describe('when calling figma_list_pages', () => {
    it('should list the pages and mark the ones with a configured generator', async () => {
      const result = JSON.parse((await call('figma_list_pages')).text)

      expect(result.pages).toHaveLength(57)
      expect(result.pages.find((p: { name: string }) => p.name === 'Icons').configured).toEqual({
        kind: 'icons',
        category: 'Icons'
      })
    })
  })

  describe('when calling figma_diff against the committed generated folder', () => {
    it('should answer without writing', async () => {
      const { isError, text } = await call('figma_diff')

      expect(isError).toBe(false)
      expect(JSON.parse(text).summary).toMatchObject({ icons: 474, tokens: 161 })
    })
  })
})
