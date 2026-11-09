import { beforeEach, describe, expect, it } from 'vitest'
import { loadConfig, type FigmaConfig } from './config.js'
import { configuredPages } from './page-identity.js'
import { projectRoot, testConfig, testConfigFile } from '../test/snapshot.js'
import { createApp, type App } from './app.js'

// Migration invariants only. Exact drift of the committed tree is the CLI `diff` step's job (ci.yml and
// figma-sync.yml), so a legitimate recapture never breaks this unit suite.
describe('when migrating the main capture to stable page identities', () => {
  let config: FigmaConfig
  beforeEach(async () => {
    config = await loadConfig(projectRoot)
  })
  it('should retain all 35 entries, including the five unresolved legacy entries', () => {
    expect(configuredPages(config)).toHaveLength(35)
    expect(
      configuredPages(config)
        .filter((page) => page.id === null)
        .map((page) => page.name)
    ).toEqual([
      'Theme',
      'Bars: Nav bar - Tabs bar - Footer',
      'Tags',
      'Community Streaming',
      'External'
    ])
  })
})

// The synthetic snapshot keeps a legacy capture whose page id is now claimed by a canonical page.
describe('when a legacy capture shares its page id with a canonical page', () => {
  let config: FigmaConfig
  let app: App
  beforeEach(async () => {
    config = await testConfig()
    app = await createApp(projectRoot, testConfigFile)
  })
  it('should preserve the unresolved legacy capture outside both inventory scopes', async () => {
    const legacy = 'Demo: Legacy Bars'
    const pages = configuredPages(config)
    expect(pages.find((page) => page.name === legacy)).toMatchObject({
      key: `legacy:${legacy}`,
      id: null
    })
    const source = app.createSource('snapshot')
    const saved = await source.getComponentPage(legacy)
    expect(saved?.page).toEqual({ id: '10:7', name: legacy })
    expect(saved?.components.length).toBeGreaterThan(0)
    expect((await source.getComponentPageById!('10:7', 'Demo Nav'))?.page.name).toBe('Demo Nav')
    const inventory = await source.getGenerationInventory!()
    expect(inventory).toBeDefined()
    for (const scope of ['review', 'promotion'] as const) {
      const selected = inventory![scope].map(
        (entry) => pages.find((page) => page.id === entry.pageId)?.name
      )
      expect(selected).toContain('Demo Nav')
      expect(selected).not.toContain(legacy)
    }
  })
})
