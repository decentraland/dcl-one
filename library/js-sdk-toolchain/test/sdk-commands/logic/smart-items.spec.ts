import {
  LEGACY_PREFIX,
  PREFIX,
  SCRIPT_COMPONENT,
  isEditorComponent,
  renameLegacyComponentName,
  renameLegacyComposite
} from '../../../packages/@dcl/sdk-commands/src/logic/smart-items'

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

  it('renames the legacy component names of a composite in place', () => {
    const composite = {
      version: 1,
      components: [
        { name: 'core::Transform', data: {} },
        { name: 'asset-packs::Actions', data: {} },
        { name: 'smart-items::States', data: {} },
        { name: 'asset-packs::Script', data: {} }
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

  it('leaves a composite without components or without names alone', () => {
    expect(renameLegacyComposite({ version: 1 })).toEqual({ version: 1 })
    const malformed = { components: [{ data: {} } as unknown as { name: string }] }
    expect(renameLegacyComposite(malformed)).toBe(malformed)
  })
})
