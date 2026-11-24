import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { readdirSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { resolveReviewUrl } from '../src/review/review-url.ts'

// A module of the built Review lives at <app root>/dcl-ui-review/assets/, which is how a relative base finds
// the root.
const MODULE = 'https://cdn.example.com/@dcl/one-ui/1.2.3/dcl-ui-review/assets/index-abc.js'

describe('when mapping a logical Review path onto where the app is served', () => {
  describe('and the base is the root used locally', () => {
    it('should leave the path unchanged', () => {
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
  })

  describe('and the base is an absolute CDN base', () => {
    it('should prefix the path with the base', () => {
      assert.equal(
        resolveReviewUrl('/dcl-ui-review/fidelity.json', '/@scope/pkg/1.2.3/', MODULE),
        '/@scope/pkg/1.2.3/dcl-ui-review/fidelity.json'
      )
    })

    describe('and the base has no trailing slash', () => {
      it('should add the missing trailing slash', () => {
        assert.equal(
          resolveReviewUrl('/dcl-ui-review/?a=1', '/@scope/pkg/1.2.3', MODULE),
          '/@scope/pkg/1.2.3/dcl-ui-review/?a=1'
        )
      })
    })
  })

  for (const base of ['./', '', '.']) {
    describe(`and the base is the relative base "${base}"`, () => {
      it("should resolve the root from the module's own URL", () => {
        assert.equal(
          resolveReviewUrl('/dcl-ui-review/icons/a.svg', base, MODULE),
          'https://cdn.example.com/@dcl/one-ui/1.2.3/dcl-ui-review/icons/a.svg'
        )
      })
    })
  }

  describe('and the path is outside the Review', () => {
    it('should refuse it as not a Review path', () => {
      assert.throws(() => resolveReviewUrl('/other/x.png', '/', MODULE), /Not a Review path/)
    })
  })

  describe('and the path is relative', () => {
    it('should refuse it as not a Review path', () => {
      assert.throws(
        () => resolveReviewUrl('./dcl-ui-review/x.png', '/', MODULE),
        /Not a Review path/
      )
    })
  })
})

// A hard-coded logical path in the Review source would silently break under a prefix or a relative base.
// The file list is read at collection time because it generates one case per file.
describe('when scanning the Review source for logical paths', () => {
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
