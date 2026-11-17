import { mkdtemp, readdir, rm } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { runSync, type App } from '../app.js'
import { errorText } from '../server-errors.js'
import { createTools } from './index.js'

// Live Figma access is for local inspection: it returns data to the caller and never writes tracked files.
const KEY = 'FAKEKEY000example000000'
const PNG = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])

let root: string
let app: App
const tool = (name: string) => createTools(app).find((entry) => entry.name === name)!

async function filesUnder(dir: string): Promise<string[]> {
  return (await readdir(dir, { recursive: true })).map(String)
}

beforeEach(async () => {
  root = await mkdtemp(path.join(os.tmpdir(), 'figma-live-'))
  app = {
    cwd: path.join(root, 'figma'),
    config: {},
    createSource: () => ({
      name: 'rest',
      getIcons: async () => [
        { name: 'general/icon', category: 'general', file: 'a.svg', svg: '<svg/>' }
      ]
    })
  } as unknown as App
  vi.stubEnv('FIGMA_TOKEN', 'figd_test_token')
  vi.stubEnv('FIGMA_FILE_KEY', KEY)
})

afterEach(async () => {
  vi.unstubAllEnvs()
  vi.unstubAllGlobals()
  await rm(root, { recursive: true, force: true })
})

describe('when figma_screenshot renders a node', () => {
  it('should return the PNG in memory and write nothing', async () => {
    const fetchFn = vi.fn<typeof fetch>(async (input) =>
      String(input).includes('/v1/images/')
        ? Response.json({ images: { '1:2': 'https://images.figmausercontent.com/render' } })
        : new Response(new Uint8Array(PNG))
    )
    vi.stubGlobal('fetch', fetchFn)

    const result = (await tool('figma_screenshot').handler({ nodeId: '1-2', scale: 1 })) as {
      nodeId: string
      mimeType: string
      data: string
    }

    expect(result).toEqual({ nodeId: '1:2', mimeType: 'image/png', data: PNG.toString('base64') })
    expect(await filesUnder(root)).toEqual([])
  })

  it('should redact the file key from an API failure', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn<typeof fetch>(async () => new Response('{"err":"forbidden"}', { status: 403 }))
    )

    const failure = await tool('figma_screenshot')
      .handler({ nodeId: '1:2', scale: 1 })
      .catch((error: unknown) => error)

    expect(failure).toBeInstanceOf(Error)
    expect(errorText(failure)).not.toContain(KEY)
    expect((failure as Error).message).not.toContain(KEY)
  })

  it('should refuse to run without a token instead of reading any file key into a request', async () => {
    vi.stubEnv('FIGMA_TOKEN', '')
    const fetchFn = vi.fn<typeof fetch>()
    vi.stubGlobal('fetch', fetchFn)

    await expect(tool('figma_screenshot').handler({ nodeId: '1:2', scale: 1 })).rejects.toThrow(
      'FIGMA_TOKEN is not set'
    )
    expect(fetchFn).not.toHaveBeenCalled()
  })
})

describe('when a writing command is asked to read the live file', () => {
  it('should refuse sync from rest before reading anything', async () => {
    const createSource = vi.fn()
    await expect(
      runSync({ ...app, createSource } as unknown as App, { source: 'rest' })
    ).rejects.toThrow('reads only the verified fetch-source cache')
    expect(createSource).not.toHaveBeenCalled()
    expect(await filesUnder(root)).toEqual([])
  })

  it('should refuse to write live icons to disk, and return them inline instead', async () => {
    await expect(
      tool('figma_export_icons').handler({ source: 'rest', outDir: '../output' })
    ).rejects.toThrow('writes files only from the verified cache')
    expect(await filesUnder(root)).toEqual([])

    const inline = (await tool('figma_export_icons').handler({ source: 'rest' })) as {
      count: number
    }
    expect(inline.count).toBe(1)
  })
})

describe('when an error carries the configured key', () => {
  it('should redact it from the text the CLI and the MCP server show', () => {
    expect(errorText(new Error(`Request failed on /v1/files/${KEY}/nodes`))).toBe(
      'Request failed on /v1/files/<DCL_FIGMA_FILE>/nodes'
    )
  })
})
