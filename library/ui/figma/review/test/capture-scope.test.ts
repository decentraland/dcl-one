import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { captureScope, preserveFrozenCases } from '../scripts/capture-scope.mjs'

const first = { name: 'First', page: 'Alpha', variants: [{ id: '10:1' }] }
const second = { name: 'Second', page: 'Beta', variants: [{ id: '10:2' }] }
const catalog = { components: [first, second] }
const config = {
  pages: { '1:1': { id: '1:1', name: 'Alpha' }, '1:2': { id: '1:2', name: 'Beta' } }
}

describe('when limiting offline visual evidence to configured page IDs', () => {
  it('should refresh only the selected page and freeze the other one', () => {
    assert.deepEqual(captureScope(catalog, config, '1:1'), { selected: [first], frozen: [second] })
  })
  it('should accept multiple selected IDs', () => {
    assert.deepEqual(captureScope(catalog, config, '1:1,1:2').selected, [first, second])
  })
  it('should retain a full offline capture when selection is not provided', () => {
    assert.deepEqual(captureScope(catalog, {}, undefined), {
      selected: [first, second],
      frozen: []
    })
  })
  it('should freeze every component for an empty explicit selection', () => {
    assert.deepEqual(captureScope(catalog, config, '').selected, [])
  })
  it('should preserve component evidence when only a configured foundation is selected', () => {
    assert.deepEqual(
      captureScope(
        catalog,
        { pages: { ...config.pages, '1:3': { id: '1:3', name: 'Colors' } } },
        '1:3'
      ),
      { selected: [], frozen: [first, second] }
    )
  })
  it('should reject display names and unknown IDs', () => {
    assert.throws(() => captureScope(catalog, config, 'Alpha'), /Unknown capture page ID/)
  })
  it('should reject ambiguous configured IDs', () => {
    assert.throws(
      () =>
        captureScope(
          catalog,
          { pages: { first: { id: '1:1', name: 'Alpha' }, second: { id: '1:1', name: 'Beta' } } },
          '1:1'
        ),
      /Ambiguous configured page IDs/
    )
  })
  it('should reject canonical names shared by selected and frozen owners', () => {
    assert.throws(
      () =>
        captureScope(
          catalog,
          { pages: { first: { id: '1:1', name: 'Alpha' }, second: { id: '1:2', name: 'Alpha' } } },
          '1:1'
        ),
      /Ambiguous canonical/
    )
  })
  it('should keep an explicit null identity unresolved', () => {
    assert.throws(
      () => captureScope(catalog, { pages: { '1:1': { id: null, name: 'Alpha' } } }, '1:1'),
      /Unknown capture page ID/
    )
  })
  it('should preserve the original frozen evidence values, signed by the capture they came from', () => {
    const evidence = { sourceId: '10:2', image: '/render.png', differentPixels: 4 }
    assert.deepEqual(
      preserveFrozenCases([second], { signature: 'earlier', cases: { 'Second-0': evidence } }),
      { 'Second-0': { ...evidence, signature: 'earlier' } }
    )
  })
  it('should mark evidence from an unsigned report so that it can never match a signature', () => {
    const kept = preserveFrozenCases([second], { cases: { 'Second-0': { sourceId: '10:2' } } })
    assert.equal((kept['Second-0'] as { signature: string }).signature, 'unsigned')
  })
  for (const previous of [
    undefined,
    { cases: {} },
    { cases: { 'Second-0': { sourceId: 'replaced' } } }
  ])
    it('should reject incomplete frozen evidence', () => {
      assert.throws(() => preserveFrozenCases([second], previous), /Missing frozen visual evidence/)
    })
})
