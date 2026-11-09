import { resolve } from 'path'
import { readFileSync } from 'fs'
import { itExecutes, ensureFileExists, itDeletesFolder } from '../../scripts/helpers'

const sdkCommands = resolve(__dirname, '../../packages/@dcl/sdk-commands/dist/index.js')

describe('build: scene importing @dcl/asset-packs', () => {
  const cwd = resolve(__dirname, './fixtures/ecs7-composite')

  itDeletesFolder('./bin', cwd)
  itExecutes(`node ${sdkCommands} build`, cwd)

  it('bundles the @dcl/smart-items modules in place of the asset-packs ones', () => {
    const binPath = ensureFileExists('bin/game.js', cwd)
    const fileText = readFileSync(binPath, 'utf8')

    expect(fileText).toContain('function initSmartItems(')
    expect(fileText).toContain('function getPlayerPosition(')
    expect(fileText).not.toContain('require("@dcl/asset-packs')
  })
})
