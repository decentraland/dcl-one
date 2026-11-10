import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { afterEach, beforeEach, describe, it } from 'node:test'
import { generateSharedInputs, readSharedPaths, WEB, MANIFEST } from './shared-inputs.mjs'
const here = path.dirname(fileURLToPath(import.meta.url)),
  repository = path.resolve(here, '../../../../..')
let root, web
const put = (file, bytes) => {
  const target = path.join(root, file)
  mkdirSync(path.dirname(target), { recursive: true })
  writeFileSync(target, bytes)
}
const git = (...args) => execFileSync('git', ['-C', root, ...args], { stdio: 'pipe' })
const save = async () => {
  const bytes = await generateSharedInputs({ root })
  put(MANIFEST, bytes)
  git('add', MANIFEST)
  return JSON.parse(bytes)
}
const hashes = () =>
  JSON.parse(
    execFileSync(
      'python3',
      [
        '-c',
        `import importlib.util,json,sys;s=importlib.util.spec_from_file_location('hud',sys.argv[1]);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);print(json.dumps(m.input_hashes(sys.argv[2]),sort_keys=True))`,
        path.join(repository, 'tooling/hud_shared_inputs.py'),
        root
      ],
      { encoding: 'utf8' }
    )
  )
beforeEach(() => {
  root = mkdtempSync(path.join(tmpdir(), 'hud-shared-graph-'))
  web = path.join(root, WEB)
  put('.gitignore', 'node_modules/\n')
  put(WEB + '/package.json', '{"type":"module"}')
  put(WEB + '/package-lock.json', '{"lockfileVersion":3}')
  put(WEB + '/scripts/shared-inputs.mjs', readFileSync(path.join(here, 'shared-inputs.mjs')))
  put(
    WEB + '/tsconfig.json',
    JSON.stringify({
      compilerOptions: {
        target: 'ES2022',
        module: 'ESNext',
        moduleResolution: 'Bundler',
        strict: true,
        types: [],
        noEmit: true,
        paths: {
          '@dcl/one-ui/foundations': ['../../../../library/ui/foundations/index.ts'],
          '@shop/*': ['../../../../library/shop/*'],
          '@catalyst-api/*': ['../../../../library/catalyst-api/*']
        }
      },
      include: ['src'],
      exclude: ['src/excluded.test.ts']
    })
  )
  put(WEB + '/vite.shared.ts', 'export {}')
  put(WEB + '/vitest.config.ts', "export {default} from './vite.config.ts'")
  put(
    WEB + '/vite.config.ts',
    `import {defineConfig} from 'vite';import path from 'node:path';import {fileURLToPath}from'node:url';const here=path.dirname(fileURLToPath(import.meta.url));export default defineConfig(({mode})=>({resolve:{alias:{'@dcl/one-ui/foundations':path.resolve(here,'../../../../library/ui/foundations/index.ts'),'@shop':path.resolve(here,'../../../../library/shop'),'@catalyst-api':path.resolve(here,'../../../../library/catalyst-api')}},build:{rollupOptions:{input:{app:path.join(here,'app.html'),...(mode==='native'?{}:{index:path.join(here,'index.html')})}}}}));`
  )
  put(WEB + '/app.html', '<script type="module" src="/src/main.ts"></script>')
  put(WEB + '/index.html', '<script type="module" src="/src/main.ts"></script>')
  put(WEB + '/src/vite-env.d.ts', '/// <reference types="vite/client" />\n')
  put(
    WEB + '/src/main.ts',
    "import {token} from '@dcl/one-ui/foundations'; export {token};export const later=()=>import('@shop/data');"
  )
  put(
    WEB + '/src/type.test.ts',
    "import type {Datum} from '@catalyst-api/model';export const value:Datum={id:1};"
  )
  put(WEB + '/src/excluded.test.ts', "import {testValue}from'@shop/test-only';export{testValue};")
  put('library/shop/test-only.ts', "import './test-only.css';export const testValue=1;")
  put('library/shop/test-only.css', ".onlyTest{background:url('./onlytest.svg')}")
  put('library/shop/onlytest.svg', '<svg xmlns="http://www.w3.org/2000/svg" width="3" height="3"/>')
  put(
    'library/ui/package.json',
    '{"name":"@dcl/one-ui","type":"module","exports":{".":"./index.ts","./foundations":"./foundations/index.ts"}}'
  )
  put(
    'library/ui/foundations/typography/font-source.json',
    '{"package":"@fontsource-variable/inter","version":"5.3.0","files":{}}'
  )
  put('library/ui/index.ts', "export const NavBar='registration';")
  put('library/ui/foundations/index.ts', "import './style.css';export {token} from './token';")
  put('library/ui/foundations/token.ts', 'export const token=1;')
  put('library/ui/foundations/style.css', ".token{background:url('./pixel.svg')}")
  put(
    'library/ui/foundations/pixel.svg',
    '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>'
  )
  put('library/shop/data.ts', 'export const data=1;')
  put('library/catalyst-api/model.ts', "import './model.css';export type Datum={id:number};")
  put(
    'library/catalyst-api/model.css',
    "@import './nested.css';.model{background:url('./type-pixel.svg')}"
  )
  put('library/catalyst-api/nested.css', "@import './empty.css';.nested{color:red}")
  put('library/catalyst-api/empty.css', '')
  put(
    'library/catalyst-api/type-pixel.svg',
    '<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"/>'
  )
  symlinkSync(path.resolve(here, '../node_modules'), path.join(web, 'node_modules'), 'dir')
  git('init', '--quiet')
  git('add', '.')
})
afterEach(() => rmSync(root, { recursive: true, force: true }))
describe('the actual compiler and Vite shared-source contract', () => {
  it('records type-only, literal dynamic and CSS asset dependencies without loading the public component root', async () => {
    const manifest = await save(),
      files = manifest.files.map((file) => file.path)
    for (const file of [
      'library/catalyst-api/model.ts',
      'library/shop/data.ts',
      'library/ui/foundations/pixel.svg',
      'library/ui/foundations/style.css',
      'library/catalyst-api/model.css',
      'library/catalyst-api/type-pixel.svg',
      'library/catalyst-api/nested.css',
      'library/catalyst-api/empty.css'
    ])
      assert(files.includes(file), file)
    for (const file of [
      'library/shop/test-only.ts',
      'library/shop/test-only.css',
      'library/shop/onlytest.svg'
    ])
      assert(files.includes(file), file)
    assert(
      manifest.edges.some(
        (edge) => edge.kind === 'test:runtime' && edge.to === 'library/shop/test-only.ts'
      )
    )
    assert(!files.includes('library/ui/index.ts'))
    assert(
      manifest.edges.some(
        (edge) => edge.kind === 'type' && edge.to === 'library/catalyst-api/model.ts'
      )
    )
    assert(
      manifest.edges.some(
        (edge) => edge.kind.endsWith(':dynamic') && edge.to === 'library/shop/data.ts'
      )
    )
    assert(readSharedPaths(root).length > 0)
    assert(Object.keys(hashes()).length > 0)
  })
  it('changes the real offline fingerprint for a new tracked import and drops removed edges', async () => {
    await save()
    const before = hashes()
    put('library/shop/extra.ts', 'export const extra=2;')
    git('add', 'library/shop/extra.ts')
    put(WEB + '/src/main.ts', "import {extra} from '@shop/extra';export {extra};")
    assert.throws(() => readSharedPaths(root), /Stale/)
    git('add', WEB + '/src/main.ts')
    const added = await save(),
      after = hashes()
    assert(added.files.some((file) => file.path === 'library/shop/extra.ts'))
    assert.notDeepEqual(after, before)
    put(WEB + '/src/main.ts', 'export const own=1;')
    git('add', WEB + '/src/main.ts')
    const removed = await save()
    assert(!removed.files.some((file) => file.path === 'library/shop/extra.ts'))
    assert.notDeepEqual(hashes(), after)
  })
  it('ignores unrelated shared files and the unimported public root', async () => {
    const first = await save(),
      before = hashes()
    put('library/ui/index.ts', 'export const NavBar="changed";')
    put('library/shop/unused.ts', 'export const unused=1;')
    git('add', '.')
    assert(readSharedPaths(root).length > 0)
    const second = await save()
    assert.deepEqual(second, first)
    assert.deepEqual(hashes(), before)
  })
  it('retains an empty nested CSS input so future content changes the real fingerprint', async () => {
    const first = await save(),
      before = hashes()
    assert(first.files.some((file) => file.path === 'library/catalyst-api/empty.css'))
    put('library/catalyst-api/empty.css', '.later{color:blue}')
    assert.throws(() => readSharedPaths(root), /Stale/)
    git('add', 'library/catalyst-api/empty.css')
    await save()
    assert.notDeepEqual(hashes(), before)
  })
  it('validates a filtered source without Git and refuses stale bytes, new roots and symlinks', async () => {
    const expected = await save()
    rmSync(path.join(root, '.git'), { recursive: true, force: true })
    put(WEB + '/src/engine/generated/Player.ts', 'export interface Player {}')
    assert.deepEqual(
      hashes(),
      Object.fromEntries(
        [...expected.controls, ...expected.files].map((entry) => [entry.path, entry.sha256])
      )
    )
    assert(readSharedPaths(root).length > 0)
    assert.deepEqual(
      JSON.parse(await generateSharedInputs({ root, filteredSource: true })),
      expected
    )
    await assert.rejects(generateSharedInputs({ root }), /requires a Git source checkout/)
    for (const control of ['library/tsconfig.json', 'package.json']) {
      put(control, '{}')
      assert.throws(() => readSharedPaths(root), /New HUD resolver control/)
      rmSync(path.join(root, control))
    }

    const metadata = 'library/ui/foundations/typography/font-source.json'
    const original = readFileSync(path.join(root, metadata))
    put(metadata, 'changed font identity')
    assert.throws(() => readSharedPaths(root), /Stale/)
    put(metadata, original)
    put(WEB + '/src/new.ts', 'export const added = 1')
    assert.throws(() => readSharedPaths(root), /root inventory/)
    rmSync(path.join(web, 'src/new.ts'))
    const token = 'library/ui/foundations/token.ts'
    const bytes = readFileSync(path.join(root, token))
    put('outside-token.ts', bytes)
    rmSync(path.join(root, token))
    symlinkSync(path.join(root, 'outside-token.ts'), path.join(root, token))
    assert.throws(() => readSharedPaths(root), /Linked/)
  })
  it('refuses a new nearer resolver control without replacing the checked manifest', async () => {
    await save()
    const original = readFileSync(path.join(root, MANIFEST))
    put('library/ui/foundations/tsconfig.json', '{}')
    git('add', '.')
    assert.throws(() => readSharedPaths(root), /New HUD resolver control/)
    assert.throws(hashes, /New HUD resolver control/)
    assert.deepEqual(readFileSync(path.join(root, MANIFEST)), original)
    const updated = await save()
    assert(updated.controls.some((entry) => entry.path === 'library/ui/foundations/tsconfig.json'))
  })
  it('binds actual imported Vite configuration dependencies and refuses their stale identities', async () => {
    const config = WEB + '/vite.config.ts'
    put(config, "import './config-helper.ts';\n" + readFileSync(path.join(root, config)))
    put(WEB + '/config-helper.ts', 'export const resolverIdentity=1;')
    git('add', '.')
    const manifest = await save()
    assert(manifest.controls.some((entry) => entry.path === WEB + '/config-helper.ts'))
    put(WEB + '/config-helper.ts', 'export const resolverIdentity=2;')
    assert.throws(() => readSharedPaths(root), /Stale/)
  })
  it('refuses automatic PostCSS configuration without replacing the manifest', async () => {
    await save()
    const original = readFileSync(path.join(root, MANIFEST))
    put(WEB + '/postcss.config.mjs', 'export default {plugins:[]}')
    git('add', '.')
    await assert.rejects(generateSharedInputs({ root }), /Automatic PostCSS/)
    assert.deepEqual(readFileSync(path.join(root, MANIFEST)), original)
  })
  it('preserves inline PostCSS plugins instead of replacing the actual pipeline', async () => {
    const config = WEB + '/vite.config.ts'
    put(
      config,
      readFileSync(path.join(root, config), 'utf8').replace(
        'build:{',
        "css:{postcss:{plugins:[{postcssPlugin:'config-sentinel',Once(){throw Error('Actual inline PostCSS plugin executed')}}]}},build:{"
      )
    )
    git('add', '.')
    await assert.rejects(generateSharedInputs({ root }), /Actual inline PostCSS plugin executed/)
  })
  for (const property of ['composes', 'compose-with', 'COMPOSES'])
    it(
      'refuses unsupported ' +
        property +
        ' through the actual processor without replacing the manifest',
      async () => {
        await save()
        const original = readFileSync(path.join(root, MANIFEST))
        put(
          'library/catalyst-api/model.ts',
          "import './model.module.css';export type Datum={id:number};"
        )
        put(
          'library/catalyst-api/model.module.css',
          '.model{' +
            property +
            ":base from './composed.module.css';background:url('./type-pixel.svg')}"
        )
        put('library/catalyst-api/composed.module.css', '.base{color:purple}')
        git('add', '.')
        await assert.rejects(
          generateSharedInputs({ root }),
          /Unsupported HUD CSS Modules composition/
        )
        assert.throws(
          () =>
            execFileSync(
              process.execPath,
              [path.join(web, 'scripts/shared-inputs.mjs'), '--write'],
              { cwd: root, stdio: 'pipe' }
            ),
          (error) =>
            error.status === 1 &&
            error.stderr.toString().includes('Upstream dependency reporting is required')
        )
        assert.deepEqual(readFileSync(path.join(root, MANIFEST)), original)
      }
    )
  it('refuses an unresolved CSS asset instead of emitting an incomplete graph', async () => {
    put('library/ui/foundations/style.css', ".token{background:url('./missing.svg')}")
    git('add', 'library/ui/foundations/style.css')
    await assert.rejects(generateSharedInputs({ root }), /Unresolved Vite dependency/)
  })
  it('refuses unresolved and untracked import targets and a newly added root with a stale manifest', async () => {
    await save()
    put(WEB + '/src/added.ts', 'export const added=1;')
    git('add', WEB + '/src/added.ts')
    assert.throws(() => readSharedPaths(root), /root inventory/)
    put(WEB + '/src/main.ts', "import {missing}from'@shop/missing';export{missing};")
    await assert.rejects(generateSharedInputs({ root }), /compiler prerequisites/)
    put('library/shop/missing.ts', 'export const missing=1;')
    await assert.rejects(generateSharedInputs({ root }), /Untracked shared/)
  })
})
