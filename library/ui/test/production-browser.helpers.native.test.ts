// @vitest-environment node
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { stat } from 'node:fs/promises'
const deps = vi.hoisted(() => ({
  build: vi.fn(),
  find: vi.fn(),
  open: vi.fn(),
  directories: [] as string[]
}))
vi.mock('vite', () => ({ build: deps.build }))
vi.mock('node:fs/promises', async (importOriginal) => {
  const actual = await importOriginal<typeof import('node:fs/promises')>()
  return {
    ...actual,
    mkdtemp: async (prefix: string) => {
      const directory = await actual.mkdtemp(prefix)
      deps.directories.push(directory)
      return directory
    }
  }
})
vi.mock('../figma/review/scripts/capture-browser.mjs', () => ({
  findBrowser: deps.find,
  openSession: deps.open
}))
import { productionBrowser } from './production-browser.helpers'
function fixture() {
  let start!: () => Promise<void>
  let stop!: () => Promise<void>
  const browser = productionBrowser('test/consumer/fixture.tsx', {
    beforeAll: (fn) => {
      start = fn
    },
    afterAll: (fn) => {
      stop = fn
    }
  })
  return { browser, start: () => start(), stop: () => stop() }
}
beforeEach(() => {
  vi.resetAllMocks()
  deps.directories.length = 0
  deps.build.mockResolvedValue(undefined)
})
describe('when real fixture resources outlive an interrupted startup phase', () => {
  it('should close its actual HTTP listener and delete its scratch before a late browser selection can launch', async () => {
    let select!: (value: string) => void
    deps.find.mockReturnValue(
      new Promise<string>((resolve) => {
        select = resolve
      })
    )
    const previous = process.env.NODE_ENV
    const f = fixture()
    const setup = f.start().catch((error) => error)
    await vi.waitFor(() => expect(deps.find).toHaveBeenCalledOnce())
    const origin = f.browser.origin
    expect((await fetch(origin)).status).toBe(200)
    const cleanup = f.stop()
    select('late-browser')
    await cleanup
    expect(await setup).toBeInstanceOf(Error)
    expect(deps.open).not.toHaveBeenCalled()
    await expect(fetch(origin)).rejects.toThrow()
    expect(deps.directories).toHaveLength(1)
    await expect(stat(deps.directories[0]!)).rejects.toHaveProperty('code', 'ENOENT')
    expect(process.env.NODE_ENV).toBe(previous)
  })
  it('should await an aborted startup before releasing actual listening and scratch resources', async () => {
    deps.find.mockResolvedValue('selected-browser')
    deps.open.mockImplementation(
      (_exe, options) =>
        new Promise((_resolve, reject) => {
          options.signal.addEventListener('abort', () => reject(options.signal.reason), {
            once: true
          })
        })
    )
    const f = fixture()
    const setup = f.start().catch((error) => error)
    await vi.waitFor(() => expect(deps.open).toHaveBeenCalledOnce())
    const origin = f.browser.origin
    expect((await fetch(origin)).status).toBe(200)
    await f.stop()
    expect(deps.open.mock.calls[0]![1].signal.aborted).toBe(true)
    expect(await setup).toBeInstanceOf(Error)
    await expect(fetch(origin)).rejects.toThrow()
    await expect(stat(deps.directories[0]!)).rejects.toHaveProperty('code', 'ENOENT')
  })
})
