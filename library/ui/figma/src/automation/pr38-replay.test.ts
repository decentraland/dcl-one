import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { beforeEach, describe, expect, it } from 'vitest'
import { loadConfig, type FigmaConfig } from '../config.js'
import { generateAll } from '../generators/index.js'
import { resolvePageIdentities, selectCapturePages, type PageState } from '../page-identity.js'
import { projectRoot, snapshotSource } from '../../test/snapshot.js'
import type { FigmaSource } from '../sources/types.js'

describe('when replaying PR 38 readiness from a9150fa', () => {
  let config: FigmaConfig
  let source: FigmaSource
  let replay: { pages: PageState[] }
  beforeEach(async () => {
    config = await loadConfig(projectRoot)
    source = snapshotSource()
    replay = JSON.parse(
      await readFile(path.join(projectRoot, 'test/fixtures/pr38-sync-state.json'), 'utf8')
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
    ).toContain('9373:1395')
  })
})
