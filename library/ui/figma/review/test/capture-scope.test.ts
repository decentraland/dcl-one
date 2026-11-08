import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { captureScope, orderedCases, preserveFrozenCases } from '../scripts/capture-scope.mjs'

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

describe('when writing the cases of a capture run', () => {
  const evidence = (sourceId: string) => ({ sourceId })
  // After a run that recaptured only Places, Places sits at the end of fidelity.json.
  const previousOrder = ['Button-0', 'SearchBar-0', 'TabsBar-0', 'PlaceCard-0']

  it('should keep the order of the entries the run did not select', () => {
    // The next run selects Button and Places: frozen entries come first in catalog order, captures after.
    const run = {
      'SearchBar-0': evidence('frozen'),
      'TabsBar-0': evidence('frozen'),
      'Button-0': evidence('new'),
      'PlaceCard-0': evidence('new')
    }
    const unselected = (cases: Record<string, unknown>) =>
      JSON.stringify(
        Object.entries(cases).filter(([key]) => key === 'SearchBar-0' || key === 'TabsBar-0')
      )

    const ordered = orderedCases(run, previousOrder)

    assert.deepEqual(Object.keys(ordered), previousOrder)
    assert.equal(
      unselected(ordered),
      unselected(
        Object.fromEntries(previousOrder.map((key) => [key, run[key as keyof typeof run]]))
      )
    )
  })

  it('should append cases new to this run in capture order, the same way every time', () => {
    const run = {
      'Tooltip-1': evidence('new'),
      'Button-0': evidence('new'),
      'Tooltip-0': evidence('new'),
      'TabsBar-0': evidence('frozen')
    }

    const first = orderedCases(run, previousOrder)
    const second = orderedCases(run, previousOrder)

    assert.deepEqual(Object.keys(first), ['Button-0', 'TabsBar-0', 'Tooltip-1', 'Tooltip-0'])
    assert.equal(JSON.stringify(first), JSON.stringify(second))
  })

  it('should keep capture order when there is no earlier evidence', () => {
    const run = { 'Button-1': evidence('new'), 'Button-0': evidence('new') }

    assert.deepEqual(Object.keys(orderedCases(run)), ['Button-1', 'Button-0'])
  })
})
