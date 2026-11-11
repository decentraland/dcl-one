import { describe, expect, it } from 'vitest'
import { metadataDiff, scopeErrorSummary, type MetadataChange } from './metadata-diagnostics.js'

describe('when catalog metadata diagnostics compare JSON fields', () => {
  it.each<{ name: string; before: unknown; after: unknown; expected: MetadataChange[] }>([
    {
      name: 'unchanged values',
      before: { foundations: { strokeWidths: [1, 2] } },
      after: { foundations: { strokeWidths: [1, 2] } },
      expected: []
    },
    {
      name: 'complete stroke arrays',
      before: { foundations: { strokeWidths: [1, 2] } },
      after: { foundations: { strokeWidths: [1, 3] } },
      expected: [{ field: '/foundations/strokeWidths', before: [1, 2], after: [1, 3] }]
    },
    {
      name: 'escaped pointer keys',
      before: { 'a/b~c': { enabled: true } },
      after: { 'a/b~c': { enabled: false } },
      expected: [{ field: '/a~1b~0c/enabled', before: true, after: false }]
    },
    {
      name: 'added null properties',
      before: {},
      after: { status: null },
      expected: [{ field: '/status', after: null }]
    },
    {
      name: 'removed null properties',
      before: { status: null },
      after: {},
      expected: [{ field: '/status', before: null }]
    },
    {
      name: 'falsy values',
      before: { empty: '', enabled: false, width: 0 },
      after: { empty: null, enabled: 0, width: false },
      expected: [
        { field: '/empty', before: '', after: null },
        { field: '/enabled', before: false, after: 0 },
        { field: '/width', before: 0, after: false }
      ]
    },
    {
      name: 'object ordering differences retained by the guard',
      before: { details: { first: 1, second: 2 } },
      after: { details: { second: 2, first: 1 } },
      expected: [
        { field: '/details', before: { first: 1, second: 2 }, after: { second: 2, first: 1 } }
      ]
    },
    {
      name: 'object to null changes',
      before: { details: { enabled: true } },
      after: { details: null },
      expected: [{ field: '/details', before: { enabled: true }, after: null }]
    },
    {
      name: 'inherited constructor keys',
      before: {},
      after: { constructor: 2 },
      expected: [{ field: '/constructor', after: 2 }]
    },
    {
      name: 'inherited prototype keys',
      before: {},
      after: { ['__proto__']: { enabled: true } },
      expected: [{ field: '/__proto__', after: { enabled: true } }]
    },
    {
      name: 'root values',
      before: null,
      after: [],
      expected: [{ field: '', before: null, after: [] }]
    }
  ])(
    'should preserve exact values for $name after JSON serialization',
    ({ before, after, expected }) => {
      expect(JSON.parse(JSON.stringify(metadataDiff(before, after)))).toEqual(expected)
    }
  )
})

describe('when a scope failure is printed to job logs', () => {
  it('should expose metadata pointers without the baseline or staged values', () => {
    expect(
      scopeErrorSummary([
        {
          path: 'catalog.json',
          kind: 'aggregate-entry-changed',
          owner: { aggregate: 'catalog' },
          cause: 'blocked metadata',
          metadataChanges: [{ field: '/foundations/status', before: 'old', after: 'new' }]
        },
        { path: 'frozen.ts', kind: 'removed', owner: null, cause: 'missing retirement' }
      ])
    ).toEqual([
      {
        path: 'catalog.json',
        kind: 'aggregate-entry-changed',
        owner: { aggregate: 'catalog' },
        cause: 'blocked metadata',
        metadataFields: ['/foundations/status']
      },
      { path: 'frozen.ts', kind: 'removed', owner: null, cause: 'missing retirement' }
    ])
  })
  it('should retain an explicit empty metadata-field list', () => {
    expect(
      scopeErrorSummary([
        {
          path: 'catalog.json',
          kind: 'aggregate-entry-changed',
          owner: null,
          cause: 'blocked metadata',
          metadataChanges: []
        }
      ])
    ).toEqual([
      {
        path: 'catalog.json',
        kind: 'aggregate-entry-changed',
        owner: null,
        cause: 'blocked metadata',
        metadataFields: []
      }
    ])
  })
})
