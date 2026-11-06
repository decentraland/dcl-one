import path from 'node:path'
import { build } from 'vite'
import { describe, expect, it } from 'vitest'

// Vitest stubs CSS, so a broken CSS Modules `composes` or `url()` path only shows up in a real
// build. These build the library the way an app does: client (CSS bundled) and SSR.
const root = path.resolve(import.meta.dirname, '..')
const entry = path.join(root, 'test/consumer/entry.tsx')

type Output = { output: { type: string; fileName: string; source?: string | Uint8Array }[] }

async function bundle(ssr: boolean): Promise<Output['output']> {
  const result = (await build({
    configFile: false,
    root,
    logLevel: 'silent',
    build: {
      write: false,
      ssr: ssr ? entry : undefined,
      lib: ssr ? undefined : { entry, formats: ['es'], fileName: 'consumer' },
      rolldownOptions: { external: ['react', 'react-dom', 'react/jsx-runtime'] }
    }
  })) as Output | Output[]
  return (Array.isArray(result) ? result : [result]).flatMap((item) => item.output)
}

describe('a consumer build of the library', () => {
  it('should bundle the client build and its CSS, including the text styles promoted CSS composes', async () => {
    const output = await bundle(false)

    const css = output
      .filter((item) => item.fileName.endsWith('.css'))
      .map((item) => String(item.source))
      .join('\n')
    expect(css).toContain('--dcl-primary-primary')
    expect(css).toMatch(/font-family:\s*'?Inter/)
  }, 60_000)

  it('should load in an SSR build', async () => {
    const output = await bundle(true)

    expect(output.some((item) => item.type === 'chunk')).toBe(true)
  }, 60_000)
})
