import { execFileSync } from 'node:child_process'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'
import { pathToFileURL } from 'node:url'
import { build } from 'vite'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import * as publicUi from '../index'

describe('when a consumer bundles public foundations and the NavBar component', () => {
  let root: string
  let directory: string | undefined
  beforeEach(() => {
    root = path.resolve(import.meta.dirname, '..')
    directory = undefined
  })
  afterEach(async () => {
    if (directory) await rm(directory, { recursive: true, force: true })
  })
  it('should expose NavBar alongside the preserved foundations', () => {
    expect(publicUi).toHaveProperty('theme')
    expect(publicUi.NavBar).toBeTypeOf('function')
  })
  it('should compile the public props contract and reject malformed props and deep imports', () => {
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
  it('should bundle the public component with its composed foundation CSS', async () => {
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
  it('should server-render the signed-out, signed-in and empty-sections contracts through the SSR bundle', async () => {
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
      const sections = [{ label: 'Shop', href: '/shop' }];
      const signedOut = renderNavigationRoute({ sections, signIn: { href: '/sign-in' } }, onError);
      assert.equal(signedOut.status, 200);
      assert.ok(signedOut.html.includes('href="/sign-in"') && signedOut.html.includes('>Sign in</a>'));
      const signedIn = renderNavigationRoute({ sections, account: { address: '0x0000000000000000000000000000000000000000' } }, onError);
      assert.equal(signedIn.status, 200);
      assert.match(signedIn.html, /aria-label="Account: 0x0000...0000"/);
      assert.doesNotMatch(signedIn.html, />Sign in</);
      const empty = renderNavigationRoute({ sections: [], signIn: { onSelect() {} } }, onError);
      assert.equal(empty.status, 200);
      assert.match(empty.html, /<nav[^>]*aria-label="Main"/);
      assert.equal(errors.length, 0);
      console.log('SSR route: signedOut=200 signedIn=200 empty=200');
    `
    )
    expect(
      execFileSync(process.execPath, [path.join(directory, 'check.mjs')], {
        encoding: 'utf8'
      })
    ).toContain('signedOut=200 signedIn=200 empty=200')
  }, 60_000)
})
