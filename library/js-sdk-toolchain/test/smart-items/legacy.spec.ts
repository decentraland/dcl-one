import { Schemas } from '../../packages/@dcl/ecs/src'
import { componentNumberFromName } from '../../packages/@dcl/ecs/src/components/component-number'
import { ReadWriteByteBuffer } from '../../packages/@dcl/ecs/src/serialization/ByteBuffer'
import { BaseComponentNames } from '../../packages/@dcl/smart-items/src/constants'
import {
  LEGACY_COMPONENT_IDS,
  LEGACY_PREFIX,
  PREFIX,
  SYNC_COMPONENTS,
  remapLegacyComponentId,
  renameLegacyComponentName,
  renameLegacyComposite,
  renameLegacyPlaceholder
} from '../../packages/@dcl/smart-items/src/legacy'

const SYNC_SCHEMA = Schemas.Map({ componentIds: Schemas.Array(Schemas.Int64) })

function syncBytes(componentIds: number[]): Uint8Array {
  const buffer = new ReadWriteByteBuffer()
  SYNC_SCHEMA.serialize({ componentIds }, buffer)
  return buffer.toBinary()
}

function syncIds(bytes: Uint8Array): number[] {
  return SYNC_SCHEMA.deserialize(new ReadWriteByteBuffer(bytes)).componentIds
}

// legacy -> current component number of every smart-item component (CRC32 of the name + 2048)
const SMART_ITEM_COMPONENT_NUMBERS: Record<string, [number, number]> = {
  ActionTypes: [1429051521, 4214668643],
  Actions: [2193198513, 748857427],
  Counter: [3981387903, 1129581981],
  Triggers: [585272453, 2365172071],
  States: [2709697014, 261723156],
  CounterBar: [574670431, 2354815933],
  AdminTools: [99929642, 2884627400],
  VideoScreen: [2597742855, 885869797],
  Rewards: [993215415, 2502436437],
  TextAnnouncements: [760563130, 2202935384],
  VideoControlState: [2092194694, 3534716516],
  Script: [4149214247, 1498732997],
  Placeholder: [3776766571, 1325926281]
}

const [LEGACY_STATES, STATES] = SMART_ITEM_COMPONENT_NUMBERS.States
const [LEGACY_ACTIONS, ACTIONS] = SMART_ITEM_COMPONENT_NUMBERS.Actions

type JsonEntry = { json: unknown }
type DefinitionEntry = { data: { $case: 'json'; json: unknown } }

function jsonBlock(name: string, entries: Record<string, unknown>, jsonSchema?: unknown) {
  const data: Record<string, JsonEntry> = {}
  for (const [entity, json] of Object.entries(entries)) data[entity] = { json }
  return { name, jsonSchema, data }
}

function definitionBlock(name: string, entries: Record<string, unknown>, jsonSchema?: unknown) {
  const data = new Map<number, DefinitionEntry>()
  for (const [entity, json] of Object.entries(entries)) data.set(Number(entity), { data: { $case: 'json', json } })
  return { name, jsonSchema, data }
}

describe('smart-items legacy component numbers', () => {
  it('derives the legacy and current number of every smart-item component from its name', () => {
    expect(LEGACY_COMPONENT_IDS.size).toBe(13)
    expect(Object.values(BaseComponentNames)).toHaveLength(13)
    for (const [name, [legacy, current]] of Object.entries(SMART_ITEM_COMPONENT_NUMBERS)) {
      expect(componentNumberFromName(`${LEGACY_PREFIX}${name}`)).toBe(legacy)
      expect(componentNumberFromName(`${PREFIX}${name}`)).toBe(current)
      expect(LEGACY_COMPONENT_IDS.get(legacy)).toBe(current)
    }
  })

  it('maps a legacy number to the current one and leaves other numbers alone', () => {
    expect(remapLegacyComponentId(LEGACY_STATES)).toBe(STATES)
    expect(remapLegacyComponentId(STATES)).toBe(STATES)
    expect(remapLegacyComponentId(1)).toBe(1)
    expect(remapLegacyComponentId(999999)).toBe(999999)
  })

  it('renames a legacy component name once', () => {
    expect(renameLegacyComponentName('asset-packs::Triggers')).toBe('smart-items::Triggers')
    expect(renameLegacyComponentName('smart-items::Triggers')).toBe('smart-items::Triggers')
    expect(renameLegacyComponentName('core::Transform')).toBe('core::Transform')
  })

  it('renames the component a trigger placeholder names and leaves other strings alone', () => {
    expect(renameLegacyPlaceholder('{self:asset-packs::Actions}')).toBe('{self:smart-items::Actions}')
    expect(renameLegacyPlaceholder('{512:asset-packs::States}')).toBe('{512:smart-items::States}')
    expect(renameLegacyPlaceholder('{self:smart-items::Actions}')).toBe('{self:smart-items::Actions}')
    expect(renameLegacyPlaceholder('{self}')).toBe('{self}')
    expect(renameLegacyPlaceholder('asset-packs::Actions')).toBe('asset-packs::Actions')
  })
})

