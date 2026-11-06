import { mkdtemp, mkdir, readFile, rm, symlink, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import type { App } from '../app.js'
import { createTools } from './index.js'

describe('when exporting snapshot icons to files', () => {
  let root: string
  let file: string
  let app: App
  const svg = '<svg xmlns="http://www.w3.org/2000/svg"/>'
  beforeEach(async () => {
    root = await mkdtemp(path.join(os.tmpdir(), 'icon-export-'))
    file = 'safe.svg'
    app = {
      cwd: path.join(root, 'figma'),
      config: {},
      createSource: () => ({
        getIcons: async () => [{ name: 'general/icon', category: 'general', file, svg }]
      })
    } as unknown as App
  })
  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })
  const exportIcons = (app: App) =>
    createTools(app)
      .find((tool) => tool.name === 'figma_export_icons')!
      .handler({ source: 'snapshot', outDir: '../output' })

  it('should atomically write an ordinary SVG basename', async () => {
    await exportIcons(app)
    expect(await readFile(path.join(root, 'output/safe.svg'), 'utf8')).toBe(svg)
  })
  it.each(['../escape.svg', '..\\escape.svg', 'C:escape.svg', 'wrong.txt'])(
    'should reject %j',
    async (name) => {
      file = name
      await expect(exportIcons(app)).rejects.toThrow('Invalid icon filename')
    }
  )
  it('should preserve the target of an existing symlink', async () => {
    await mkdir(path.join(root, 'output'))
    await writeFile(path.join(root, 'precious'), 'keep')
    await symlink(path.join(root, 'precious'), path.join(root, 'output/safe.svg'))
    await expect(exportIcons(app)).rejects.toThrow('symlink')
    expect(await readFile(path.join(root, 'precious'), 'utf8')).toBe('keep')
  })
})
