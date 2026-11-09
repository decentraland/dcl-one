import path from 'node:path'
import { beforeEach, describe, expect, it } from 'vitest'
import { build, type Rolldown } from 'vite'

describe('when a browser consumer imports the public foundation entries', () => {
  let files: Rolldown.OutputChunk[]
  let assets: Rolldown.OutputAsset[]

  beforeEach(async () => {
    const result = await build({
      configFile: false,
      root: path.resolve(import.meta.dirname, '..'),
      logLevel: 'silent',
      build: {
        write: false,
        minify: false,
        lib: {
          entry: path.resolve(import.meta.dirname, 'foundations-consumer.ts'),
          formats: ['es']
        },
        rolldownOptions: { external: ['react', 'react-dom', 'react/jsx-runtime'] }
      }
    })
    if (!Array.isArray(result) && !('output' in result)) {
      throw new Error('Expected a one-shot browser build')
    }
    files = (Array.isArray(result) ? result : [result])
      .flatMap((output) => output.output)
      .filter((file): file is Rolldown.OutputChunk => file.type === 'chunk')
    assets = (Array.isArray(result) ? result : [result])
      .flatMap((output) => output.output)
      .filter((file): file is Rolldown.OutputAsset => file.type === 'asset')
  })

  it('should bundle browser tokens, typography and fonts without the Node tooling', () => {
    expect({
      nodeTooling: files
        .flatMap((file) => Object.keys(file.modules))
        .filter((module) => /figma\/|node:|@modelcontextprotocol/.test(module)),
      foundationCss: assets.some(
        (file) =>
          file.fileName.endsWith('.css') &&
          file.source.toString().includes('--dcl-bg-page') &&
          file.source.toString().includes('font-face')
      )
    }).toEqual({ nodeTooling: [], foundationCss: true })
  })
})
