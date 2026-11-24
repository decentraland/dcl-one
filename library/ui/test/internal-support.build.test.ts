import { execFileSync } from 'node:child_process'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { pathToFileURL } from 'node:url'
import { build } from 'vite'
import { describe, expect, it } from 'vitest'
import * as publicUi from '../index'

describe('when consumers import the public UI entry', () => {
  it('should keep support modules internal', () => {
    expect(publicUi).not.toHaveProperty('CardImage')
    expect(publicUi).not.toHaveProperty('hasRenderableContent')
    expect(publicUi).not.toHaveProperty('linkHref')
    expect(publicUi).not.toHaveProperty('contentLabel')
  })
})

describe('when the internal support modules are bundled for the browser and the server', () => {
  it('should keep the browser graph free of node-only modules and server-render their policies', async () => {
    const root = path.resolve(import.meta.dirname, '..')
    const directory = await mkdtemp(path.join(root, '.consumer-ssr-'))
    try {
      const repository = path.resolve(root, '..', '..')
      const namespaceChecker = pathToFileURL(
        path.join(repository, 'rendering/bevy-explorer/source/react-web/scripts/shared-inputs.mjs')
      ).href
      expect(
        execFileSync(
          process.execPath,
          [
            '--input-type=module',
            '-e',
            `import {readSharedPaths} from ${JSON.stringify(namespaceChecker)};readSharedPaths(${JSON.stringify(repository)});`
          ],
          { encoding: 'utf8' }
        )
      ).toBe('')
      const entry = path.join(directory, 'entry.tsx')
      await writeFile(
        entry,
        `import {renderToString} from 'react-dom/server';
import {CardImage} from '../components/CardImage/CardImage';
import {hasRenderableContent,contentFallback,contentLabel} from '../content.helpers';
import {linkHref,secureRel} from '../link.helpers';
export const policy={unsafe:linkHref('javascript:bad'),launch:linkHref('decentraland://realm',true),ordinary:linkHref('decentraland://realm'),rel:secureRel('preview','author'),label:contentLabel(null,'Action'),zero:contentFallback(0,'Action')};
export const missing=renderToString(<CardImage src={null} alt="Artwork"/>);
export const unsafe=renderToString(<CardImage src="javascript:alert(1)" alt=""/>);
export const valid=renderToString(<CardImage src="/poster.webp" alt="Poster"/>);
export const zero=hasRenderableContent([null,[false,0]]);`
      )
      const browserEntry = path.join(directory, 'browser.tsx')
      await writeFile(
        browserEntry,
        `import {CardImage} from '../components/CardImage/CardImage';
import {hasRenderableContent,contentFallback} from '../content.helpers';
import {linkHref} from '../link.helpers';
export function View(){return hasRenderableContent(contentFallback(0,'Action'))&&linkHref('/poster.webp')?<CardImage src="/poster.webp" alt="Poster"/>:null}`
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
import {missing,unsafe,valid,zero,policy} from ${JSON.stringify(pathToFileURL(path.join(directory, 'dist/entry.mjs')).href)};
assert.deepEqual(policy,{unsafe:undefined,launch:'decentraland://realm',ordinary:undefined,rel:'author noopener noreferrer',label:'Action',zero:0});
assert.match(missing,/role="img"/);assert.match(missing,/aria-label="Artwork"/);
assert.doesNotMatch(unsafe,/<img/);assert.ok(valid.includes('src="/poster.webp"'));assert.equal(zero,true);`
      )
      expect(execFileSync(process.execPath, [check], { encoding: 'utf8' })).toBe('')
    } finally {
      await rm(directory, { recursive: true, force: true })
    }
  }, 60_000)
})
