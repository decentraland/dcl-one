import { spawn } from 'child_process'
import { rmSync } from 'fs'
import { initComponents } from '../../../packages/@dcl/sdk-commands/src/components'
import { createProcessSpawnerComponent } from '../../../packages/@dcl/sdk-commands/src/logic/exec'
import { runSdkCommand } from '../../../packages/@dcl/sdk-commands/src/run-command'
import { createInitDiagnostics, getInitSceneDirectory } from './init-diagnostics'

describe('blackbox: init', () => {
  let diagnostics: ReturnType<typeof createInitDiagnostics>

  afterEach(() => diagnostics?.reportPending())

  test('init command', async () => {
    diagnostics = createInitDiagnostics((receipt: string) => console.error(`[sdk-init] ${receipt}`))
    const components = await diagnostics.step('components', initComponents)
    components.fetch = diagnostics.wrapFetch(components.fetch)
    components.fs = diagnostics.wrapFs(components.fs)
    const spawner = createProcessSpawnerComponent(diagnostics.wrapSpawn(spawn))
    components.spawner = {
      exec: (...args) => diagnostics.step('dependency-install', () => spawner.exec(...args))
    }

    const sceneDirectory = getInitSceneDirectory(process.env.SDK_INIT_FIXTURE)
    await diagnostics.step('fixture', async () => {
      rmSync(sceneDirectory, { recursive: true, force: true })
      await components.fs.mkdir(sceneDirectory, { recursive: true })
    })

    await diagnostics.step('init', () => runSdkCommand(components, 'init', [`--dir=${sceneDirectory}`]))
    await diagnostics.step('build', () =>
      runSdkCommand(components, 'build', [`--dir=${sceneDirectory}`, '--customEntryPoint'])
    )
  }, 180000) // The budget covers both commands, including dependency installation.
})
