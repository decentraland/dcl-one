import * as ui from '@dcl/one-ui'
import * as foundations from '@dcl/one-ui/foundations'
import * as shopUi from '../../apps/shop'
import * as sitesUi from '../../apps/sites'
import { describe, expect, it } from 'vitest'
import { componentExports, coverageProblems } from '../src/coverage'
import { entries, exclusions } from '../src/examples'

// The gallery accounts for shared components. App-scoped compositions have their own consumers.
const components = componentExports(ui, { ...foundations, ...sitesUi, ...shopUi })
const page = (name: string, shown: string[]) => ({
  name,
  components: shown,
  examples: [{ id: 'a', title: 'A', render: () => null }]
})

describe('when the gallery is checked against the public package entry', () => {
  it('should find the component exports of named and star re-exports, and no foundation or helper', () => {
    expect(components).toEqual(
      expect.arrayContaining(['Button', 'NavBar', 'WebSelect', 'WebTextarea', 'EventCardHost'])
    )
    expect(components).not.toContain('CardImage')
    expect(components).not.toContain('GeneralLike')
    expect(components).not.toContain('textClasses')
    expect(components).not.toContain('decentralandNavigation')
  })

  it('should account for every public component export', () => {
    expect(coverageProblems(components, entries, exclusions)).toEqual([])
  })
})

describe('when the coverage rule meets an incomplete gallery', () => {
  it('should fail a component export that no example renders', () => {
    expect(coverageProblems(['Button', 'Unshown'], [page('Button', ['Button'])], [])).toEqual([
      'Unshown: no example renders it and no exclusion explains why'
    ])
  })

  it('should fail an exclusion without a reason', () => {
    expect(coverageProblems(['Hidden'], [], [{ name: 'Hidden', reason: '  ' }])).toEqual([
      'Hidden: the exclusion gives no reason'
    ])
  })

  it('should accept an exclusion with a reason', () => {
    expect(
      coverageProblems(['Hidden'], [], [{ name: 'Hidden', reason: 'Shown in its parent' }])
    ).toEqual([])
  })

  it('should fail stale names, repeated pages, empty pages and double accounting', () => {
    expect(
      coverageProblems(
        ['Button'],
        [
          page('Button', ['Button', 'Gone']),
          page('Button', []),
          { ...page('Empty', []), examples: [] }
        ],
        [
          { name: 'Button', reason: 'duplicate' },
          { name: 'Removed', reason: 'gone' }
        ]
      )
    ).toEqual([
      'Button: lists Gone, which is not a public component export',
      'Button: the page name is repeated',
      'Empty: the page has no example',
      'Button: both excluded and shown',
      'Removed: excluded, but not a public component export'
    ])
  })

  it('should treat memo and forwardRef objects as components and other objects as values', () => {
    const memo = { $$typeof: Symbol.for('react.memo') }
    const forwardRef = { $$typeof: Symbol.for('react.forward_ref') }
    expect(
      componentExports({ Memo: memo, Forward: forwardRef, Plain: {}, lower: () => null }, {})
    ).toEqual(['Forward', 'Memo'])
  })
})
