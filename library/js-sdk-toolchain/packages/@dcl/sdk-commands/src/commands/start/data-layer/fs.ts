import { FileSystemInterface } from '@dcl/inspector-service'
import path from 'path'
import { CliComponents } from '../../../components'

/**
 * Convert paths to posix stlye
 * .i.e: scene\\assets\\main.composite -> scene/assets/main.composite
 */
export function pathToPosix(value: string): string {
  return value.replace(/\\/g, '/')
}

/**
 * The file system the editor's data layer works on: every path it names is resolved against the
 * project directory and must stay inside it, whatever the client sends.
 */
export function createFileSystemInterfaceFromFsComponent(
  { fs }: Pick<CliComponents, 'fs'>,
  projectWorkingDirectory: string = process.cwd()
): FileSystemInterface {
  const root = path.resolve(projectWorkingDirectory)

  function resolveInside(value: string): string {
    const resolved = path.resolve(root, value)
    const relative = path.relative(root, resolved)
    if (relative.startsWith('..') || path.isAbsolute(relative)) {
      throw new Error(`Path ${JSON.stringify(value)} is outside the project`)
    }
    return resolved
  }

  return {
    dirname(value: string): string {
      return pathToPosix(path.dirname(value))
    },
    basename(value: string): string {
      return pathToPosix(path.basename(value))
    },
    join(...paths: string[]): string {
      return path.join(...paths)
    },
    async existFile(filePath: string): Promise<boolean> {
      return fs.fileExists(resolveInside(filePath))
    },
    async readFile(filePath: string): Promise<Buffer> {
      return fs.readFile(resolveInside(filePath))
    },
    async writeFile(filePath: string, content: Buffer): Promise<void> {
      const resolvedPath = resolveInside(filePath)
      const folder = path.dirname(resolvedPath)
      if (!(await fs.directoryExists(folder))) {
        await fs.mkdir(folder, { recursive: true })
      }
      await fs.writeFile(resolvedPath, content as Uint8Array)
    },
    async rm(filePath: string) {
      await fs.rm(resolveInside(filePath))
    },
    async rmdir(dirPath: string) {
      await fs.rm(resolveInside(dirPath), { recursive: true })
    },
    async readdir(dirPath: string): Promise<{ name: string; isDirectory: boolean }[]> {
      const resolvedPath = resolveInside(dirPath)
      if (dirPath.indexOf('/../') !== -1) {
        throw new Error('The usage of /../ is not allowed')
      }

      const result = await fs.readdir(resolvedPath)
      return Promise.all(
        result.map(async (name) => ({
          name: pathToPosix(name),
          isDirectory: await fs.directoryExists(path.resolve(resolvedPath, name))
        }))
      )
    },
    cwd(): string {
      return pathToPosix(root)
    },
    async stat(filePath: string): Promise<{ size: number }> {
      const stats = await fs.stat(resolveInside(filePath))
      return { size: Number(stats.size) }
    }
  }
}