describe('renameLegacyComposite', () => {
  it('renames the legacy blocks of composite JSON in place', () => {
    const composite = {
      version: 1,
      components: [
        jsonBlock('core::Transform', {}),
        jsonBlock('asset-packs::Actions', {}),
        jsonBlock('smart-items::States', {})
      ]
    }
    expect(renameLegacyComposite(composite)).toBe(composite)
    expect(composite.components.map((c) => c.name)).toEqual([
      'core::Transform',
      'smart-items::Actions',
      'smart-items::States'
    ])
  })

  it('merges both spellings of a block at the position of the first, the current spelling winning per entity', () => {
    const composite = {
      version: 1,
      components: [
        jsonBlock('core::Transform', { '512': { position: 1 } }),
        jsonBlock('asset-packs::Actions', { '512': 'legacy-512', '513': 'legacy-513' }, { legacy: true }),
        jsonBlock('core::Name', { '512': { value: 'cube' } }),
        jsonBlock('smart-items::Actions', { '513': 'current-513', '514': 'current-514' }, { current: true })
      ]
    }

    renameLegacyComposite(composite)

    expect(composite.components.map((c) => c.name)).toEqual(['core::Transform', 'smart-items::Actions', 'core::Name'])
    const actions = composite.components[1]
    expect(actions.jsonSchema).toEqual({ current: true })
    expect(actions.data).toEqual({
      '512': { json: 'legacy-512' },
      '513': { json: 'current-513' },
      '514': { json: 'current-514' }
    })
  })

  it('keeps the first block of a spelling when a name repeats under the same spelling', () => {
    const composite = {
      components: [
        jsonBlock('smart-items::Counter', { '512': 'first' }),
        jsonBlock('smart-items::Counter', { '512': 'second', '513': 'second-only' }),
        jsonBlock('asset-packs::Counter', { '512': 'legacy', '514': 'legacy-only' }, { legacy: true })
      ]
    }

    renameLegacyComposite(composite)

    expect(composite.components).toHaveLength(1)
    expect(composite.components[0].jsonSchema).toEqual({ legacy: true })
    expect(composite.components[0].data).toEqual({
      '512': { json: 'first' },
      '513': { json: 'second-only' },
      '514': { json: 'legacy-only' }
    })
  })

  it('takes the jsonSchema from the first block that has one when the winner has none', () => {
    const composite = {
      components: [
        jsonBlock('asset-packs::States', { '512': 'legacy' }, { legacy: true }),
        jsonBlock('smart-items::States', { '512': 'current' })
      ]
    }

    renameLegacyComposite(composite)

    expect(composite.components).toHaveLength(1)
    expect(composite.components[0].jsonSchema).toEqual({ legacy: true })
    expect(composite.components[0].data).toEqual({ '512': { json: 'current' } })
  })

  it('rewrites the legacy numbers of Sync-Components and leaves strings and other numbers alone', () => {
    const composite = {
      components: [
        jsonBlock(SYNC_COMPONENTS, {
          '512': { componentIds: [LEGACY_STATES, 'smart-items::Counter', 42, 'asset-packs::Actions', LEGACY_ACTIONS] },
          '513': { componentIds: [STATES] },
          '514': { other: true }
        })
      ]
    }

    renameLegacyComposite(composite)

    expect(composite.components[0].data).toEqual({
      '512': { json: { componentIds: [STATES, 'smart-items::Counter', 42, 'asset-packs::Actions', ACTIONS] } },
      '513': { json: { componentIds: [STATES] } },
      '514': { json: { other: true } }
    })
  })

  it('handles a Composite.Definition with entity data in a Map', () => {
    const composite = {
      version: 1,
      components: [
        definitionBlock('asset-packs::Actions', { '512': 'legacy-512', '513': 'legacy-513' }),
        definitionBlock(SYNC_COMPONENTS, { '512': { componentIds: [LEGACY_STATES, 7] } }),
        definitionBlock('smart-items::Actions', { '513': 'current-513' }, { current: true })
      ]
    }

    renameLegacyComposite(composite)

    expect(composite.components.map((c) => c.name)).toEqual(['smart-items::Actions', SYNC_COMPONENTS])
    const actions = composite.components[0]
    expect(actions.jsonSchema).toEqual({ current: true })
    expect([...actions.data.entries()]).toEqual([
      [512, { data: { $case: 'json', json: 'legacy-512' } }],
      [513, { data: { $case: 'json', json: 'current-513' } }]
    ])
    expect(composite.components[1].data.get(512)).toEqual({
      data: { $case: 'json', json: { componentIds: [STATES, 7] } }
    })
  })

  it('is idempotent', () => {
    const build = () => ({
      components: [
        jsonBlock('asset-packs::Actions', { '512': 'legacy-512' }),
        jsonBlock(SYNC_COMPONENTS, { '512': { componentIds: [LEGACY_STATES, 'smart-items::Counter'] } }),
        jsonBlock('smart-items::Actions', { '513': 'current-513' })
      ]
    })
    const once = renameLegacyComposite(build())
    const twice = renameLegacyComposite(renameLegacyComposite(build()))
    expect(twice).toEqual(once)
    expect(renameLegacyComposite(JSON.parse(JSON.stringify(once)))).toEqual(once)
  })

  it('leaves a composite without components alone', () => {
    expect(renameLegacyComposite({ version: 1 })).toEqual({ version: 1 })
  })
})

