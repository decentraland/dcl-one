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

  it('should ingest into the configured snapshot directory rather than the working directory', async () => {
    await run(['capture-ingest', chunk])
    expect(
      JSON.parse(await readFile(path.join(directory, 'captures/components/Ready.json'), 'utf8'))
    ).toMatchObject({
      page: { id: 'ready-page', name: 'Ready' },
      components: []
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
