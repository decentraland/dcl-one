import { initComponents } from '../../../packages/@dcl/sdk-commands/src/components'
import { runSdkCommand } from '../../../packages/@dcl/sdk-commands/src/run-command'
import { mkdirSync, readFileSync, rmSync, symlinkSync } from 'fs'
import path from 'path'

// The downloaded template pins the registry `@dcl/sdk`; the scene is built with this tree's packages
// instead, linked the way pnpm links the workspace fixtures, so `init` and this tree's bundler are
// covered together, the template's `baseUrl` rewrite for TypeScript 7 included
const TREE_PACKAGES = ['sdk', 'sdk-commands', 'ecs', 'react-ecs', 'js-runtime']

describe('blackbox: init', () => {
  test('init command', async () => {
    const components = await initComponents()
    const dir = 'tmp/blackbox-scene'

    rmSync(dir, { recursive: true, force: true })
    await components.fs.mkdir(dir, { recursive: true })

    await runSdkCommand(components, 'init', [`--dir=${dir}`, '--skip-install'])

    const scope = path.join(dir, 'node_modules', '@dcl')
    mkdirSync(scope, { recursive: true })
    for (const name of TREE_PACKAGES) {
      symlinkSync(path.resolve('packages', '@dcl', name), path.join(scope, name), 'dir')
    }

    await runSdkCommand(components, 'build', [`--dir=${dir}`, '--skip-install', '--customEntryPoint'])

    const tsconfig = JSON.parse(readFileSync(path.join(dir, 'tsconfig.json'), 'utf-8'))
    expect(tsconfig.compilerOptions.baseUrl).toBeUndefined()
    expect(tsconfig.compilerOptions.paths['*']).toEqual(['./*'])
    for (const entries of Object.values<string[]>(tsconfig.compilerOptions.paths)) {
      for (const entry of entries) expect(entry).toMatch(/^\.\.?\//)
    }
  }, 180000) // the template download can be slow when running with other tests
})
