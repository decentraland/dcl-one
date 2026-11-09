import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { beforeEach, describe, expect, it } from 'vitest'
import type { FigmaConfig } from '../config.js'
import { generateAll } from '../generators/index.js'
import { resolvePageIdentities, selectCapturePages, type PageState } from '../page-identity.js'
import { fixturesDir, snapshotSource, testConfig } from '../../test/snapshot.js'
import type { FigmaSource } from '../sources/types.js'

// A synthetic replay of a readiness change that renames ready pages and flips readiness elsewhere.
describe('when replaying a readiness change that renames ready pages', () => {
  let config: FigmaConfig
  let source: FigmaSource
  let replay: { pages: PageState[] }
  beforeEach(async () => {
    config = await testConfig()
    source = snapshotSource()
    replay = JSON.parse(
      await readFile(path.join(fixturesDir, 'readiness-replay-sync-state.json'), 'utf8')
    )
  })
  it('should preserve every generated file and its content regardless of readiness', async () => {
    const before = await generateAll(source, {}, config)
    source.getSyncState = async () => replay
    expect((await generateAll(source, {}, config)).files).toEqual(before.files)
  })
  it('should select renamed ready pages by their configured IDs', async () => {
    expect(
      selectCapturePages(resolvePageIdentities(config, replay.pages))
        .filter((page) => page.renamed)
        .map((page) => page.id)
    ).toEqual(['10:7', '10:6'])
  })
})
