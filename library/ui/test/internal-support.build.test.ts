import { execFileSync } from 'node:child_process'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { pathToFileURL } from 'node:url'
import { build } from 'vite'
import { expect, it } from 'vitest'
import * as publicUi from '../index'

it('should keep support modules internal and server-render their browser-only dependency graph', async () => {
  expect(publicUi).not.toHaveProperty('CardImage')
  expect(publicUi).not.toHaveProperty('hasRenderableContent')
  const root = path.resolve(import.meta.dirname, '..')
  const directory = await mkdtemp(path.join(root, '.internal-support-'))
  try {
    const entry = path.join(directory, 'entry.tsx')
    await writeFile(
      entry,
      `import {renderToString} from 'react-dom/server';
import {CardImage} from '../components/CardImage/CardImage';
import {hasRenderableContent} from '../content.helpers';
export const missing=renderToString(<CardImage src={null} alt="Artwork"/>);
export const unsafe=renderToString(<CardImage src="javascript:alert(1)" alt=""/>);
export const valid=renderToString(<CardImage src="/poster.webp" alt="Poster"/>);
export const zero=hasRenderableContent([null,[false,0]]);`
    )
    const browserEntry = path.join(directory, 'browser.tsx')
    await writeFile(
      browserEntry,
      `import {CardImage} from '../components/CardImage/CardImage';
import {hasRenderableContent} from '../content.helpers';
export function View(){return hasRenderableContent(0)?<CardImage src="/poster.webp" alt="Poster"/>:null}`
    )
    const modules: string[] = []
    await build({
      configFile: false,
      root,
      logLevel: 'silent',
      plugins: [
        {
          name: 'internal-runtime-boundary',
          generateBundle() {
            modules.push(...this.getModuleIds())
          }
        }
      ],
      build: {
        write: false,
        lib: { entry: browserEntry, formats: ['es'] },
        rolldownOptions: { external: ['react', 'react/jsx-runtime'] }
      }
    })
    expect(modules.filter((id) => /\/figma\/|^node:|@modelcontextprotocol/.test(id))).toEqual([])
    await build({
      configFile: false,
      root,
      logLevel: 'silent',
      build: {
        outDir: path.join(directory, 'dist'),
        ssr: entry,
        rolldownOptions: {
          external: ['react', 'react-dom/server', 'react/jsx-runtime'],
          output: { entryFileNames: 'entry.mjs' }
        }
      }
    })
    const check = path.join(directory, 'check.mjs')
    await writeFile(
      check,
      `import assert from 'node:assert/strict';
import {missing,unsafe,valid,zero} from ${JSON.stringify(pathToFileURL(path.join(directory, 'dist/entry.mjs')).href)};
assert.match(missing,/role="img"/);assert.match(missing,/aria-label="Artwork"/);
assert.doesNotMatch(unsafe,/<img/);assert.ok(valid.includes('src="/poster.webp"'));assert.equal(zero,true);`
    )
    expect(execFileSync(process.execPath, [check], { encoding: 'utf8' })).toBe('')
  } finally {
    await rm(directory, { recursive: true, force: true })
  }
}, 60_000)
