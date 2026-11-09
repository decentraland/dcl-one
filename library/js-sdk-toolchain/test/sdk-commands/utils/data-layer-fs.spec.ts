import path from 'path'
import {
  createFileSystemInterfaceFromFsComponent,
  pathToPosix
} from '../../../packages/@dcl/sdk-commands/src/commands/start/data-layer/fs'
import { createFsComponent } from '../../../packages/@dcl/sdk-commands/src/components/fs'

describe('pathToPosix', () => {
  test('should convert Windows-style paths to POSIX-style paths', () => {
    const input = 'scene\\assets\\main.composite'
    const expectedOutput = 'scene/assets/main.composite'
    expect(pathToPosix(input)).toEqual(expectedOutput)
  })

  test('should keep POSIX-style paths unchanged', () => {
    const input = 'scene/assets/main.composite'
    const expectedOutput = 'scene/assets/main.composite'
    expect(pathToPosix(input)).toEqual(expectedOutput)
  })
})

describe('FsInterface', () => {
  const fs = createFsComponent()
  const fsInterface = createFileSystemInterfaceFromFsComponent({ fs })

  describe('dirname', () => {
    test('should return the directory name of the given path', () => {
      const input = 'scene/assets/main.composite'
      const expectedOutput = 'scene/assets'
      expect(fsInterface.dirname(input)).toEqual(expectedOutput)
    })

    test('[Win] should return the directory name of the given path', () => {
      const input = 'scene\\assets\\main.composite'
      const expectedOutput = 'scene/assets'
      const dirname = path.dirname
      path.dirname = path.win32.dirname
      expect(fsInterface.dirname(input)).toEqual(expectedOutput)
      path.dirname = dirname
    })

    test('should return a . if the path is a file name', () => {
      const input = 'main.composite'
      const expectedOutput = '.'
      expect(fsInterface.dirname(input)).toEqual(expectedOutput)
    })
  })

  describe('basename', () => {
    test('[Win] should return the base name of the given path', () => {
      const input = 'scene\\assets\\main.composite'
      const expectedOutput = 'main.composite'
      const basename = path.basename
      path.basename = path.win32.basename
      expect(fsInterface.basename(input)).toEqual(expectedOutput)
      path.basename = basename
    })

    test('should return the base name of the given path', () => {
      const input = 'scene/assets/main.composite'
      const expectedOutput = 'main.composite'
      expect(fsInterface.basename(input)).toEqual(expectedOutput)
    })
  })

  describe('join', () => {
    test('should join multiple path segments into a single path', () => {
      const input = ['scene', 'assets', 'main.composite']
      const expectedOutput = 'scene/assets/main.composite'
      expect(fsInterface.join(...input)).toEqual(expectedOutput)
    })
  })
  describe('readdir', () => {
    test('should return an array of file names and their types (directory or file)', async () => {
      const input = 'scene/assets'
      const expectedOutput = [
        { name: 'boedo.casla', isDirectory: false },
        { name: 'casla.boedo', isDirectory: false },
        { name: 'sanlorenzo', isDirectory: true }
      ]

      fs.readdir = jest.fn().mockResolvedValue(['boedo.casla', 'casla.boedo', 'sanlorenzo'])

      // Mock the fs.directoryExists function to return the expected values
      fs.directoryExists = jest
        .fn()
        .mockResolvedValueOnce(false)
        .mockResolvedValueOnce(false)
        .mockResolvedValueOnce(true)

      const result = await fsInterface.readdir(input)
      expect(result).toEqual(expectedOutput)
    })

    test('should throw an error when the path contains "/../"', async () => {
      const input = 'scene/assets/../'
      await expect(fsInterface.readdir(input)).rejects.toThrow('The usage of /../ is not allowed')
    })
  })
})

describe('FsInterface confinement', () => {
  const fs = createFsComponent()
  const root = path.resolve('/project')
  const fsInterface = createFileSystemInterfaceFromFsComponent({ fs }, root)

  beforeEach(() => {
    fs.readFile = jest.fn().mockResolvedValue(Buffer.from('x'))
    fs.fileExists = jest.fn().mockResolvedValue(true)
    fs.writeFile = jest.fn().mockResolvedValue(undefined)
    fs.directoryExists = jest.fn().mockResolvedValue(true)
    fs.rm = jest.fn().mockResolvedValue(undefined)
    fs.stat = jest.fn().mockResolvedValue({ size: 1 })
    fs.readdir = jest.fn().mockResolvedValue([])
  })

  test('resolves relative paths against the project directory', async () => {
    await fsInterface.readFile('assets/scene/main.composite')
    expect(fs.readFile).toHaveBeenCalledWith(path.join(root, 'assets/scene/main.composite'))
    await fsInterface.writeFile('./assets/pack/../notes.txt', Buffer.from('n'))
    expect(fs.writeFile).toHaveBeenCalledWith(path.join(root, 'assets/notes.txt'), expect.anything())
  })

  test('accepts an absolute path inside the project', async () => {
    expect(await fsInterface.existFile(path.join(root, 'scene.json'))).toBe(true)
    expect(fs.fileExists).toHaveBeenCalledWith(path.join(root, 'scene.json'))
  })

  test.each(['..', '../secret.txt', 'assets/../../secret.txt', '/etc/passwd', path.resolve('/project-other/x')])(
    'rejects %s in every operation without touching the disk',
    async (input) => {
      for (const operation of [
        () => fsInterface.readFile(input),
        () => fsInterface.existFile(input),
        () => fsInterface.writeFile(input, Buffer.from('x')),
        () => fsInterface.rm(input),
        () => fsInterface.rmdir(input),
        () => fsInterface.readdir(input),
        () => fsInterface.stat(input)
      ]) {
        await expect(operation()).rejects.toThrow('outside the project')
      }
      for (const mocked of [fs.readFile, fs.fileExists, fs.writeFile, fs.rm, fs.stat, fs.readdir]) {
        expect(mocked).not.toHaveBeenCalled()
      }
    }
  )

  test('reports the project directory as the working directory', () => {
    expect(fsInterface.cwd()).toBe(pathToPosix(root))
  })
})
