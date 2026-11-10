import assert from 'node:assert/strict'
import { Buffer } from 'node:buffer'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
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
  it('normalizes caller roots and ignores only exact regular Finder metadata', async () => {
    await save()
    for (const alternate of [root + '/', path.join(root, 'library', '..')]) {
      assert.deepEqual(readSharedPaths(alternate), readSharedPaths(root))
    }
    put('library/shop/.DS_Store', 'Finder metadata')
    assert(readSharedPaths(root).length > 0)
    assert(Object.keys(hashes()).length > 0)
    await save()
    put('library/shop/.DS_Store.ts', 'export {}')
    assert.throws(() => readSharedPaths(root), /Untracked/)
    rmSync(path.join(root, 'library/shop/.DS_Store'))
    symlinkSync(path.join(root, 'library/shop/data.ts'), path.join(root, 'library/shop/.DS_Store'))
    assert.throws(() => readSharedPaths(root), /Unsafe/)
  })
  it('isolates production environment from Vitest and restores the caller on failure', async () => {
    const original = process.env.NODE_ENV
    try {
      delete process.env.NODE_ENV
      const config = readFileSync(path.join(web, 'vite.config.ts'), 'utf8')
      put(
        WEB + '/vite.config.ts',
        config.replace(
          'resolve:{',
          "plugins:[{name:'environment-check',buildStart(){if(mode!=='test'&&process.env.NODE_ENV!=='production')throw Error('wrong production environment')}}],resolve:{"
        )
      )
      git('add', '.')
      await save()
      assert.equal(process.env.NODE_ENV, undefined)
      process.env.NODE_ENV = 'caller-value'
      put(
        WEB + '/vite.config.ts',
        config.replace(
          'resolve:{',
          "plugins:[{name:'fail-build',buildStart(){throw Error('intentional build failure')}}],resolve:{"
        )
      )
      git('add', '.')
      await assert.rejects(generateSharedInputs({ root }), /intentional build failure/)
      assert.equal(process.env.NODE_ENV, 'caller-value')
    } finally {
      if (original === undefined) delete process.env.NODE_ENV
      else process.env.NODE_ENV = original
    }
  })
  it('executes the real check CLI and refuses stale bytes without rewriting the manifest', async () => {
    await save()
    const previous = readFileSync(path.join(root, MANIFEST))
    const command = () =>
      execFileSync(process.execPath, [path.join(web, 'scripts/shared-inputs.mjs'), '--check'], {
        cwd: root,
        stdio: 'pipe'
      })
    command()
    put(
      WEB + '/src/main.ts',
      readFileSync(path.join(web, 'src/main.ts'), 'utf8') + '\n// changed owned input\n'
    )
    assert.throws(
      command,
      (error) => error.status === 1 && error.stderr.toString().includes('manifest is stale')
    )
    assert.deepEqual(readFileSync(path.join(root, MANIFEST)), previous)
  })
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
  it('ignores unrelated existing shared-file bodies and the unimported public root', async () => {
    put('library/shop/unused.ts', 'export const unused=0;')
    git('add', 'library/shop/unused.ts')
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
  for (const control of [
    'package.json',
    'library/tsconfig.json',
    'rendering/bevy-explorer/package.json'
  ])
    it('regenerates and repeats after adding ancestor resolver control ' + control, async () => {
      await save()
      const before = hashes()
      put(control, control.endsWith('package.json') ? '{"type":"module"}' : '{}')
      git('add', control)
      assert.throws(() => readSharedPaths(root), /New HUD resolver control/)
      const updated = await save()
      const identity = updated.controls.find((entry) => entry.path === control)
      assert(identity)
      assert.equal(hashes()[control], identity.sha256)
      assert.notDeepEqual(hashes(), before)
      assert.deepEqual(JSON.parse(await generateSharedInputs({ root })), updated)
      rmSync(path.join(root, '.git'), { recursive: true, force: true })
      assert(readSharedPaths(root).length > 0)
      assert.deepEqual(
        JSON.parse(await generateSharedInputs({ root, filteredSource: true })),
        updated
      )
    })
  it('refuses ancestor resolver bytes changed during generation without replacing the manifest', async () => {
    const control = 'package.json'
    put(control, '{"type":"module"}')
    git('add', control)
    await save()
    const original = readFileSync(path.join(root, MANIFEST))
    const config = WEB + '/vite.config.ts'
    put(
      config,
      "import {appendFileSync} from 'node:fs';\n" +
        readFileSync(path.join(root, config), 'utf8').replace(
          'build:{',
          "plugins:[{name:'mutate-ancestor-control',buildStart(){appendFileSync(path.resolve(here,'../../../../package.json'),'\\n')}}],build:{"
        )
    )
    git('add', config)
    await assert.rejects(
      generateSharedInputs({ root }),
      /HUD input changed while resolving imports: package.json/
    )
    assert.deepEqual(readFileSync(path.join(root, MANIFEST)), original)
  })
  for (const [control, operation] of [
    ['package.json', 'add'],
    ['library/tsconfig.json', 'add'],
    ['package.json', 'untrack'],
    ['package.json', 'delete']
  ])
    it(
      'refuses resolver presence or tracking drift during generation: ' + operation + ' ' + control,
      async () => {
        if (operation !== 'add') {
          put(control, '{"type":"module"}')
          git('add', control)
        }
        await save()
        const original = readFileSync(path.join(root, MANIFEST))
        const config = WEB + '/vite.config.ts'
        const mutation =
          operation === 'add'
            ? `writeFileSync(target, ${JSON.stringify(control.endsWith('package.json') ? '{"type":"module"}' : '{}')});execFileSync('git',['-C',repo,'add',${JSON.stringify(control)}]);`
            : operation === 'delete'
              ? 'unlinkSync(target);'
              : `execFileSync('git',['-C',repo,'rm','--cached',${JSON.stringify(control)}]);`
        put(
          config,
          "import {existsSync,writeFileSync,unlinkSync} from 'node:fs';import {execFileSync} from 'node:child_process';\n" +
            readFileSync(path.join(root, config), 'utf8').replace(
              'build:{',
              `plugins:[{name:'mutate-resolver-presence',buildStart(){const repo=path.resolve(here,'../../../..'),target=path.join(repo,${JSON.stringify(control)}),marker=path.join(here,'resolver-mutated');if(!existsSync(marker)){${mutation}writeFileSync(marker,'done')}}}],build:{`
            )
        )
        git('add', config)
        await assert.rejects(
          generateSharedInputs({ root }),
          /HUD resolver control inventory changed/
        )
        assert.deepEqual(readFileSync(path.join(root, MANIFEST)), original)
      }
    )
  it('refuses an untracked or linked ancestor resolver before the compiler runs', async () => {
    await save()
    const original = readFileSync(path.join(root, MANIFEST))
    put('package.json', '{}')
    await assert.rejects(
      generateSharedInputs({ root }),
      /Untracked HUD resolver control: package.json/
    )
    git('add', 'package.json')
    rmSync(path.join(root, 'package.json'))
    put('outside-package.json', '{}')
    symlinkSync(path.join(root, 'outside-package.json'), path.join(root, 'package.json'))
    await assert.rejects(generateSharedInputs({ root }), /Linked shared input: package.json/)
    assert.deepEqual(readFileSync(path.join(root, MANIFEST)), original)
  })
  for (const mutation of ['add', 'delete', 'bytes'])
    it(
      'refuses filtered-source ancestor ' +
        mutation +
        ' during generation without replacing metadata',
      async () => {
        put('package.json', '{"type":"module"}')
        const config = WEB + '/vite.config.ts'
        const target = mutation === 'add' ? 'library/tsconfig.json' : 'package.json'
        const operation =
          mutation === 'delete'
            ? 'unlinkSync(target)'
            : mutation === 'bytes'
              ? "appendFileSync(target,'\\n')"
              : "writeFileSync(target,'{}')"
        put(
          config,
          "import {existsSync,writeFileSync,unlinkSync,appendFileSync} from 'node:fs';\n" +
            readFileSync(path.join(root, config), 'utf8').replace(
              'build:{',
              `plugins:[{name:'filtered-resolver-mutation',buildStart(){const repo=path.resolve(here,'../../../..'),target=path.join(repo,${JSON.stringify(target)}),marker=path.join(here,'filtered-mutated');if(process.env.HUD_FILTERED_MUTATION==='yes'&&!existsSync(marker)){${operation};writeFileSync(marker,'done')}}}],build:{`
            )
        )
        git('add', '.')
        await save()
        const original = readFileSync(path.join(root, MANIFEST))
        rmSync(path.join(root, '.git'), { recursive: true, force: true })
        assert(readSharedPaths(root).length > 0)
        const before = process.env.HUD_FILTERED_MUTATION
        try {
          process.env.HUD_FILTERED_MUTATION = 'yes'
          await assert.rejects(
            generateSharedInputs({ root, filteredSource: true }),
            /HUD resolver control inventory changed|HUD input changed while resolving imports: package.json/
          )
        } finally {
          if (before === undefined) delete process.env.HUD_FILTERED_MUTATION
          else process.env.HUD_FILTERED_MUTATION = before
        }
        assert.deepEqual(readFileSync(path.join(root, MANIFEST)), original)
      }
    )
  for (const [source, directory] of [
    [WEB + '/bridge-scene/src/owned.ts', WEB + '/bridge-scene'],
    [
      'rendering/bevy-explorer/source/deploy/web/owned.ts',
      'rendering/bevy-explorer/source/deploy/web'
    ]
  ])
    it(
      'binds resolver ancestors of actually consumed non-root owned control ' + source,
      async () => {
        put(source, 'export const ownedValue=1;')
        const importer = WEB + '/src/main.ts'
        const relative = path.posix
          .relative(path.posix.dirname(importer), source)
          .replace(/\.ts$/, '')
        put(importer, `import {ownedValue}from${JSON.stringify(relative)};export{ownedValue};`)
        const scope = directory + '/package.json'
        put(scope, '{"type":"module"}')
        git('add', '.')
        const first = await save()
        assert(first.controls.some((entry) => entry.path === source))
        assert(first.controls.some((entry) => entry.path === scope))
        const original = readFileSync(path.join(root, MANIFEST))
        put(scope, '{"type":"module","version":"2"}')
        assert.throws(() => readSharedPaths(root), /Stale/)
        assert.throws(hashes, /Stale/)
        assert.deepEqual(readFileSync(path.join(root, MANIFEST)), original)
        git('add', scope)
        const updated = await save()
        assert.notDeepEqual(updated, first)
        const nearer = directory + '/jsconfig.json'
        put(nearer, '{}')
        git('add', nearer)
        assert.throws(() => readSharedPaths(root), /New HUD resolver control/)
        assert.throws(hashes, /New HUD resolver control/)
        const refreshed = await save()
        assert(refreshed.controls.some((entry) => entry.path === nearer))
        assert.deepEqual(JSON.parse(await generateSharedInputs({ root })), refreshed)
      }
    )
  it('refuses every shared negative-manifest corpus case', async () => {
    const nodeModule = 'library/shop/node_modules/fixture.ts'
    put(nodeModule, 'export const dependency = 1')
    git('add', '-f', nodeModule)
    const lfsPayload = Buffer.concat([Buffer.from('wOF2'), Buffer.alloc(60)])
    const lfsOid = createHash('sha256').update(lfsPayload).digest('hex')
    const decimalPath = 'library/shop/decimal-lfs.woff2',
      pointerPath = 'library/shop/pointer.woff2'
    put(decimalPath, lfsPayload)
    put(
      pointerPath,
      'version https://git-lfs.github.com/spec/v1\noid sha256:' + lfsOid + '\nsize 64\n'
    )
    const crlfPath = 'library/shop/pointer-crlf.woff2'
    put(crlfPath, readFileSync(path.join(root, pointerPath)).toString().replaceAll('\n', '\r\n'))
    git('add', crlfPath)
    git('add', decimalPath, pointerPath)
    const pointerBlob = git('hash-object', '-w', path.join(root, pointerPath)).toString().trim()
    git('update-index', '--add', '--cacheinfo', '100644', pointerBlob, decimalPath)
    const baseline = await save()
    const cases = JSON.parse(
      readFileSync(path.join(repository, 'tooling/tests/hud_manifest_invalid_cases.json'), 'utf8')
    )
    for (const scenario of cases) {
      const manifest = JSON.parse(JSON.stringify(baseline)),
        row = manifest.files[0]
      switch (scenario.mutation) {
        case 'null-lfs':
          row.lfs = null
          break
        case 'extra-lfs-key':
          row.lfs = { oid: row.sha256, size: 0, extra: true }
          break
        case 'unsafe-lfs-size':
          row.lfs = { oid: row.sha256, size: 9007199254740992 }
          break
        case 'duplicate-groups':
          manifest.files.push({ ...manifest.controls[0] })
          break
        case 'owned-file': {
          const owned = manifest.controls.find((entry) => entry.path === WEB + '/src/main.ts')
          manifest.controls = manifest.controls.filter((entry) => entry !== owned)
          manifest.files.push(owned)
          break
        }
        case 'node-modules-file':
          manifest.files.push({
            path: nodeModule,
            sha256: createHash('sha256').update('export const dependency = 1').digest('hex')
          })
          break
        case 'integral-decimal':
        case 'fractional-lfs-size':
        case 'boolean-lfs-size':
        case 'unmaterialized-lfs':
          manifest.schema = scenario.mutation === 'integral-decimal' ? 2.0 : 2
          manifest.files.push({
            path: scenario.mutation === 'unmaterialized-lfs' ? pointerPath : decimalPath,
            sha256: lfsOid,
            lfs: {
              oid: lfsOid,
              size:
                scenario.mutation === 'fractional-lfs-size'
                  ? 64.5
                  : scenario.mutation === 'boolean-lfs-size'
                    ? true
                    : 64.0
            }
          })
          break
        case 'forged-pointer-raw-hash':
        case 'forged-crlf-pointer-lfs':
        case 'wrong-materialized-lfs-size': {
          const target =
            scenario.mutation === 'wrong-materialized-lfs-size'
              ? decimalPath
              : scenario.mutation === 'forged-crlf-pointer-lfs'
                ? crlfPath
                : pointerPath
          const bytes = readFileSync(path.join(root, target)),
            sha256 = createHash('sha256').update(bytes).digest('hex')
          const entry = { path: target, sha256 }
          if (scenario.mutation !== 'forged-pointer-raw-hash')
            entry.lfs = {
              oid: sha256,
              size: scenario.mutation === 'wrong-materialized-lfs-size' ? 32 : bytes.length
            }
          manifest.files.push(entry)
          break
        }
        case 'missing-resolution-scope':
          manifest.resolutionDirectories = []
          break
        case 'extra-resolution-scope':
          manifest.resolutionDirectories.push({
            path: 'library/ui/unused',
            entries: []
          })
          break
        case 'unsorted-namespace':
          manifest.resolutionDirectories[0].entries.reverse()
          break
        case 'duplicate-namespace-name':
          manifest.resolutionDirectories[0].entries.push({
            ...manifest.resolutionDirectories[0].entries[0]
          })
          break
        case 'unsafe-namespace-name':
          manifest.resolutionDirectories[0].entries[0].name = '../escape'
          break
        case 'control-character-namespace-name':
          manifest.resolutionDirectories[0].entries[0].name = 'bad\n\nfile'
          break
        case 'boolean-schema':
          manifest.schema = true
          break
        case 'fractional-schema':
          manifest.schema = 2.5
          break
        case 'invalid-namespace-kind':
          manifest.resolutionDirectories[0].entries[0].kind = 'symlink'
          break
        case 'duplicate-edge':
          manifest.edges.push({ ...manifest.edges[0] })
          break
        case 'unsorted-edges':
          manifest.edges.reverse()
          break
        default:
          assert.fail('Unknown shared corpus mutation: ' + scenario.mutation)
      }
      manifest.files.sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0))
      let bytes = JSON.stringify(manifest)
      if (scenario.mutation === 'integral-decimal')
        bytes = bytes.replace('"schema":2', '"schema":2.0').replace('"size":64', '"size":64.0')
      put(MANIFEST, bytes)
      if (scenario.valid) {
        assert(readSharedPaths(root).length > 0)
        hashes()
      } else {
        assert.throws(() => readSharedPaths(root), undefined, scenario.name)
        assert.throws(hashes, undefined, scenario.name)
      }
    }
    put(MANIFEST, JSON.stringify(baseline))
    assert(readSharedPaths(root).length > 0)
  })
  it('externalizes only canonical engine outputs with Windows native and POSIX absolute module IDs', () => {
    const source = readFileSync(path.join(here, 'shared-inputs.mjs'), 'utf8')
    const callbackStart =
      source.indexOf('options.external = (id, importer) => {') + 'options.external = '.length
    const callbackEnd =
      source.indexOf('\n                }', callbackStart) + '\n                }'.length
    const rootsStart =
      source.indexOf("const engineOutputs = ['pkg', 'pkg-scene'].map((name) =>") +
      'const engineOutputs = '.length
    const rootsEnd = source.indexOf('\n                options.external', rootsStart)
    assert(
      callbackStart > 'options.external = '.length &&
        rootsEnd > rootsStart &&
        callbackEnd > callbackStart
    )
    const root = 'C:\\repo'
    const directories = new Function(
      'path',
      'root',
      'return (' + source.slice(rootsStart, rootsEnd) + ')'
    )(path.win32, root)
    const external = new Function(
      'path',
      'engineOutputs',
      'return (' + source.slice(callbackStart, callbackEnd) + ')'
    )(path.win32, directories)
    const importer = 'C:/repo/rendering/bevy-explorer/source/react-web/src/engine/client.ts'
    for (const name of ['pkg', 'pkg-scene']) {
      const native = path.win32.join(
        root,
        'rendering/bevy-explorer/source/deploy/web/engine',
        name,
        'module.js'
      )
      assert.equal(external(native, importer), true)
      assert.equal(external(native.replaceAll('\\', '/'), importer), true)
      assert.equal(
        external(
          path.win32.relative(path.win32.dirname(importer), native).replaceAll('\\', '/'),
          importer
        ),
        true
      )
    }
    assert.equal(
      external(
        'C:/repo/rendering/bevy-explorer/source/deploy/web/engine/pkg-extra/module.js',
        importer
      ),
      false
    )
    assert.equal(external('C:/repo/library/ui/foundations/index.ts', importer), false)
  })
  it('refuses tracked and untracked sibling shadows until canonical resolution updates the namespace', async () => {
    put('library/ui/foundations/foo/index.ts', 'export const shadow=73;')
    put('library/ui/foundations/index.ts', "export{shadow}from'./foo';")
    put(WEB + '/src/main.ts', "import{shadow}from'@dcl/one-ui/foundations';export{shadow};")
    git('add', '.')
    const original = await save(),
      before = hashes(),
      bytes = readFileSync(path.join(root, MANIFEST))
    assert(original.files.some((entry) => entry.path === 'library/ui/foundations/foo/index.ts'))
    put('library/ui/foundations/foo.ts', 'export const shadow=99;')
    assert.throws(() => readSharedPaths(root), /Untracked/)
    assert.throws(hashes, /Untracked/)
    await assert.rejects(generateSharedInputs({ root }), /Untracked|namespace/)
    assert.deepEqual(readFileSync(path.join(root, MANIFEST)), bytes)
    git('add', 'library/ui/foundations/foo.ts')
    assert.throws(() => readSharedPaths(root), /Stale HUD resolution namespace/)
    assert.throws(hashes, /Stale HUD resolution namespace/)
    const updated = await save()
    assert(updated.files.some((entry) => entry.path === 'library/ui/foundations/foo.ts'))
    assert(!updated.files.some((entry) => entry.path === 'library/ui/foundations/foo/index.ts'))
    assert.notDeepEqual(hashes(), before)
    const after = hashes()
    put('library/ui/foundations/foo/index.ts', 'export const shadow=123;')
    git('add', 'library/ui/foundations/foo/index.ts')
    assert.deepEqual(hashes(), after)
    assert.deepEqual(JSON.parse(await generateSharedInputs({ root })), updated)
  })
  it('migrates a checked schema1 Git source through real Vite config loading and repeats filtered schema2', async () => {
    put('library/shop/unused-before-filter.ts', 'export const unused=1;')
    put(
      WEB + '/vite.shared.ts',
      "import{readSharedPaths}from'./scripts/shared-inputs.mjs';export const paths=readSharedPaths(" +
        JSON.stringify(root) +
        ');'
    )
    const config = WEB + '/vite.config.ts'
    put(config, "import './vite.shared';\n" + readFileSync(path.join(root, config), 'utf8'))
    git('add', '.')
    const first = await save(),
      old = JSON.parse(JSON.stringify(first))
    old.schema = 1
    delete old.resolutionDirectories
    put(MANIFEST, JSON.stringify(old))
    git('add', MANIFEST)
    assert.throws(() => readSharedPaths(root), /Invalid HUD shared-input contract/)
    assert.throws(hashes, /Unsupported HUD input manifest schema/)
    const migrated = await save()
    assert.deepEqual(migrated, first)
    rmSync(path.join(root, '.git'), { recursive: true, force: true })
    rmSync(path.join(root, 'library/shop/unused-before-filter.ts'))
    assert(readSharedPaths(root).length > 0)
    assert.deepEqual(
      JSON.parse(await generateSharedInputs({ root, filteredSource: true })),
      migrated
    )
    put('library/shop/new-unlisted.ts', 'export const unexpected=1')
    assert.throws(() => readSharedPaths(root), /Unexpected filtered/)
    assert.throws(hashes, /Unexpected filtered/)
  })
  it('witnesses output-prefixed regular files but excludes only canonical output directories', async () => {
    for (const name of ['A.ts', 'a-lower.ts', '_punct.ts', '中.ts', '\ue000.ts', '\u{10000}.ts'])
      put('library/shop/' + name, 'export {}')
    git('add', '.')
    const initial = await save()
    const names = initial.resolutionDirectories
      .find((row) => row.path === 'library/shop')
      .entries.map((entry) => entry.name)
    assert.deepEqual(
      names.filter((name) =>
        ['A.ts', 'a-lower.ts', '_punct.ts', '中.ts', '\ue000.ts', '\u{10000}.ts'].includes(name)
      ),
      ['A.ts', '_punct.ts', 'a-lower.ts', '中.ts', '\ue000.ts', '\u{10000}.ts']
    )
    put('library/shop/.consumer-foundations-private/output.ts', 'canonical output')
    assert(readSharedPaths(root).length > 0)
    put('library/shop/.consumer-foundations-shadow.ts', 'export {}')
    assert.throws(() => readSharedPaths(root), /Untracked/)
    git('add', '-f', 'library/shop/.consumer-foundations-shadow.ts')
    assert.throws(() => readSharedPaths(root), /Stale HUD resolution namespace/)
    const updated = await save()
    assert(
      updated.resolutionDirectories
        .find((row) => row.path === 'library/shop')
        .entries.some(
          (entry) => entry.name === '.consumer-foundations-shadow.ts' && entry.kind === 'regular'
        )
    )
    const previous = readFileSync(path.join(root, MANIFEST))
    symlinkSync(
      path.join(root, 'library/shop/data.ts'),
      path.join(root, 'library/shop/unsafe-alias.ts')
    )
    await assert.rejects(generateSharedInputs({ root }), /Unsafe resolution namespace|Untracked/)
    assert.deepEqual(readFileSync(path.join(root, MANIFEST)), previous)
  })
  it('rejects resealed CRLF pointer bytes before the filtered LFS fast path', async () => {
    const file = 'library/shop/forged-pointer.woff2'
    const pointer = Buffer.from(
      [
        'version https://git-lfs.github.com/spec/v1',
        'oid sha256:' + 'a'.repeat(64),
        'size 64',
        ''
      ].join('\r\n')
    )
    put(file, pointer)
    git('add', file)
    const manifest = await save()
    const oid = createHash('sha256').update(pointer).digest('hex')
    manifest.files.push({
      path: file,
      sha256: oid,
      lfs: { oid, size: pointer.length }
    })
    manifest.files.sort((a, b) => Buffer.compare(Buffer.from(a.path), Buffer.from(b.path)))
    put(MANIFEST, JSON.stringify(manifest))
    rmSync(path.join(root, '.git'), { recursive: true, force: true })
    const previous = readFileSync(path.join(root, MANIFEST))
    assert.throws(() => readSharedPaths(root), /Unmaterialized/)
    assert.throws(hashes, /Unmaterialized/)
    await assert.rejects(generateSharedInputs({ root, filteredSource: true }), /Unmaterialized/)
    assert.deepEqual(readFileSync(path.join(root, MANIFEST)), previous)
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
  it('runs a non-idempotent inline PostCSS transform exactly once per AST', async () => {
    const config = WEB + '/vite.config.ts'
    put(
      config,
      "import {appendFileSync} from 'node:fs';\n" +
        readFileSync(path.join(root, config), 'utf8').replace(
          'build:{',
          "css:{postcss:{plugins:[{postcssPlugin:'non-idempotent-inline',Once(css){if(css.raws.checkedOnce)throw Error('Inline PostCSS plugin duplicated');css.raws.checkedOnce=true;css.walkDecls('color',declaration=>{if(declaration.value==='red'){declaration.value='blue';appendFileSync(path.join(here,'postcss-transform.log'),'blue\\n')}else if(declaration.value==='blue')throw Error('Inline PostCSS transform repeated')})}}]}},build:{"
        )
    )
    git('add', config)
    const first = await save()
    assert.match(readFileSync(path.join(web, 'postcss-transform.log'), 'utf8'), /blue/)
    assert(first.files.some((entry) => entry.path === 'library/catalyst-api/nested.css'))
    assert.deepEqual(JSON.parse(await generateSharedInputs({ root })), first)
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
  for (const change of ['hud', 'new-root', 'config', 'lock'])
    it(
      'loads the actual Vite configuration after a local ' +
        change +
        ' edit while strict validation refuses it',
      async () => {
        put(
          WEB + '/vite.shared.ts',
          "import {readSharedPaths} from './scripts/shared-inputs.mjs';export const paths=readSharedPaths(" +
            JSON.stringify(root) +
            ",{mode:'config'});"
        )
        const config = WEB + '/vite.config.ts'
        put(config, "import './vite.shared';\n" + readFileSync(path.join(root, config)))
        git('add', '.')
        await save()
        if (change === 'hud')
          put(
            WEB + '/src/main.ts',
            readFileSync(path.join(web, 'src/main.ts')) + '\n// local edit\n'
          )
        else if (change === 'new-root') {
          put(WEB + '/src/local-added.ts', 'export const local=1;')
          git('add', WEB + '/src/local-added.ts')
        } else if (change === 'config')
          put(config, readFileSync(path.join(root, config)) + '\n// local resolver edit\n')
        else put(WEB + '/package-lock.json', '{"lockfileVersion":3,"localChange":true}')
        const before = readFileSync(path.join(root, MANIFEST))
        assert.throws(() => readSharedPaths(root), /Stale/)
        assert(readSharedPaths(root, { mode: 'config' }).length > 0)
        const { loadConfigFromFile } = await import(
          pathToFileURL(path.join(web, 'node_modules/vite/dist/node/index.js')).href
        )
        assert(
          await loadConfigFromFile(
            { command: 'serve', mode: 'development' },
            path.join(web, 'vite.config.ts'),
            web
          )
        )
        assert.deepEqual(readFileSync(path.join(root, MANIFEST)), before)
      }
    )
  for (const operation of ['delete', 'rename'])
    it(
      'loads actual Vite config after an owned root ' +
        operation +
        ' while strict validation refuses it',
      async () => {
        const file = WEB + '/src/local-edit.ts'
        put(file, 'export const local=1;')
        put(
          WEB + '/vite.shared.ts',
          "import {readSharedPaths} from './scripts/shared-inputs.mjs';export const paths=readSharedPaths(" +
            JSON.stringify(root) +
            ",{mode:'config'});"
        )
        const config = WEB + '/vite.config.ts'
        put(config, "import './vite.shared';\n" + readFileSync(path.join(root, config)))
        git('add', '.')
        await save()
        const before = readFileSync(path.join(root, MANIFEST))
        if (operation === 'delete') {
          rmSync(path.join(root, file))
          git('add', '-u', file)
        } else git('mv', file, WEB + '/src/renamed-edit.ts')
        assert.throws(() => readSharedPaths(root), /Stale/)
        const { loadConfigFromFile } = await import(
          pathToFileURL(path.join(web, 'node_modules/vite/dist/node/index.js')).href
        )
        assert(
          await loadConfigFromFile(
            { command: 'serve', mode: 'development' },
            path.join(web, 'vite.config.ts'),
            web
          )
        )
        assert.deepEqual(readFileSync(path.join(root, MANIFEST)), before)
      }
    )
  it('reports a missing shared input actionably and allows its canonical regeneration', async () => {
    put(
      WEB + '/vite.shared.ts',
      "import {readSharedPaths} from './scripts/shared-inputs.mjs';export const paths=readSharedPaths(" +
        JSON.stringify(root) +
        ",{mode:'config'});"
    )
    const config = WEB + '/vite.config.ts'
    put(config, "import './vite.shared';\n" + readFileSync(path.join(root, config)))
    git('add', '.')
    await save()
    git('mv', 'library/ui/foundations/token.ts', 'library/ui/foundations/token-next.ts')
    put(
      'library/ui/foundations/index.ts',
      readFileSync(path.join(root, 'library/ui/foundations/index.ts'), 'utf8').replace(
        './token',
        './token-next'
      )
    )
    git('add', 'library/ui/foundations/index.ts')
    assert.throws(
      () => readSharedPaths(root, { mode: 'config' }),
      /Missing shared input .*run npm run gen:shared-inputs/
    )
    const { loadConfigFromFile } = await import(
      pathToFileURL(path.join(web, 'node_modules/vite/dist/node/index.js')).href
    )
    await assert.rejects(
      loadConfigFromFile(
        { command: 'serve', mode: 'development' },
        path.join(web, 'vite.config.ts'),
        web
      ),
      /Missing shared input .*run npm run gen:shared-inputs/
    )
    assert.throws(
      () =>
        execFileSync(
          'python3',
          [
            path.join(repository, 'tooling/hud_shared_inputs.py'),
            '--root',
            root,
            '--newer-than',
            path.join(root, 'native-stamp')
          ],
          { stdio: 'pipe' }
        ),
      (error) =>
        error.status === 2 &&
        error.stderr.toString().includes('npm --prefix ' + WEB + ' run gen:shared-inputs')
    )
    const regenerated = await save()
    assert(regenerated.files.some((entry) => entry.path === 'library/ui/foundations/token-next.ts'))
    assert(Object.hasOwn(hashes(), 'library/ui/foundations/token-next.ts'))
    assert(readSharedPaths(root).length > 0)
  })
  it('keeps malformed metadata and unsafe physical inputs closed in config mode despite owned-byte drift', async () => {
    await save()
    const original = JSON.parse(readFileSync(path.join(root, MANIFEST)))
    put(WEB + '/src/main.ts', 'export const local=1;')
    for (const mutate of [
      (manifest) => {
        manifest.schema = 99
      },
      (manifest) => {
        manifest.files[0].path = '../escape'
      },
      (manifest) => {
        manifest.files[0].lfs = null
      },
      (manifest) => {
        manifest.roots.push('../escape')
      },
      (manifest) => {
        manifest.edges.push({
          from: WEB + '/src/main.ts',
          to: 'missing',
          kind: 'test'
        })
      }
    ]) {
      const manifest = globalThis.structuredClone(original)
      mutate(manifest)
      put(MANIFEST, JSON.stringify(manifest))
      assert.throws(() => readSharedPaths(root, { mode: 'config' }))
    }
    put(MANIFEST, JSON.stringify(original))
    const token = 'library/ui/foundations/token.ts',
      bytes = readFileSync(path.join(root, token))
    rmSync(path.join(root, token))
    assert.throws(() => readSharedPaths(root, { mode: 'config' }))
    put('outside-token.ts', bytes)
    symlinkSync(path.join(root, 'outside-token.ts'), path.join(root, token))
    assert.throws(() => readSharedPaths(root, { mode: 'config' }), /Linked/)
  })
  it('binds an actually consumed tracked engine source as a control and changes its fingerprint', async () => {
    const source = 'rendering/bevy-explorer/source/deploy/web/audit-engine-helper.ts'
    put(source, 'export const engineValue=73;')
    put(
      WEB + '/src/main.ts',
      "import {engineValue} from '../../deploy/web/audit-engine-helper';export {engineValue};"
    )
    git('add', '.')
    const manifest = await save(),
      before = hashes()
    assert(manifest.controls.some((entry) => entry.path === source))
    assert(!manifest.files.some((entry) => entry.path === source))
    put(source, 'export const engineValue=99;')
    assert.throws(() => readSharedPaths(root), /Stale/)
    git('add', source)
    await save()
    assert.notDeepEqual(hashes(), before)
  })
  for (const scenario of [
    {
      name: 'an untracked consumed HUD module',
      target: WEB + '/src/audit-untracked.ts',
      tracked: false,
      error: 'Untracked HUD import target'
    },
    {
      name: 'a tracked repository module outside the supported shared domains',
      target: 'library/audit-extra/value.ts',
      tracked: true,
      error: 'Unsupported resolved first-party input'
    }
  ])
    it('refuses ' + scenario.name + ' without replacing the previous manifest', async () => {
      await save()
      const previous = readFileSync(path.join(root, MANIFEST))
      put(scenario.target, 'export const auditedValue=73;')
      const specifier = path
        .relative(path.join(web, 'src'), path.join(root, scenario.target.slice(0, -3)))
        .split(path.sep)
        .join('/')
      put(
        WEB + '/src/main.ts',
        "import {auditedValue} from '" +
          (specifier.startsWith('.') ? specifier : './' + specifier) +
          "';export {auditedValue};"
      )
      git('add', WEB + '/src/main.ts')
      if (scenario.tracked) git('add', scenario.target)
      await assert.rejects(generateSharedInputs({ root }), new RegExp(scenario.error))
      assert.throws(
        () =>
          execFileSync(process.execPath, [path.join(web, 'scripts/shared-inputs.mjs'), '--write'], {
            cwd: root,
            stdio: 'pipe'
          }),
        (error) => error.status === 1 && error.stderr.toString().includes(scenario.error)
      )
      assert.deepEqual(readFileSync(path.join(root, MANIFEST)), previous)
      put(scenario.target, 'export const auditedValue=99;')
      await assert.rejects(generateSharedInputs({ root }), new RegExp(scenario.error))
      assert.deepEqual(readFileSync(path.join(root, MANIFEST)), previous)
    })
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
