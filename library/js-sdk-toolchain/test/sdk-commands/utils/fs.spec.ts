import * as fsUtils from '../../../packages/@dcl/sdk-commands/src/logic/fs'
import { readZip } from '../../../packages/@dcl/sdk-commands/src/logic/zip'
import { createFsComponent } from '../../../packages/@dcl/sdk-commands/src/components/fs'
import { createFetchComponent } from '../../../packages/@dcl/sdk-commands/src/components/fetch'
import path from 'path'
import os from 'os'
import { promises as fs } from 'fs'
import { crc32, deflateRawSync } from 'zlib'
import { normalizeDecentralandFilename } from '../../../packages/@dcl/sdk-commands/src/logic/project-files'

afterEach(() => {
  jest.clearAllMocks()
  jest.restoreAllMocks()
})

describe('utils/fs', () => {
  it("download: should download a file and return it's destination", async () => {
    const fs = createFsComponent()
    const fetch = createFetchComponent()

    const fetchSpy = jest.spyOn(fetch, 'fetch').mockResolvedValue({
      arrayBuffer: async () => new ArrayBuffer(123)
    } as any)

    const writeFileSpy = jest.spyOn(fs, 'writeFile').mockImplementation()

    const dist = await fsUtils.download({ fs, fetch }, 'some/path', 'other/path')

    expect(dist).toBe('other/path')
    expect(fetchSpy).toHaveBeenCalledWith('some/path')
    expect(writeFileSpy).toHaveBeenCalledWith('other/path', Buffer.from(new ArrayBuffer(123)))
  })

  it('fileExists directoryExists', async () => {
    const fs = createFsComponent()
    expect(await fs.fileExists('package.json')).toBeTruthy()
    expect(await fs.directoryExists('node_modules')).toBeTruthy()
    expect(await fs.directoryExists('node_modulesASD')).toBeFalsy()
    expect(await fs.fileExists('noooooooooo.json')).toBeFalsy()
  })

  describe('extract', () => {
    type Entry = {
      name: string
      data?: Buffer
      stored?: boolean
      mode?: number
      /** The size that the archive records, when it is not the size of data. */
      size?: number
      /** An extra field in the local header. */
      extra?: Buffer
      /** The local header leaves out the checksum and sizes; a data descriptor after the data gives them. */
      descriptor?: boolean
    }

    // A zip as GitHub's branch archives are laid out: local headers, the central directory, its end record.
    function zip(entries: Entry[]): Buffer {
      const locals: Buffer[] = []
      const centrals: Buffer[] = []
      let offset = 0
      for (const entry of entries) {
        const data = entry.data ?? Buffer.alloc(0)
        const body = entry.stored ? data : deflateRawSync(data)
        const size = entry.size ?? data.length
        const name = Buffer.from(entry.name)
        const extra = entry.extra ?? Buffer.alloc(0)
        const flags = entry.descriptor ? 0x8 : 0
        const local = Buffer.alloc(30)
        local.writeUInt32LE(0x04034b50, 0)
        local.writeUInt16LE(flags, 6)
        local.writeUInt16LE(entry.stored ? 0 : 8, 8)
        if (!entry.descriptor) {
          local.writeUInt32LE(crc32(data), 14)
          local.writeUInt32LE(body.length, 18)
          local.writeUInt32LE(size, 22)
        }
        local.writeUInt16LE(name.length, 26)
        local.writeUInt16LE(extra.length, 28)
        const descriptor = Buffer.alloc(entry.descriptor ? 16 : 0)
        if (entry.descriptor) {
          descriptor.writeUInt32LE(0x08074b50, 0)
          descriptor.writeUInt32LE(crc32(data), 4)
          descriptor.writeUInt32LE(body.length, 8)
          descriptor.writeUInt32LE(size, 12)
        }
        const central = Buffer.alloc(46)
        central.writeUInt32LE(0x02014b50, 0)
        central.writeUInt16LE((3 << 8) | 20, 4) // made by a Unix tool, so the mode applies
        central.writeUInt16LE(flags, 8)
        central.writeUInt16LE(entry.stored ? 0 : 8, 10)
        central.writeUInt32LE(crc32(data), 16)
        central.writeUInt32LE(body.length, 20)
        central.writeUInt32LE(size, 24)
        central.writeUInt16LE(name.length, 28)
        central.writeUInt32LE(((entry.mode ?? 0) << 16) >>> 0, 38)
        central.writeUInt32LE(offset, 42)
        locals.push(local, name, extra, body, descriptor)
        centrals.push(central, name)
        offset += local.length + name.length + extra.length + body.length + descriptor.length
      }
      const directory = Buffer.concat(centrals)
      const end = Buffer.alloc(22)
      end.writeUInt32LE(0x06054b50, 0)
      end.writeUInt16LE(entries.length, 8)
      end.writeUInt16LE(entries.length, 10)
      end.writeUInt32LE(directory.length, 12)
      end.writeUInt32LE(offset, 16)
      return Buffer.concat([...locals, directory, end])
    }

    let dir: string
    beforeEach(async () => {
      dir = await fs.mkdtemp(path.join(os.tmpdir(), 'sdk-commands-extract-'))
    })
    afterEach(() => fs.rm(dir, { recursive: true, force: true }))

    it('writes stored and deflated entries, keeps modes and names the top-level folders', async () => {
      // Stored, as the template's thumbnail is: the entry that stalled extract-zip.
      const png = Buffer.alloc(106278)
      for (let i = 0; i < png.length; i++) png[i] = (i * 2654435761) >>> 24
      const archive = path.join(dir, 'template.zip')
      await fs.writeFile(
        archive,
        zip([
          { name: 'scene-main/' },
          { name: 'scene-main/package.json', data: Buffer.from('{"name":"scene"}\n') },
          { name: 'scene-main/images/scene-thumbnail.png', data: png, stored: true },
          { name: 'scene-main/bin/run', data: Buffer.from('#!/bin/sh\n'), mode: 0o100755 },
          { name: 'scene-main/README.md', data: Buffer.from('# scene\n'), extra: Buffer.alloc(9), descriptor: true },
          { name: 'LICENSE', data: Buffer.from('MIT\n') }
        ])
      )

      const result = await fsUtils.extract(archive, path.join(dir, 'out'))

      expect(result).toStrictEqual({ destPath: path.join(dir, 'out'), topLevelFolders: ['scene-main', 'LICENSE'] })
      expect(await fs.readFile(path.join(dir, 'out/scene-main/package.json'), 'utf8')).toBe('{"name":"scene"}\n')
      expect((await fs.readFile(path.join(dir, 'out/scene-main/images/scene-thumbnail.png'))).equals(png)).toBe(true)
      expect(await fs.readFile(path.join(dir, 'out/scene-main/README.md'), 'utf8')).toBe('# scene\n')
      expect(await fs.readFile(path.join(dir, 'out/LICENSE'), 'utf8')).toBe('MIT\n')
      if (process.platform !== 'win32') {
        expect((await fs.stat(path.join(dir, 'out/scene-main/bin/run'))).mode & 0o111).toBe(0o111)
      }
    })

    it.each([
      '../outside.txt',
      '/outside.txt',
      'C:/outside.txt',
      'scene-main\\..\\outside.txt',
      'scene-main/.. /x.txt',
      'a\0b'
    ])('refuses the entry name %j before it writes any file', async (name) => {
      const archive = path.join(dir, 'evil.zip')
      await fs.writeFile(
        archive,
        zip([
          { name: 'scene-main/ok.txt', data: Buffer.from('ok') },
          { name, data: Buffer.from('x') }
        ])
      )
      await expect(fsUtils.extract(archive, path.join(dir, 'out'))).rejects.toThrow(
        /invalid zip entry name|escapes the destination/
      )
      await expect(fs.access(path.join(dir, 'out'))).rejects.toThrow()
      await expect(fs.access(path.join(dir, 'outside.txt'))).rejects.toThrow()
    })

    it('skips a symbolic link entry, so no later entry writes through it', async () => {
      const archive = path.join(dir, 'link.zip')
      await fs.writeFile(
        archive,
        zip([
          { name: 'scene-main/escape', data: Buffer.from('../..'), mode: 0o120777 },
          { name: 'scene-main/escape/outside.txt', data: Buffer.from('x') }
        ])
      )
      await fsUtils.extract(archive, path.join(dir, 'out'))
      expect((await fs.lstat(path.join(dir, 'out/scene-main/escape'))).isDirectory()).toBe(true)
      expect(await fs.readFile(path.join(dir, 'out/scene-main/escape/outside.txt'), 'utf8')).toBe('x')
      await expect(fs.access(path.join(dir, 'outside.txt'))).rejects.toThrow()
    })

    const unix = process.platform === 'win32' ? it.skip : it
    unix('refuses to write through a link that is in the destination already', async () => {
      await fs.mkdir(path.join(dir, 'elsewhere'))
      await fs.mkdir(path.join(dir, 'out'))
      await fs.symlink(path.join(dir, 'elsewhere'), path.join(dir, 'out/scene-main'))
      const archive = path.join(dir, 'template.zip')
      await fs.writeFile(archive, zip([{ name: 'scene-main/a/b.txt', data: Buffer.from('x') }]))
      await expect(fsUtils.extract(archive, path.join(dir, 'out'))).rejects.toThrow('goes through a link')
      await expect(fs.access(path.join(dir, 'elsewhere/a'))).rejects.toThrow()
    })

    it('refuses an entry that inflates to more than its recorded size, and an archive too big to hold', () => {
      const bomb = zip([{ name: 'a.txt', data: Buffer.alloc(1024 * 1024), size: 100 }])
      expect(() => readZip(bomb)).toThrow('inflates to more than 100 bytes')
      const huge = zip([{ name: 'a.txt', data: Buffer.from('x'), stored: true, size: 0x80000000 }])
      expect(() => readZip(huge)).toThrow('holds more than')
    })

    it('refuses what is not a zip, a corrupt entry and zip64', () => {
      expect(() => readZip(Buffer.from('PK not really'))).toThrow('not a zip archive')
      const corrupt = zip([{ name: 'a.txt', data: Buffer.from('hello') }])
      corrupt.writeUInt32LE(0, corrupt.lastIndexOf(Buffer.from('a.txt')) - 46 + 16) // the central directory's crc
      expect(() => readZip(corrupt)).toThrow('checksum')
      const big = zip([{ name: 'a.txt', data: Buffer.from('hello') }])
      big.writeUInt16LE(0xffff, big.length - 22 + 10)
      expect(() => readZip(big)).toThrow('zip64')
    })
  })
})

test('filename normalization works as expected', () => {
  expect(normalizeDecentralandFilename('/a/root/folder', '/a/root/folder/models/a.gltf')).toEqual('models/a.gltf')
  expect(normalizeDecentralandFilename('/a/root/folder', '/a/root/folder/models/B.GLTF')).toEqual('models/b.gltf')
  expect(normalizeDecentralandFilename('/a/root/folder', 'modeLS/C.GLTF')).toEqual('models/c.gltf')
  expect(normalizeDecentralandFilename('/a/root/folder', 'modeLS/a/../d.GLTF')).toEqual('models/d.gltf')
})
