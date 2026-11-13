import * as fsUtils from '../../../packages/@dcl/sdk-commands/src/logic/fs'
import { MAX_TOTAL_SIZE, readZip } from '../../../packages/@dcl/sdk-commands/src/logic/zip'
import { CliError } from '../../../packages/@dcl/sdk-commands/src/logic/error'
import { initLanguage, Language } from '../../../packages/@dcl/sdk-commands/src/logic/lang'
import { createFsComponent } from '../../../packages/@dcl/sdk-commands/src/components/fs'
import { createFetchComponent } from '../../../packages/@dcl/sdk-commands/src/components/fetch'
import path from 'path'
import os from 'os'
import { promises as fs } from 'fs'
import { crc32, deflateRawSync } from 'zlib'
import { normalizeDecentralandFilename } from '../../../packages/@dcl/sdk-commands/src/logic/project-files'

beforeAll(() => initLanguage(Language.EN))

afterEach(() => {
  jest.clearAllMocks()
  jest.restoreAllMocks()
})

async function rejectsWith(promise: Promise<unknown>, name: string, message: string) {
  const error = await promise.then(
    () => undefined,
    (e) => e
  )
  expect(error).toBeInstanceOf(CliError)
  expect(error.name).toBe(name)
  expect(error.message).toContain(message)
}

