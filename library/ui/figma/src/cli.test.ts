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

  // Live Figma access and capture ingest belong to figma-design-system: this tree reads only the fetch-source cache.
  it.each(['pull-live', 'probe', 'capture-ingest', 'capture-merge'])(
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

  it('should refuse a live --source', async () => {
    await expect(run(['sync', '--source', 'rest'])).rejects.toMatchObject({
      stderr: expect.stringContaining('only the fetch-source cache')
    })
  })

  it('should resolve sync source paths against the explicit configuration', async () => {
    await expect(run(['sync', '--developer'])).rejects.toMatchObject({
      stderr: expect.stringContaining(path.join(directory, 'captures'))
    })
  })

  it('should resolve diff source paths against the explicit configuration', async () => {
    await expect(run(['diff'])).rejects.toMatchObject({
      stderr: expect.stringContaining(path.join(directory, 'captures'))
    })
  })
})
