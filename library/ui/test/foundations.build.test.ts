import { execFileSync } from 'node:child_process'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { pathToFileURL } from 'node:url'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { build, type Rolldown } from 'vite'

function componentModules(modules: readonly string[], root: string): string[] {
  const prefix = root.replaceAll('\\', '/') + '/components/'
  return modules.filter((file) => file.replaceAll('\\', '/').startsWith(prefix))
}

describe('when a browser consumer imports the public foundation entries', () => {
  let files: Rolldown.OutputChunk[]
  let assets: Rolldown.OutputAsset[]
  let modules: string[]
  let directory: string | undefined
  const root = path.resolve(import.meta.dirname, '..')

  async function bundle(entry: string, record: (ids: string[]) => void) {
    return build({
      configFile: false,
      root,
      plugins: [
        {
          name: 'foundation-module-closure',
          generateBundle() {
            record([...this.getModuleIds()])
          }
        }
      ],
      logLevel: 'silent',
      build: {
        write: false,
        minify: false,
        lib: {
          entry,
          formats: ['es']
        },
        rolldownOptions: {
          external: ['react', 'react-dom', 'react/jsx-runtime', 'react/jsx-dev-runtime']
        }
      }
    })
  }

  beforeEach(async () => {
    modules = []
    directory = undefined
    const result = await bundle(
      path.resolve(import.meta.dirname, 'foundations-consumer.ts'),
      (ids) => {
        modules = ids
      }
    )
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

  afterEach(async () => {
    if (directory) await rm(directory, { recursive: true, force: true })
  })

  it('should resolve the real public foundations export and expose icons and tokens without NavBar', async () => {
    const resolved = execFileSync(
      process.execPath,
      ['--input-type=module', '-e', "console.log(import.meta.resolve('@dcl/one-ui/foundations'))"],
      { cwd: root, encoding: 'utf8' }
    ).trim()
    expect(resolved).toBe(pathToFileURL(path.join(root, 'foundations/index.ts')).href)
    expect(files).toHaveLength(1)
    directory = await mkdtemp(path.join(root, '.consumer-foundations-'))
    const entry = path.join(directory, 'entry.mjs')
    await writeFile(entry, files[0]!.code)
    const output = execFileSync(
      process.execPath,
      [
        '--input-type=module',
        '-e',
        `
      import assert from 'node:assert/strict';
      import { ShopCart, theme, foundationExports } from ${JSON.stringify(pathToFileURL(entry).href)};
      assert.equal(typeof ShopCart, 'function');
      assert.equal(theme.palette.primary.primary, 'var(--dcl-primary-primary)');
      assert.equal(Object.hasOwn(foundationExports, 'NavBar'), false);
      console.log('Public foundations: icon/token exports without NavBar');
    `
      ],
      { cwd: root, encoding: 'utf8' }
    )
    expect(output).toContain('icon/token exports without NavBar')
  })

  it('should never compile generated components, registration runtime or NavBar styles through foundations', () => {
    expect(componentModules(modules, root)).toEqual([])
    const javascript = files.map((file) => file.code).join('\n')
    expect(javascript).not.toMatch(/registerComponent|NavBar has no captured view/)
    expect(
      assets
        .filter((file) => file.fileName.endsWith('.css'))
        .map((file) => file.source.toString())
        .join('\n')
    ).not.toMatch(/NavBar|figma-module-runtime/)
  })

  it('should detect actual public-root component modules and Windows module paths as a negative control', async () => {
    let componentGraph: string[] = []
    await bundle(path.resolve(import.meta.dirname, 'consumer/entry.tsx'), (ids) => {
      componentGraph = ids
    })
    expect(componentModules(componentGraph, root)).toContain(
      path.join(root, 'components/NavBar/NavBar.tsx').replaceAll('\\', '/')
    )
    expect(componentModules(['C:/ui/components/NavBar/NavBar.tsx'], 'C:\\ui')).toEqual([
      'C:/ui/components/NavBar/NavBar.tsx'
    ])
    expect(componentModules(['C:\\ui\\components\\NavBar\\NavBar.tsx'], 'C:\\ui')).toHaveLength(1)
    expect(componentModules(['C:/ui/components-other/source.ts'], 'C:\\ui')).toEqual([])
  })

  it('should embed seven materialized WOFF2 payloads in the browser CSS', () => {
    const css = assets
      .filter((file) => file.fileName.endsWith('.css'))
      .map((file) => file.source.toString())
      .join('\n')
    const fonts = [...css.matchAll(/data:font\/woff2;base64,([A-Za-z0-9+/=]+)/g)]
    expect(fonts).toHaveLength(7)
    for (const [, base64] of fonts) {
      const bytes = Buffer.from(base64!, 'base64')
      expect(bytes.subarray(0, 4).toString('ascii')).toBe('wOF2')
      expect(bytes.length).toBeGreaterThan(1000)
    }
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
