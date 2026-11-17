import { execFile } from 'node:child_process'
import { readFile, rm } from 'node:fs/promises'
import path from 'node:path'
import { promisify } from 'node:util'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { lifecycleFixture, put, json, SNAP } from './lifecycle-fixture.js'
import { pinTestCapture } from '../../test/snapshot.js'

const execute = promisify(execFile)
describe('when recording a retirement through the CLI', () => {
  let root: string
  let run: (args: string[]) => Promise<unknown>
  beforeEach(async () => {
    ;({ root } = await lifecycleFixture())
    const config = await json(root, 'figma/figma.config.json')
    config.pages['1:2'] = { id: '1:2', name: 'Other', kind: 'components' }
    await put(root, 'figma/figma.config.json', config)
    // retire verifies the capture before it reads ownership, as fetch-source would have pinned it.
    await pinTestCapture(path.join(root, 'figma'))
    run = (args) =>
      execute(
        process.execPath,
        [
          '--import',
          import.meta.resolve('tsx'),
          path.resolve(import.meta.dirname, '../cli.ts'),
          'retire',
          ...args,
          '--reason',
          'Explicit removal'
        ],
        { cwd: path.join(root, 'figma') }
      )
  })
  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })
  it('should reject an unknown component even if a display name is provided', async () => {
    await expect(
      run(['--page-id', '1:1', '--component-id', 'missing', '--name', 'First'])
    ).rejects.toMatchObject({ stderr: expect.stringContaining('Unknown component ID') })
    expect((await json(root, `figma/ledger/retirements.json`)).retired).toEqual([])
  })
  it('should reject a known component from another page', async () => {
    await expect(run(['--page-id', '1:2', '--component-id', '2:1'])).rejects.toMatchObject({
      stderr: expect.stringContaining('does not belong')
    })
    expect((await json(root, `figma/ledger/retirements.json`)).retired).toEqual([])
  })
  it('should record exact ownership without deleting output', async () => {
    await run(['--page-id', '1:1', '--component-id', '2:1'])
    expect((await json(root, `figma/ledger/retirements.json`)).retired[0]).toMatchObject({
      pageId: '1:1',
      componentId: '2:1',
      sharedExclusions: expect.arrayContaining([expect.stringContaining('/assets/')])
    })
    expect(await readFile(path.join(root, 'components/page/First.tsx'), 'utf8')).toContain('First')
  })

  it("should write the committed ledger, outside the cache, and leave the cache's copy as fetch-source pinned it", async () => {
    const cached = await readFile(path.join(root, SNAP, 'retirements.json'))
    await run(['--page-id', '1:1', '--component-id', '2:1'])
    expect((await json(root, 'figma/ledger/retirements.json')).retired).toHaveLength(1)
    expect(await readFile(path.join(root, SNAP, 'retirements.json'))).toEqual(cached)
  })

  it.each([
    ['edited', async () => put(root, `${SNAP}/pages.json`, { pages: [] })],
    ['not materialized', async () => rm(path.join(root, SNAP, '.fds-source.json'))]
  ])(
    'should refuse to write when the cache is %s and leave the ledger unchanged',
    async (_, spoil) => {
      const ledger = await readFile(path.join(root, 'figma/ledger/retirements.json'))
      await spoil()
      await expect(run(['--page-id', '1:1', '--component-id', '2:1'])).rejects.toMatchObject({
        stderr: expect.stringContaining('fetch-source')
      })
      expect(await readFile(path.join(root, 'figma/ledger/retirements.json'))).toEqual(ledger)
    }
  )
})
