import { execFile } from 'node:child_process'
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { promisify } from 'node:util'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'

const execute = promisify(execFile)

describe('when the CLI receives an explicit configuration outside its working directory', () => {
  let directory: string
  let cwd: string
  let config: string
  let chunk: string
  let run: (args: string[]) => Promise<string>

  beforeEach(async () => {
    directory = await mkdtemp(path.join(tmpdir(), 'figma-cli-config-'))
    cwd = path.join(directory, 'unrelated')
    await mkdir(cwd)
    config = path.join(directory, 'figma.config.json')
    chunk = path.join(directory, 'capture.json')
    await writeFile(
      config,
      JSON.stringify({
        pages: {},
        snapshotDir: 'captures',
        snapshotLock: 'captures.lock.json',
        outDir: 'output'
      })
    )
    await writeFile(
      chunk,
      JSON.stringify({
        pages: [{ page: { id: 'ready-page', name: 'Ready' }, components: [] }],
        resume: null
      })
    )
    run = async (args) =>
      (
        await execute(
          process.execPath,
          [
            '--import',
            import.meta.resolve('tsx'),
            path.join(import.meta.dirname, 'cli.ts'),
            ...args,
            '--config',
            config
          ],
          { cwd }
        )
      ).stdout
  })

  afterEach(async () => {
    await rm(directory, { recursive: true, force: true })
  })

  // Capture writing and chunk ingest are gone: the live file is only read for inspection.
  it.each(['pull-live', 'capture-ingest', 'capture-merge'])(
    'should not offer the removed %s command',
    async (command) => {
      await expect(run([command, chunk])).rejects.toMatchObject({
        stderr: expect.stringContaining('Usage:')
      })
      await expect(
        readFile(path.join(directory, 'captures/components/Ready.json'))
      ).rejects.toThrow()
    }
  )

  it('should refuse installing from the live source', async () => {
    await expect(run(['install', 'foundations', '--source', 'rest'])).rejects.toMatchObject({
      stderr: expect.stringContaining('never rest')
    })
  })

  it('should reject an unknown install target', async () => {
    await expect(run(['install', 'unknown'])).rejects.toMatchObject({
      stderr: expect.stringContaining('Unknown install target')
    })
  })

  it.each([['install'], ['install', 'foundations', 'extra']])(
    'should reject the incomplete install arguments %j',
    async (...args) => {
      await expect(run(args)).rejects.toMatchObject({ stderr: expect.stringContaining('Usage:') })
    }
  )

  it('should refuse sync from the live source, which only diff may read', async () => {
    await expect(run(['sync', '--source', 'rest'])).rejects.toMatchObject({
      stderr: expect.stringContaining('reads only the verified fetch-source cache')
    })
  })

  it('should resolve the sync capture pin against the explicit configuration', async () => {
    await expect(run(['sync', '--developer'])).rejects.toMatchObject({
      stderr: expect.stringContaining(path.join(directory, 'captures.lock.json'))
    })
  })

  it('should resolve the diff capture pin against the explicit configuration', async () => {
    await expect(run(['diff'])).rejects.toMatchObject({
      stderr: expect.stringContaining(path.join(directory, 'captures.lock.json'))
    })
  })

  it('should refuse a configuration that names no capture pin', async () => {
    const config2 = path.join(directory, 'unpinned.config.json')
    await writeFile(
      config2,
      JSON.stringify({ pages: {}, snapshotDir: 'captures', outDir: 'output' })
    )
    await expect(
      execute(
        process.execPath,
        [
          '--import',
          import.meta.resolve('tsx'),
          path.join(import.meta.dirname, 'cli.ts'),
          'diff',
          '--config',
          config2
        ],
        { cwd }
      )
    ).rejects.toMatchObject({ stderr: expect.stringContaining('names no snapshotLock') })
  })
})