describe('renameLegacyComposite on trigger placeholders', () => {
  // the Health Bar item of the published @dcl/asset-packs 2.26.0 catalog
  const healthBarTriggers = [
    { type: 'on_damage', actions: [{ id: '{self:asset-packs::Actions}', name: 'Damage' }] },
    { type: 'on_heal_player', actions: [{ id: '{self:asset-packs::Actions}', name: 'Heal' }] }
  ]
  // the Video Player item: conditions reference the item's States
  const videoPlayerTrigger = {
    type: 'on_state_change',
    conditions: [{ id: '{self:asset-packs::States}', type: 'when_state_is', value: 'Playing' }],
    actions: [{ id: '{self:asset-packs::Actions}', name: 'Play Video Stream' }],
    basicViewId: 'trigger-when-turned-on'
  }

  it('renames the Actions and States the catalog triggers reference', () => {
    const composite = {
      components: [jsonBlock('asset-packs::Triggers', { '0': { value: [...healthBarTriggers, videoPlayerTrigger] } })]
    }

    renameLegacyComposite(composite)

    expect(composite.components[0].name).toBe('smart-items::Triggers')
    const triggers = (composite.components[0].data['0'].json as { value: any[] }).value
    expect(triggers[0].actions[0]).toEqual({ id: '{self:smart-items::Actions}', name: 'Damage' })
    expect(triggers[1].actions[0]).toEqual({ id: '{self:smart-items::Actions}', name: 'Heal' })
    expect(triggers[2].conditions[0]).toEqual({ id: '{self:smart-items::States}', type: 'when_state_is', value: 'Playing' })
    expect(triggers[2].actions[0]).toEqual({ id: '{self:smart-items::Actions}', name: 'Play Video Stream' })
    expect(triggers[2].basicViewId).toBe('trigger-when-turned-on')
  })

  it('renames cross-entity placeholders and keeps resolved numeric ids and triggers without a payload', () => {
    const composite = {
      components: [
        definitionBlock('asset-packs::Triggers', {
          '512': { value: [{ type: 'on_click', conditions: [{ id: '{513:asset-packs::States}' }], actions: [{ id: 7 }] }] },
          '513': 'not an object'
        })
      ]
    }

    renameLegacyComposite(composite)

    expect(composite.components[0].data.get(512)).toEqual({
      data: {
        $case: 'json',
        json: { value: [{ type: 'on_click', conditions: [{ id: '{513:smart-items::States}' }], actions: [{ id: 7 }] }] }
      }
    })
    expect(composite.components[0].data.get(513)).toEqual({ data: { $case: 'json', json: 'not an object' } })
  })

  it('leaves the placeholders of a block that already carries the current name untouched', () => {
    const composite = {
      components: [jsonBlock('smart-items::Triggers', { '0': { value: [{ actions: [{ id: '{self:smart-items::Actions}' }] }] } })]
    }
    const before = JSON.stringify(composite)
    renameLegacyComposite(composite)
    expect(JSON.stringify(composite)).toBe(before)
  })
})