describe('utils/fs', () => {
  it("download: should download a file and return it's destination", async () => {
    const fs = createFsComponent()
    const fetch = createFetchComponent()

    const fetchSpy = jest.spyOn(fetch, 'fetch').mockResolvedValue({
      ok: true,
      arrayBuffer: async () => new ArrayBuffer(123)
    } as any)

    const writeFileSpy = jest.spyOn(fs, 'writeFile').mockImplementation()

    const dist = await fsUtils.download({ fs, fetch }, 'some/path', 'other/path')

    expect(dist).toBe('other/path')
    expect(fetchSpy).toHaveBeenCalledWith('some/path')
    expect(writeFileSpy).toHaveBeenCalledWith('other/path', Buffer.from(new ArrayBuffer(123)))
  })

  it('download: reports a failed response instead of saving its body', async () => {
    const fs = createFsComponent()
    const fetch = createFetchComponent()
    const cancel = jest.fn().mockResolvedValue(undefined)
    jest.spyOn(fetch, 'fetch').mockResolvedValue({
      ok: false,
      status: 404,
      body: { cancel },
      arrayBuffer: async () => new ArrayBuffer(9)
    } as any)
    const writeFileSpy = jest.spyOn(fs, 'writeFile').mockImplementation()

    await rejectsWith(
      fsUtils.download({ fs, fetch }, 'https://example.test/main.zip', 'other/path'),
      'INIT_TEMPLATE_DOWNLOAD_FAILED',
      'https://example.test/main.zip failed with HTTP status 404'
    )
    expect(writeFileSpy).not.toHaveBeenCalled()
    // The unread body is released rather than holding its connection until it is collected.
    expect(cancel).toHaveBeenCalledTimes(1)
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
      /** An extra field for both headers, as the extended timestamps GitHub's archives carry. */
      extra?: Buffer
      /** General-purpose flags: 0x8 moves the sizes to a data descriptor, 0x1 marks encryption. */
      flags?: number
      method?: number
      /** What the central directory claims the entry inflates to, when not the truth. */
      declaredSize?: number
    }

    // A zip as GitHub's branch archives are laid out: local headers, the central directory, its end record.
    function zip(entries: Entry[], comment = Buffer.alloc(0)): Buffer {
      const locals: Buffer[] = []
      const centrals: Buffer[] = []
      let offset = 0
      for (const entry of entries) {
        const data = entry.data ?? Buffer.alloc(0)
        const body = entry.stored ? data : deflateRawSync(data)
        const name = Buffer.from(entry.name)
        const extra = entry.extra ?? Buffer.alloc(0)
        const flags = entry.flags ?? 0
        const method = entry.method ?? (entry.stored ? 0 : 8)
        const local = Buffer.alloc(30)
        local.writeUInt32LE(0x04034b50, 0)
        local.writeUInt16LE(flags, 6)
        local.writeUInt16LE(method, 8)
        if (!(flags & 0x8)) {
          local.writeUInt32LE(crc32(data), 14)
          local.writeUInt32LE(body.length, 18)
          local.writeUInt32LE(data.length, 22)
        }
        local.writeUInt16LE(name.length, 26)
        local.writeUInt16LE(extra.length, 28)
        const descriptor = Buffer.alloc(flags & 0x8 ? 16 : 0)
        if (flags & 0x8) {
          descriptor.writeUInt32LE(0x08074b50, 0)
          descriptor.writeUInt32LE(crc32(data), 4)
          descriptor.writeUInt32LE(body.length, 8)
          descriptor.writeUInt32LE(data.length, 12)
        }
        const central = Buffer.alloc(46)
        central.writeUInt32LE(0x02014b50, 0)
        central.writeUInt16LE(0x031e, 4) // made by a Unix zip 3.0, so the external attributes carry a mode
        central.writeUInt16LE(flags, 8)
        central.writeUInt16LE(method, 10)
        central.writeUInt32LE(crc32(data), 16)
        central.writeUInt32LE(body.length, 20)
        central.writeUInt32LE(entry.declaredSize ?? data.length, 24)
        central.writeUInt16LE(name.length, 28)
        central.writeUInt16LE(extra.length, 30)
        central.writeUInt32LE(((entry.mode ?? 0) << 16) >>> 0, 38)
        central.writeUInt32LE(offset, 42)
        locals.push(local, name, extra, body, descriptor)
        centrals.push(central, name, extra)
        offset += local.length + name.length + extra.length + body.length + descriptor.length
      }
      const directory = Buffer.concat(centrals)
      const end = Buffer.alloc(22)
      end.writeUInt32LE(0x06054b50, 0)
      end.writeUInt16LE(entries.length, 8)
      end.writeUInt16LE(entries.length, 10)
      end.writeUInt32LE(directory.length, 12)
      end.writeUInt32LE(offset, 16)
      end.writeUInt16LE(comment.length, 20)
      return Buffer.concat([...locals, directory, end, comment])
    }

    // The 0x5455 extended-timestamp field GitHub's archives attach to every entry.
    const timestamp = Buffer.from([0x55, 0x54, 0x05, 0x00, 0x01, 0x00, 0x5e, 0xd0, 0x6a])

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
        zip(
          [
            { name: 'scene-main/', extra: timestamp },
            { name: 'scene-main/package.json', data: Buffer.from('{"name":"scene"}\n'), extra: timestamp, flags: 0x8 },
            { name: 'scene-main/images/scene-thumbnail.png', data: png, stored: true, extra: timestamp },
            { name: 'scene-main/bin/run', data: Buffer.from('#!/bin/sh\n'), mode: 0o755 },
            { name: 'scene-main/empty.txt' },
            { name: 'LICENSE', data: Buffer.from('MIT\n') }
          ],
          Buffer.from('e3b0c44298fc1c149afbf4c8996fb92427ae41e4') // GitHub closes its archives with the commit
        )
      )

      const result = await fsUtils.extract(archive, path.join(dir, 'out'))

      expect(result).toStrictEqual({ destPath: path.join(dir, 'out'), topLevelFolders: ['scene-main', 'LICENSE'] })
      expect(await fs.readFile(path.join(dir, 'out/scene-main/package.json'), 'utf8')).toBe('{"name":"scene"}\n')
      expect((await fs.readFile(path.join(dir, 'out/scene-main/images/scene-thumbnail.png'))).equals(png)).toBe(true)
      expect(await fs.readFile(path.join(dir, 'out/scene-main/empty.txt'), 'utf8')).toBe('')
      expect(await fs.readFile(path.join(dir, 'out/LICENSE'), 'utf8')).toBe('MIT\n')
      if (process.platform !== 'win32') {
        // The owner's execute bit survives any umask.
        expect((await fs.stat(path.join(dir, 'out/scene-main/bin/run'))).mode & 0o100).toBe(0o100)
      }
    })

    it('takes a directory entry for the destination itself without counting it as a folder', async () => {
      const archive = path.join(dir, 'dotted.zip')
      await fs.writeFile(
        archive,
        zip([{ name: './' }, { name: './scene-main/' }, { name: './scene-main/a.txt', data: Buffer.from('a') }])
      )
      const result = await fsUtils.extract(archive, path.join(dir, 'out'))
      expect(result.topLevelFolders).toStrictEqual(['scene-main'])
      expect(await fs.readFile(path.join(dir, 'out/scene-main/a.txt'), 'utf8')).toBe('a')
    })

    it.each(['../outside.txt', '/outside.txt', 'scene-main/../../outside.txt', 'scene-main/..', '.', ''])(
      'refuses the entry %j before writing anything',
      async (name) => {
        const archive = path.join(dir, 'evil.zip')
        await fs.writeFile(
          archive,
          zip([
            { name: 'scene-main/ok.txt', data: Buffer.from('fine') },
            { name, data: Buffer.from('x') }
          ])
        )
        await rejectsWith(
          fsUtils.extract(archive, path.join(dir, 'out')),
          'INIT_INVALID_TEMPLATE_ZIP',
          `outside the destination: ${name}`
        )
        await expect(fs.access(path.join(dir, 'out'))).rejects.toThrow()
        await expect(fs.access(path.join(dir, 'outside.txt'))).rejects.toThrow()
      }
    )

    it('refuses a symbolic link entry before writing anything', async () => {
      const archive = path.join(dir, 'link.zip')
      await fs.writeFile(
        archive,
        zip([
          { name: 'scene-main/ok.txt', data: Buffer.from('fine') },
          { name: 'scene-main/link', data: Buffer.from('../../etc/passwd'), mode: 0o120777 }
        ])
      )
      await rejectsWith(
        fsUtils.extract(archive, path.join(dir, 'out')),
        'INIT_INVALID_TEMPLATE_ZIP',
        'symbolic link zip entry: scene-main/link'
      )
      await expect(fs.access(path.join(dir, 'out'))).rejects.toThrow()
    })

    it('reports an archive it cannot read as a CLI error', async () => {
      const archive = path.join(dir, 'page.zip')
      await fs.writeFile(archive, '<!doctype html><title>404</title>')
      await rejectsWith(
        fsUtils.extract(archive, path.join(dir, 'out')),
        'INIT_INVALID_TEMPLATE_ZIP',
        `Cannot extract ${archive}: not a zip archive`
      )
      await expect(fs.access(path.join(dir, 'out'))).rejects.toThrow()

      const corrupt = zip([{ name: 'a.txt', data: Buffer.from('hello') }])
      corrupt.writeUInt32LE(0, corrupt.lastIndexOf(Buffer.from('a.txt')) - 46 + 16) // the central directory's crc
      await fs.writeFile(archive, corrupt)
      await rejectsWith(
        fsUtils.extract(archive, path.join(dir, 'out')),
        'INIT_INVALID_TEMPLATE_ZIP',
        'a.txt fails its checksum'
      )
    })

    it('leaves the destination untouched when a later entry fails its checksum', async () => {
      const archive = path.join(dir, 'late.zip')
      const corrupt = zip([
        { name: 'scene-main/ok.txt', data: Buffer.from('fine') },
        { name: 'scene-main/last.txt', data: Buffer.from('hello') }
      ])
      corrupt.writeUInt32LE(0, corrupt.lastIndexOf(Buffer.from('scene-main/last.txt')) - 46 + 16)
      await fs.writeFile(archive, corrupt)
      await rejectsWith(
        fsUtils.extract(archive, path.join(dir, 'out')),
        'INIT_INVALID_TEMPLATE_ZIP',
        'scene-main/last.txt fails its checksum'
      )
      await expect(fs.access(path.join(dir, 'out'))).rejects.toThrow()
    })

    it('refuses two entries for one path before writing anything', async () => {
      const archive = path.join(dir, 'twice.zip')
      await fs.writeFile(
        archive,
        zip([
          { name: 'scene-main/a.txt', data: Buffer.from('a') },
          { name: './scene-main/a.txt', data: Buffer.from('b') }
        ])
      )
      await rejectsWith(
        fsUtils.extract(archive, path.join(dir, 'out')),
        'INIT_INVALID_TEMPLATE_ZIP',
        './scene-main/a.txt is listed twice'
      )
      await expect(fs.access(path.join(dir, 'out'))).rejects.toThrow()
    })

    const itWithSymlinks = process.platform === 'win32' ? it.skip : it
    itWithSymlinks('refuses an entry that a symbolic link already in the destination would redirect', async () => {
      const out = path.join(dir, 'out')
      const outside = path.join(dir, 'outside')
      await fs.mkdir(path.join(out, 'scene-main'), { recursive: true })
      await fs.mkdir(outside)
      await fs.symlink(outside, path.join(out, 'assets'))
      await fs.writeFile(path.join(dir, 'victim.txt'), 'keep')
      await fs.symlink(path.join(dir, 'victim.txt'), path.join(out, 'scene-main/link.txt'))
      const archive = path.join(dir, 'link.zip')

      // Through a linked directory...
      await fs.writeFile(
        archive,
        zip([
          { name: 'scene-main/ok.txt', data: Buffer.from('fine') },
          { name: 'assets/.bashrc', data: Buffer.from('x') }
        ])
      )
      await rejectsWith(
        fsUtils.extract(archive, out),
        'INIT_INVALID_TEMPLATE_ZIP',
        'outside the destination: assets/.bashrc'
      )
      // ...or onto a linked file.
      await fs.writeFile(
        archive,
        zip([
          { name: 'scene-main/ok.txt', data: Buffer.from('fine') },
          { name: 'scene-main/link.txt', data: Buffer.from('x') }
        ])
      )
      await rejectsWith(
        fsUtils.extract(archive, out),
        'INIT_INVALID_TEMPLATE_ZIP',
        'outside the destination: scene-main/link.txt'
      )

      expect(await fs.readdir(outside)).toStrictEqual([])
      expect(await fs.readFile(path.join(dir, 'victim.txt'), 'utf8')).toBe('keep')
      await expect(fs.access(path.join(out, 'scene-main/ok.txt'))).rejects.toThrow()
    })

    itWithSymlinks('extracts into a destination that is itself a symbolic link', async () => {
      await fs.mkdir(path.join(dir, 'real'))
      await fs.symlink(path.join(dir, 'real'), path.join(dir, 'out'))
      const archive = path.join(dir, 'linked-dest.zip')
      await fs.writeFile(archive, zip([{ name: 'scene-main/a.txt', data: Buffer.from('a') }]))
      await fsUtils.extract(archive, path.join(dir, 'out'))
      expect(await fs.readFile(path.join(dir, 'real/scene-main/a.txt'), 'utf8')).toBe('a')
    })

    describe('readZip', () => {
      const hello = Buffer.from('hello')
      const central = (archive: Buffer, name: string, field: number) =>
        archive.lastIndexOf(Buffer.from(name)) - 46 + field

      it('refuses what is not a zip and zip64 archives', () => {
        expect(() => readZip(Buffer.from('PK not really'))).toThrow('not a zip archive')
        const count = zip([{ name: 'a.txt', data: hello }])
        count.writeUInt16LE(0xffff, count.length - 22 + 10)
        expect(() => readZip(count)).toThrow('zip64')
        const size = zip([{ name: 'a.txt', data: hello }])
        size.writeUInt32LE(0xffffffff, central(size, 'a.txt', 24))
        expect(() => readZip(size)).toThrow('zip64')
      })

      it('refuses a central directory that does not add up', () => {
        const missing = zip([{ name: 'a.txt', data: hello }])
        missing.writeUInt16LE(2, missing.length - 22 + 10)
        expect(() => readZip(missing)).toThrow('central directory entry 1 is missing')
        const truncated = zip([{ name: 'a.txt', data: hello }])
        truncated.writeUInt16LE(0xffff, central(truncated, 'a.txt', 28)) // a name longer than the archive
        expect(() => readZip(truncated)).toThrow('central directory entry 0 is truncated')
        const local = zip([{ name: 'a.txt', data: hello }])
        local.writeUInt32LE(0, 0) // the local header's signature
        expect(() => readZip(local)).toThrow('local header of a.txt is missing')
        const short = zip([{ name: 'a.txt', data: hello }])
        short.writeUInt32LE(1000, central(short, 'a.txt', 20)) // more compressed bytes than the archive holds
        expect(() => readZip(short)).toThrow('a.txt is truncated')
      })

      it('refuses encryption, symbolic links and compression methods other than deflate', () => {
        expect(() => readZip(zip([{ name: 'a.txt', data: hello, flags: 0x1 }]))).toThrow('encrypted zip entry: a.txt')
        expect(() => readZip(zip([{ name: 'a.txt', data: Buffer.from('target'), mode: 0o120777 }]))).toThrow(
          'symbolic link zip entry: a.txt'
        )
        expect(() => readZip(zip([{ name: 'a.txt', data: hello, method: 12 }]))).toThrow(
          'unsupported zip compression method 12: a.txt'
        )
      })

      it('refuses records that alias one entry, repeated names and a total past the ceiling', () => {
        const alias = zip([
          { name: 'a.txt', data: hello },
          { name: 'b.txt', data: hello }
        ])
        alias.writeUInt32LE(0, central(alias, 'b.txt', 42)) // b.txt's record points at a.txt's local header
        expect(() => readZip(alias)).toThrow('b.txt overlaps a.txt')
        expect(() =>
          readZip(
            zip([
              { name: 'a.txt', data: hello },
              { name: 'a.txt', data: hello }
            ])
          )
        ).toThrow('a.txt is listed twice')
        const huge = zip([
          { name: 'a.txt', data: hello, declaredSize: MAX_TOTAL_SIZE },
          { name: 'b.txt', data: hello, declaredSize: 1 }
        ])
        expect(() => readZip(huge)).toThrow(`inflates past ${MAX_TOTAL_SIZE} bytes in total`)
        // Up to the ceiling the directory passes; each entry is then held to its own declared size.
        expect(readZip(zip([{ name: 'a.txt', data: hello, declaredSize: MAX_TOTAL_SIZE }]))).toHaveLength(1)
      })

      it('reads an entry only against its declared size and checksum', () => {
        const [entry] = readZip(zip([{ name: 'a.txt', data: hello }]))
        expect(entry).toMatchObject({ name: 'a.txt', size: 5, mode: 0 })
        expect(entry.read().toString()).toBe('hello')
        const [longer] = readZip(zip([{ name: 'a.txt', data: hello, declaredSize: 6 }]))
        expect(() => longer.read()).toThrow('a.txt inflates to 5 bytes, not 6')
        // A deflate bomb: a megabyte of zeros that claims to be 16 bytes stops inflating at 16.
        const [bomb] = readZip(zip([{ name: 'a.txt', data: Buffer.alloc(1 << 20), declaredSize: 16 }]))
        expect(() => bomb.read()).toThrow('a.txt inflates past its 16 bytes')
        const garbage = zip([{ name: 'a.txt', data: hello }])
        garbage.fill(0xff, 30 + 5, 30 + 5 + 3) // the first bytes of the deflate stream
        expect(() => readZip(garbage)[0].read()).toThrow('a.txt does not inflate')
      })
    })
  })
})

test('filename normalization works as expected', () => {
  expect(normalizeDecentralandFilename('/a/root/folder', '/a/root/folder/models/a.gltf')).toEqual('models/a.gltf')
  expect(normalizeDecentralandFilename('/a/root/folder', '/a/root/folder/models/B.GLTF')).toEqual('models/b.gltf')
  expect(normalizeDecentralandFilename('/a/root/folder', 'modeLS/C.GLTF')).toEqual('models/c.gltf')
  expect(normalizeDecentralandFilename('/a/root/folder', 'modeLS/a/../d.GLTF')).toEqual('models/d.gltf')
})
