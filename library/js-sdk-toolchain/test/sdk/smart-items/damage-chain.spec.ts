// `actions.ts` resolves call_script_method through the script-instance registry in `./scripts`;
// the fake registry lets the spec register real instances the way `runScripts` would.
jest.mock('../../../packages/@dcl/sdk/src/smart-items/scripts', () => jest.requireActual('./script-utils-mock'))

import { engine, getComponentEntityTree, Transform } from '@dcl/sdk/ecs'
import type { Entity } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'
import { createInputSystem, createPointerEventsSystem, createTweenSystem } from '../../../packages/@dcl/ecs/dist'

import { createComponents, getComponents, TriggerType } from '../../../packages/@dcl/sdk/src/smart-items/definitions'
import { getExplorerComponents } from '../../../packages/@dcl/sdk/src/smart-items/components'
import { getTriggerEvents } from '../../../packages/@dcl/sdk/src/smart-items/events'
import { getWorldPosition, getEntityParent } from '../../../packages/@dcl/sdk/src/smart-items/helpers'
import { createActionsSystem } from '../../../packages/@dcl/sdk/src/smart-items/actions'
import { createTriggersSystem, damageTargets } from '../../../packages/@dcl/sdk/src/smart-items/triggers'
import * as scriptRegistry from '../../../packages/@dcl/sdk/src/smart-items/scripts'

const { __register, __clear } = scriptRegistry as unknown as typeof import('./script-utils-mock')

// End-to-end guard for the migrated combat: a dealer (e.g. Spikes) reaches a Health Bar purely
// through the shared `damageTargets` registry + the on_damage trigger -> call_script_method chain.
// This is exactly what a scene runs, and it uses ONLY the primitives the smart-items runtime
// already ships (no dist/combat module), which is what lets the migrated dealer scripts work against
// the shipping toolchain. It also reproduces the reported "spikes don't reduce the Health Bar" bug.

createComponents(engine)
const components = getExplorerComponents(engine)
const inputSystem = createInputSystem(engine)
const pointerEventsSystem = createPointerEventsSystem(engine, inputSystem)
const tweenSystem = createTweenSystem(engine)
const actionsSystem = createActionsSystem(engine, pointerEventsSystem, undefined, undefined)
const triggersSystem = createTriggersSystem(engine, components, pointerEventsSystem, tweenSystem)

const HB_PATH = 'assets/smart-items/health_bar/HealthBar.ts'

function tick(dt = 0.1) {
  // Same order initSmartItems adds them: actions first (binds action listeners), then triggers
  // (drains the action queue emitted last tick, then inits/binds trigger listeners).
  actionsSystem(dt)
  triggersSystem(dt)
}

// The exact damage routine the rewritten dealer scripts inline (Spikes/Sword/Robot/Barrel): scan the
// shared registry, honor the proximity target, and emit ON_DAMAGE `hits` times to each target in range.
function dealDamage(from: Entity, radius: number, hits: number, target: 'all' | 'player' | 'non_player') {
  const origin = getWorldPosition(from)
  const ownTree = new Set(getComponentEntityTree(engine, from, Transform))
  for (const entity of damageTargets) {
    if (ownTree.has(entity)) continue
    if (target !== 'all') {
      let root = entity
      for (let parent = getEntityParent(root); parent; parent = getEntityParent(root)) root = parent
      const isPlayer = root === engine.PlayerEntity || root === engine.CameraEntity
      if (target === 'player' && !isPlayer) continue
      if (target === 'non_player' && isPlayer) continue
    }
    if (Vector3.distance(origin, getWorldPosition(entity)) <= Math.max(radius, 0)) {
      for (let i = 0; i < Math.max(hits, 1); i++) getTriggerEvents(entity).emit(TriggerType.ON_DAMAGE)
    }
  }
}

afterEach(() => {
  damageTargets.clear()
  __clear()
})

describe('migrated combat: dealer -> Health Bar', () => {
  it('a dealer within range reduces the Health Bar Counter via on_damage -> call_script_method', () => {
    const { Actions, Triggers, Counter } = getComponents(engine)

    // Health bar entity, parented to the player (as HealthBar.start() does), registered via on_damage.
    const hb = engine.addEntity()
    Transform.create(hb, {
      position: Vector3.create(0, 2.1, 0),
      parent: engine.PlayerEntity as never
    })
    Counter.create(hb, { id: hb as never, value: 100 } as never)
    Actions.create(hb, {
      id: hb as never,
      value: [
        {
          name: 'Damage',
          type: 'call_script_method',
          jsonPayload: JSON.stringify({ scriptPath: HB_PATH, methodName: 'damage', params: {} })
        }
      ]
    } as never)
    Triggers.create(hb, {
      value: [{ type: 'on_damage', actions: [{ id: hb as never, name: 'Damage' }] }]
    } as never)

    // The script instance runScripts would have registered under `${entity}:${path}`.
    __register(hb, HB_PATH, {
      damage(amount = 1) {
        const counter = Counter.getMutable(hb)
        counter.value = Math.max(0, counter.value - (amount as number))
      }
    })

    // Player + dealer both at the origin (player standing on the dealer).
    Transform.createOrReplace(engine.PlayerEntity, { position: Vector3.create(0, 0, 0) })
    const dealer = engine.addEntity()
    Transform.create(dealer, { position: Vector3.create(0, 0, 0) })

    // First tick: bind action listeners + init the trigger (registers hb in damageTargets, binds on_damage).
    tick()
    expect(damageTargets.has(hb)).toBe(true)

    // Dealer deals a tick of damage: 15 hits, player layer.
    dealDamage(dealer, 5, 15, 'player')

    // Next tick drains the enqueued Damage actions -> call_script_method -> HealthBar.damage().
    tick()

    expect(Counter.get(hb).value).toBe(85)
  })

  it('does not reduce a Health Bar out of range', () => {
    const { Actions, Triggers, Counter } = getComponents(engine)

    const hb = engine.addEntity()
    Transform.create(hb, {
      position: Vector3.create(0, 0, 0),
      parent: engine.PlayerEntity as never
    })
    Counter.create(hb, { id: hb as never, value: 100 } as never)
    Actions.create(hb, {
      id: hb as never,
      value: [
        {
          name: 'Damage',
          type: 'call_script_method',
          jsonPayload: JSON.stringify({ scriptPath: HB_PATH, methodName: 'damage', params: {} })
        }
      ]
    } as never)
    Triggers.create(hb, {
      value: [{ type: 'on_damage', actions: [{ id: hb as never, name: 'Damage' }] }]
    } as never)
    __register(hb, HB_PATH, {
      damage(amount = 1) {
        const counter = Counter.getMutable(hb)
        counter.value = Math.max(0, counter.value - (amount as number))
      }
    })

    // Player far from the dealer.
    Transform.createOrReplace(engine.PlayerEntity, { position: Vector3.create(100, 0, 100) })
    const dealer = engine.addEntity()
    Transform.create(dealer, { position: Vector3.create(0, 0, 0) })

    tick()
    dealDamage(dealer, 5, 15, 'player')
    tick()

    expect(Counter.get(hb).value).toBe(100)
  })
})