describe('renameLegacyComposite on binary Sync-Components entries', () => {
  it('rewrites the legacy numbers inside the bytes of a Composite.Definition entry', () => {
    const data = new Map<number, unknown>([
      [512, { data: { $case: 'binary', binary: syncBytes([LEGACY_STATES, 42, LEGACY_ACTIONS]) } }],
      [513, { data: { $case: 'binary', binary: undefined } }]
    ])
    const composite = { version: 1, components: [{ name: SYNC_COMPONENTS, data }] }

    renameLegacyComposite(composite)

    const entry = data.get(512) as { data: { $case: string; binary: Uint8Array } }
    expect(entry.data.$case).toBe('binary')
    expect(syncIds(entry.data.binary)).toEqual([STATES, 42, ACTIONS])
    expect(data.get(513)).toEqual({ data: { $case: 'binary', binary: undefined } })
  })

  it('rewrites the legacy numbers inside the base64 entry of a composite JSON file', () => {
    const composite = {
      components: [
        {
          name: SYNC_COMPONENTS,
          data: { '512': { binary: Buffer.from(syncBytes([LEGACY_STATES, LEGACY_ACTIONS, 3])).toString('base64') } }
        }
      ]
    }

    renameLegacyComposite(composite)

    const encoded = (composite.components[0].data as Record<string, { binary: string }>)['512'].binary
    expect(syncIds(new Uint8Array(Buffer.from(encoded, 'base64')))).toEqual([STATES, ACTIONS, 3])
    expect(encoded).toBe(Buffer.from(syncBytes([STATES, ACTIONS, 3])).toString('base64'))
  })

  it('round-trips base64 of every byte length', () => {
    for (const length of [0, 1, 2, 3, 4, 5]) {
      const ids = Array.from({ length }, (_, i) => i + 1)
      const bytes = syncBytes(ids)
      const composite = {
        components: [{ name: SYNC_COMPONENTS, data: { '0': { binary: Buffer.from(bytes).toString('base64') } } }]
      }
      renameLegacyComposite(composite)
      expect((composite.components[0].data as any)['0'].binary).toBe(Buffer.from(bytes).toString('base64'))
    }
  })
})

describe('renameLegacyComposite on hostile entity keys', () => {
  it('ignores entity keys that are not numbers instead of writing them onto the merged block', () => {
    const legacy = JSON.parse(
      '{"name":"asset-packs::Actions","data":{"__proto__":{"json":"polluted"},"constructor":{"json":1},"512":{"json":"legacy-512"}}}'
    )
    const current = JSON.parse('{"name":"smart-items::Actions","data":{"513":{"json":"current-513"}}}')
    const composite = { components: [legacy, current] }

    renameLegacyComposite(composite)

    expect(composite.components).toHaveLength(1)
    expect(Object.keys(composite.components[0].data)).toEqual(['512', '513'])
    expect(({} as any).json).toBeUndefined()
    expect(Object.getPrototypeOf(composite.components[0].data)).toBe(Object.prototype)
  })
})

