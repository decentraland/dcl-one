import { describe, it, expect } from 'vitest'
import { type ContractNetwork } from '@dcl/schemas'
import { type Item } from './items'
import { ItemFileError } from './itemFiles'
import { MappingType } from './mappings'
import { resolveLinkedManifest } from './linkedManifest'

const contract = { network: 'amoy' as ContractNetwork, address: '0x' + '1'.repeat(40) }
const collectionUrn = 'urn:decentraland:amoy:collections-thirdparty:brand:hats'
const target = { collectionUrn, contract }

function errorKey(run: () => unknown): string | undefined {
  try {
    run()
  } catch (error) {
    return error instanceof ItemFileError ? error.messageKey : 'unexpected'
  }
  return undefined
}

describe('resolveLinkedManifest', () => {
  it('takes the URN suffix and the mapping the manifest sets', () => {
    expect(resolveLinkedManifest({ id: `${collectionUrn}:cap`, mapping: { type: 'single', id: '7' } }, target)).toEqual(
      { urnSuffix: 'cap', mapping: { type: MappingType.SINGLE, id: '7' }, keptMappings: null }
    )
  })

  it('defaults to every token and a generated URN when the manifest sets neither', () => {
    expect(resolveLinkedManifest(undefined, target)).toEqual({
      urnSuffix: null,
      mapping: { type: MappingType.ANY },
      keptMappings: null
    })
  })

  it("refuses another collection's URN and an invalid mapping", () => {
    expect(
      errorKey(() => resolveLinkedManifest({ id: 'urn:decentraland:amoy:collections-thirdparty:x:y:z' }, target))
    ).toBe('linked_urn_mismatch')
    expect(errorKey(() => resolveLinkedManifest({ mapping: { type: 'range', from: '9', to: '1' } }, target))).toBe(
      'linked_mapping_invalid'
    )
  })

  it('in bulk edit, needs an existing item and keeps its mapping when the zip sets none', () => {
    const existing = {
      urn: `${collectionUrn}:cap`,
      mappings: { amoy: { [contract.address]: [{ type: 'single', id: '3' }] } }
    } as unknown as Item
    const updating = { ...target, updating: new Map([[existing.urn!, existing]]) }

    expect(errorKey(() => resolveLinkedManifest({}, updating))).toBe('bulk_edit_missing_urn')
    expect(errorKey(() => resolveLinkedManifest({ id: `${collectionUrn}:boots` }, updating))).toBe('bulk_edit_no_match')
    expect(resolveLinkedManifest({ id: existing.urn }, updating).mapping).toEqual({ type: MappingType.SINGLE, id: '3' })
  })
})

describe('resolveLinkedManifest edge cases', () => {
  it('keeps every mapping rule of a bulk-edited item when the zip sets none', () => {
    const mappings = {
      amoy: {
        [contract.address]: [
          { type: 'single', id: '3' },
          { type: 'range', from: '10', to: '20' }
        ]
      }
    }
    const existing = { urn: `${collectionUrn}:cap`, mappings } as unknown as Item
    const resolved = resolveLinkedManifest(
      { id: existing.urn },
      { ...target, updating: new Map([[existing.urn!, existing]]) }
    )
    expect(resolved.keptMappings).toBe(mappings)
  })

  it('ignores a manifest mapping when the collection has no linked contract', () => {
    expect(
      resolveLinkedManifest({ mapping: { type: 'range', from: '9', to: '1' } }, { collectionUrn, contract: null })
        .mapping
    ).toEqual({ type: MappingType.ANY })
  })

  it('refuses a URN whose own segment the server would refuse', () => {
    expect(errorKey(() => resolveLinkedManifest({ id: `${collectionUrn}:bad suffix` }, target))).toBe(
      'linked_urn_invalid'
    )
  })
})
