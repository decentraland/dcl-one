import { execFileSync } from 'node:child_process'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'
import { pathToFileURL } from 'node:url'
import { build } from 'vite'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import * as publicUi from '../index'

describe('when a consumer bundles public foundations and the internal NavBar preview', () => {
  let root: string
  let directory: string | undefined
  beforeEach(() => {
    root = path.resolve(import.meta.dirname, '..')
    directory = undefined
  })
  afterEach(async () => {
    if (directory) await rm(directory, { recursive: true, force: true })
  })
  it('should expose foundations without exposing the internal NavBar', () => {
    expect(publicUi).toHaveProperty('theme')
    expect(publicUi).not.toHaveProperty('NavBar')
  })
  it('should compile internal tuple contracts and reject public NavBar imports', () => {
    expect(
      execFileSync(
        process.execPath,
        [
          path.join(root, 'node_modules/typescript/bin/tsc'),
          '-p',
          path.join(root, 'test/consumer/tsconfig.json')
        ],
        { encoding: 'utf8' }
      )
    ).toBe('')
  }, 60_000)
  it('should bundle the internal preview with its composed foundation CSS', async () => {
    const result = await build({
      configFile: false,
      root,
      logLevel: 'silent',
      build: {
        write: false,
        lib: {
          entry: path.join(root, 'test/consumer/entry.tsx'),
          formats: ['es']
        },
        rolldownOptions: { external: ['react', 'react/jsx-runtime'] }
      }
    })
    const outputs = (Array.isArray(result) ? result : [result]).flatMap((item) =>
      'output' in item ? item.output : []
    )
    expect(
      outputs
        .filter((item) => item.type === 'asset' && item.fileName.endsWith('.css'))
        .map((item) => (item.type === 'asset' ? String(item.source) : ''))
        .join('\n')
    ).toMatch(/font-family:\s*'?Inter/)
  }, 60_000)
  it('should execute the SSR bundle and pass selection errors to server route handling', async () => {
    directory = await mkdtemp(path.join(root, '.consumer-ssr-'))
    await build({
      configFile: false,
      root,
      logLevel: 'silent',
      build: {
        outDir: directory,
        ssr: path.join(root, 'test/consumer/ssr-entry.tsx'),
        rolldownOptions: {
          external: ['react', 'react-dom/server', 'react/jsx-runtime'],
          output: { entryFileNames: 'entry.mjs' }
        }
      }
    })
    await writeFile(
      path.join(directory, 'check.mjs'),
      `
      import assert from 'node:assert/strict';
      import { renderNavigationRoute } from ${JSON.stringify(pathToFileURL(path.join(directory, 'entry.mjs')).href)};
      const errors = [];
      const onError = error => { errors.push(error); return '<main role="alert">Navigation unavailable</main>'; };
      const valid = renderNavigationRoute({}, onError);
      assert.equal(valid.status, 200);
      assert.match(valid.html, /Sign In/);
      assert.equal(errors.length, 0);
      const invalid = renderNavigationRoute({viewport:'mobile',signedIn:true,balances:'credits',theme:'light'}, onError);
      assert.deepEqual(invalid, {status:500,html:'<main role="alert">Navigation unavailable</main>'});
      assert.equal(errors.length, 1);
      assert.match(errors[0].message, /NavBar has no captured view/);
      const malformed = renderNavigationRoute({signedIn:'true'}, onError);
      assert.equal(malformed.status, 500);
      assert.equal(errors.length, 2);
      console.log('SSR route: valid=200 uncaptured=500 malformed=500');
    `
    )
    expect(
      execFileSync(process.execPath, [path.join(directory, 'check.mjs')], {
        encoding: 'utf8'
      })
    ).toContain('valid=200 uncaptured=500 malformed=500')
  }, 60_000)
})
