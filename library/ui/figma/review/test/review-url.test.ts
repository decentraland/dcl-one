import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { readdirSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { resolveReviewUrl } from '../src/review/review-url.ts'

// A module of the built Review lives at <app root>/dcl-ui-review/assets/, which is how a relative base finds
// the root.
const MODULE = 'https://cdn.example.com/@dcl/one-ui/1.2.3/dcl-ui-review/assets/index-abc.js'

describe('when mapping a logical Review path onto where the app is served', () => {
  it('should leave the path unchanged at the root base used locally', () => {
    assert.equal(
      resolveReviewUrl('/dcl-ui-review/references/Backpack-0.png', '/', MODULE),
      '/dcl-ui-review/references/Backpack-0.png'
    )
  })

  it('should keep the query of a gallery page', () => {
    assert.equal(
      resolveReviewUrl('/dcl-ui-review/?platform=web&component=A%20B', '/', MODULE),
      '/dcl-ui-review/?platform=web&component=A%20B'
    )
  })

  it('should prefix an absolute CDN base', () => {
    assert.equal(
      resolveReviewUrl('/dcl-ui-review/fidelity.json', '/@scope/pkg/1.2.3/', MODULE),
      '/@scope/pkg/1.2.3/dcl-ui-review/fidelity.json'
    )
  })

  it('should add the missing trailing slash of an absolute base', () => {
    assert.equal(
      resolveReviewUrl('/dcl-ui-review/?a=1', '/@scope/pkg/1.2.3', MODULE),
      '/@scope/pkg/1.2.3/dcl-ui-review/?a=1'
    )
  })

  for (const base of ['./', '', '.']) {
    it(`should resolve the root from the module's own URL for the relative base "${base}"`, () => {
      assert.equal(
        resolveReviewUrl('/dcl-ui-review/icons/a.svg', base, MODULE),
        'https://cdn.example.com/@dcl/one-ui/1.2.3/dcl-ui-review/icons/a.svg'
      )
    })
  }

  it('should refuse a path that is not a logical Review path', () => {
    assert.throws(() => resolveReviewUrl('/other/x.png', '/', MODULE), /Not a Review path/)
    assert.throws(() => resolveReviewUrl('./dcl-ui-review/x.png', '/', MODULE), /Not a Review path/)
  })
})

// A hard-coded logical path in the Review source would silently break under a prefix or a relative base.
describe('the Review source', () => {
  const source = path.resolve(import.meta.dirname, '../src/review')
  const files = readdirSync(source).filter(
    (name) => /\.tsx?$/.test(name) && name !== 'review-url.ts'
  )

  it('should hold the Review source files to check', () => {
    assert.ok(files.includes('main.tsx') && files.includes('ReviewNavigation.tsx'))
  })

  for (const name of files)
    it(`should not hard-code a logical /dcl-ui-review/ path in ${name}`, () => {
      assert.doesNotMatch(readFileSync(path.join(source, name), 'utf8'), /['"`]\/dcl-ui-review\//)
    })
})
