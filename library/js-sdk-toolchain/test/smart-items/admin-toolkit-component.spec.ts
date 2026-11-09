// The video helpers live beside the panel UI and its comms bus, which need the scene runtime; the
// helpers under test use neither.
jest.mock('../../packages/@dcl/smart-items/src/admin-toolkit-ui/admin-message-bus', () => ({
  getAdminMessageBus: jest.fn()
}))
jest.mock('../../packages/@dcl/smart-items/src/admin-toolkit-ui/VideoControl/index', () => ({
  DEFAULT_VOLUME: 1
}))

import { Engine } from '../../packages/@dcl/ecs/dist'
import type { Entity } from '../../packages/@dcl/ecs/dist'
import { createComponents, getComponents } from '../../packages/@dcl/smart-items/src/definitions'
import {
  getAdminToolkitVideoControl,
  getVideoPlayers
} from '../../packages/@dcl/smart-items/src/admin-toolkit-ui/VideoControl/utils'
import { getAdminConfig, getAdminEntityOrNull } from '../../packages/@dcl/smart-items/src/admin-toolkit-ui/config'

// The Admin Tools item ships as a `smart-items::AdminTools` component. The admin panel reads its
// settings through the config funnel, which must keep returning exactly what is on that component
// when no script has injected a config.

describe('when a scene has the component-based Admin Tools item', () => {
  const engine = Engine()
  createComponents(engine)
  const { AdminTools } = getComponents(engine)
  const entity = engine.addEntity()
  const screen = 600 as Entity
  AdminTools.create(entity, {
    videoControl: {
      isEnabled: true,
      disableVideoPlayersSound: true,
      showAuthorOnVideoPlayers: false,
      linkAllVideoPlayers: false,
      videoPlayers: [{ entity: screen, customName: 'Main screen' }]
    }
  } as never)

  it('should read the settings off that component', () => {
    expect(getAdminConfig(engine)).toEqual(AdminTools.get(entity))
  })

  it("should treat the component's entity as the admin entity", () => {
    expect(getAdminEntityOrNull(engine)).toBe(entity)
  })

  it('should read the video settings off that component', () => {
    expect(getAdminToolkitVideoControl(engine)).toEqual(AdminTools.get(entity).videoControl)
  })

  it('should list the video screens the component names', () => {
    expect(getVideoPlayers(engine)).toEqual([{ entity: screen, customName: 'Main screen' }])
  })
})

describe('when a scene has no Admin Tools item', () => {
  const engine = Engine()
  createComponents(engine)

  it('should have no video settings or screens', () => {
    expect(getAdminToolkitVideoControl(engine)).toBeNull()
    expect(getVideoPlayers(engine)).toEqual([])
  })
})
