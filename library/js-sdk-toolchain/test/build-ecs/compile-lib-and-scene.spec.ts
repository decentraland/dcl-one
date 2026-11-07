import { resolve } from 'path'
import { itDeletesFolder, itExecutes, ensureFileExists } from '../../scripts/helpers'

const sdkCommands = resolve(__dirname, '../../packages/@dcl/sdk-commands/dist/index.js')

function buildEcsBuildLibFlow() {
  const cwd = resolve(__dirname, './fixtures/dcl-test-lib-integration')
  describe('build-ecs: build lib', () => {
    itDeletesFolder('./bin', cwd)
    itExecutes('pnpm run build', cwd)

    it('ensure files exist', () => {
      ensureFileExists('bin/index.js', cwd)
      ensureFileExists('bin/index.d.ts', cwd)
    })
  })
}

describe('integration flow, build libs and build scene using libs', () => {
  buildEcsBuildLibFlow()
  const sceneCwd = resolve(__dirname, './fixtures/simple-scene-with-library')

  describe('build-ecs: build scene with library DEBUG mode', () => {
    itDeletesFolder('./bin', sceneCwd)

    itExecutes(`node ${sdkCommands} build`, sceneCwd)

    it('ensure files exist', () => {
      ensureFileExists('bin/game.js', sceneCwd)
    })
  })

  describe('build-ecs: build scene with library, production mode', () => {
    itDeletesFolder('./bin', sceneCwd)

    itExecutes(`node ${sdkCommands} build --production`, sceneCwd)

    it('ensure files exist', () => {
      ensureFileExists('bin/game.js', sceneCwd)
    })
  })
})

describe('integration flow with bundledDependencies, build libs and build scene using libs', () => {
  buildEcsBuildLibFlow()
  const sceneCwd = resolve(__dirname, './fixtures/simple-scene-with-bundled')

  describe('build-ecs: build scene with library DEBUG mode', () => {
    itDeletesFolder('./bin', sceneCwd)

    itExecutes(`node ${sdkCommands} build`, sceneCwd)

    it('ensure files exist', () => {
      ensureFileExists('bin/game.js', sceneCwd)
    })
  })

  describe('build-ecs: build scene with library, production mode', () => {
    itDeletesFolder('./bin', sceneCwd)

    itExecutes(`node ${sdkCommands} build --production`, sceneCwd)

    it('ensure files exist', () => {
      ensureFileExists('bin/game.js', sceneCwd)
    })
  })
})
