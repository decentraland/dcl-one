import { beforeEach, describe, expect, it } from 'vitest'
import { loadConfig, type FigmaConfig } from './config.js'
import { configuredPages } from './page-identity.js'
import { projectRoot } from '../test/snapshot.js'
import { createApp, type App } from './app.js'

// Migration invariants only. Exact drift of the committed tree is the CLI `diff` step's job (ci.yml and
// figma-sync.yml), so a legitimate recapture never breaks this unit suite.
describe('when migrating the main capture to stable page identities', () => {
  let config: FigmaConfig
  let app: App
  beforeEach(async () => {
    config = await loadConfig(projectRoot)
    app = await createApp(projectRoot)
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
  it('should preserve the unresolved legacy Bars capture outside both inventory scopes', async () => {
    const legacy = 'Bars: Nav bar - Tabs bar - Footer'
    const pages = configuredPages(config)
    expect(pages.find((page) => page.name === legacy)).toMatchObject({
      key: `legacy:${legacy}`,
      id: null
    })
    const source = app.createSource('snapshot')
    const saved = await source.getComponentPage(legacy)
    expect(saved?.page).toEqual({ id: '8711:15316', name: legacy })
    expect(saved?.components.length).toBeGreaterThan(0)
    expect((await source.getComponentPageById!('8711:15316', 'Nav bar'))?.page.name).toBe('Nav bar')
    const inventory = await source.getGenerationInventory!()
    expect(inventory).toBeDefined()
    for (const scope of ['review', 'promotion'] as const) {
      const selected = inventory![scope].map(
        (entry) => pages.find((page) => page.id === entry.pageId)?.name
      )
      expect(selected).not.toContain(legacy)
    }
  })
})
