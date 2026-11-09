import { componentNumberFromName } from '../../../packages/@dcl/ecs/src/components/component-number'
import {
  LEGACY_COMPONENT_IDS,
  LEGACY_PREFIX,
  PREFIX,
  SCRIPT_COMPONENT,
  SYNC_COMPONENTS,
  isEditorComponent,
  remapLegacyComponentId,
  renameLegacyComponentName,
  renameLegacyComposite
} from '../../../packages/@dcl/sdk-commands/src/logic/smart-items'

const SMART_ITEM_COMPONENTS = [
  'ActionTypes',
  'Actions',
  'Counter',
  'Triggers',
  'States',
  'CounterBar',
  'AdminTools',
  'VideoScreen',
  'Rewards',
  'TextAnnouncements',
  'VideoControlState',
  'Script',
  'Placeholder'
]

const LEGACY_STATES = 2709697014
const STATES = 261723156
const LEGACY_ACTIONS = 2193198513
const ACTIONS = 748857427

function block(name: string, entries: Record<string, unknown> = {}, jsonSchema?: unknown) {
  const data: Record<string, { json: unknown }> = {}
  for (const [entity, json] of Object.entries(entries)) data[entity] = { json }
  return { name, jsonSchema, data }
}

describe('smart-items', () => {
  it('names the script component under the current prefix', () => {
    expect(SCRIPT_COMPONENT).toBe(`${PREFIX}Script`)
  })

  it('detects smart-item components under both prefixes', () => {
    expect(isEditorComponent(`${PREFIX}Actions`)).toBe(true)
    expect(isEditorComponent(`${LEGACY_PREFIX}Actions`)).toBe(true)
    expect(isEditorComponent('core::Transform')).toBe(false)
    expect(isEditorComponent('inspector::Selection')).toBe(false)
  })

  it('renames a legacy component name once', () => {
    expect(renameLegacyComponentName('asset-packs::Triggers')).toBe('smart-items::Triggers')
    expect(renameLegacyComponentName('smart-items::Triggers')).toBe('smart-items::Triggers')
    expect(renameLegacyComponentName('core::Transform')).toBe('core::Transform')
  })

  it('derives the legacy and current number of every smart-item component from its name', () => {
    expect(LEGACY_COMPONENT_IDS.size).toBe(13)
    for (const name of SMART_ITEM_COMPONENTS) {
      expect(LEGACY_COMPONENT_IDS.get(componentNumberFromName(`${LEGACY_PREFIX}${name}`))).toBe(
        componentNumberFromName(`${PREFIX}${name}`)
      )
    }
    expect(LEGACY_COMPONENT_IDS.get(LEGACY_STATES)).toBe(STATES)
    expect(LEGACY_COMPONENT_IDS.get(LEGACY_ACTIONS)).toBe(ACTIONS)
  })

  it('maps a legacy number to the current one and leaves other numbers alone', () => {
    expect(remapLegacyComponentId(LEGACY_STATES)).toBe(STATES)
    expect(remapLegacyComponentId(STATES)).toBe(STATES)
    expect(remapLegacyComponentId(1)).toBe(1)
  })

  it('renames the legacy component names of a composite in place', () => {
    const composite = {
      version: 1,
      components: [
        block('core::Transform'),
        block('asset-packs::Actions'),
        block('smart-items::States'),
        block('asset-packs::Script')
      ]
    }
    expect(renameLegacyComposite(composite)).toBe(composite)
    expect(composite.components.map((c) => c.name)).toEqual([
      'core::Transform',
      'smart-items::Actions',
      'smart-items::States',
      'smart-items::Script'
    ])
  })

  it('merges both spellings of a block at the position of the first, the current spelling winning per entity', () => {
    const composite = {
      version: 1,
      components: [
        block('core::Transform', { '512': { position: 1 } }),
        block('asset-packs::Actions', { '512': 'legacy-512', '513': 'legacy-513' }, { legacy: true }),
        block('core::Name', { '512': { value: 'cube' } }),
        block('smart-items::Actions', { '513': 'current-513', '514': 'current-514' }, { current: true })
      ]
    }

    renameLegacyComposite(composite)

    expect(composite.components.map((c) => c.name)).toEqual(['core::Transform', 'smart-items::Actions', 'core::Name'])
    expect(composite.components[1].jsonSchema).toEqual({ current: true })
    expect(composite.components[1].data).toEqual({
      '512': { json: 'legacy-512' },
      '513': { json: 'current-513' },
      '514': { json: 'current-514' }
    })
  })

  it('takes the jsonSchema from the first block that has one when the winner has none', () => {
    const composite = {
      components: [
        block('asset-packs::States', { '512': 'legacy' }, { legacy: true }),
        block('smart-items::States', { '512': 'current' })
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
        block(SYNC_COMPONENTS, {
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

  it('is idempotent', () => {
    const build = () => ({
      components: [
        block('asset-packs::Actions', { '512': 'legacy-512' }),
        block(SYNC_COMPONENTS, { '512': { componentIds: [LEGACY_STATES, 'smart-items::Counter'] } }),
        block('smart-items::Actions', { '513': 'current-513' })
      ]
    })
    const once = renameLegacyComposite(build())
    expect(renameLegacyComposite(renameLegacyComposite(build()))).toEqual(once)
    expect(renameLegacyComposite(JSON.parse(JSON.stringify(once)))).toEqual(once)
  })

  it('leaves a composite without components or without names alone', () => {
    expect(renameLegacyComposite({ version: 1 })).toEqual({ version: 1 })
    const malformed = { components: [{ data: {} } as unknown as { name: string }] }
    expect(renameLegacyComposite(malformed)).toBe(malformed)
    expect(malformed.components).toHaveLength(1)
  })
})
