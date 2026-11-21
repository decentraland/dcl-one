import path from 'path'
import ignore from 'ignore'
import { dclIgnored } from '../../../../packages/@dcl/sdk-commands/src/commands/start/server/file-watch-notifier'

describe('dclIgnored', () => {
  const root = path.resolve('/project')
  const ignored = dclIgnored(root, ignore().add(['node_modules/', '*.crdt', '..foo']))
  const folder = { isDirectory: () => true } as any

  it('tests the paths in the project against .dclignore', () => {
    expect(ignored(path.join(root, 'main.crdt'))).toBe(true)
    expect(ignored(path.join(root, 'node_modules'), folder)).toBe(true)
    expect(ignored(path.join(root, 'src/index.ts'))).toBe(false)
  })

  it('tests a project file whose name starts with two dots', () => {
    expect(ignored(path.join(root, '..foo'))).toBe(true)
  })

  it('does not ignore the project root or a path outside it', () => {
    expect(ignored(root, folder)).toBe(false)
    expect(ignored(path.resolve(root, '..'), folder)).toBe(false)
    expect(ignored(path.resolve(root, '../other/main.crdt'))).toBe(false)
  })
})
