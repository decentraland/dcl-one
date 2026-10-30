import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { compareImages, comparePng } from '../driver/compare.ts'
import { decodePng, encodePng, type Image } from '../driver/png.ts'

function noise(width: number, height: number, seed = 1): Image {
  const rgba = new Uint8Array(width * height * 4)
  let x = seed
  for (let i = 0; i < rgba.length; i++) {
    x = (x * 1103515245 + 12345) & 0x7fffffff
    rgba[i] = x >> 16
  }
  return { width, height, rgba }
}

describe('png', () => {
  for (const filter of [0, 1, 2, 3, 4] as const) {
    test(`round-trips through filter ${filter}`, () => {
      const image = noise(13, 7, filter + 1)
      const decoded = decodePng(encodePng(image, filter))
      assert.equal(decoded.width, 13)
      assert.equal(decoded.height, 7)
      assert.deepEqual(Buffer.from(decoded.rgba), Buffer.from(image.rgba))
    })
  }

  test('refuses what is not a PNG', () => {
    assert.throws(() => decodePng(new Uint8Array([1, 2, 3])), /not a PNG/)
  })
})

describe('compare', () => {
  test('an identical frame differs in no pixel', () => {
    const image = noise(20, 10)
    const result = compareImages(image, image)
    assert.equal(result.differing, 0)
    assert.equal(result.ratio, 0)
    assert.equal(result.pass, true)
    assert.equal(decodePng(result.diff).width, 20)
  })

  test('a channel 9 apart counts, 8 apart does not', () => {
    const reference: Image = { width: 4, height: 4, rgba: new Uint8Array(64).fill(100) }
    const near = { ...reference, rgba: Uint8Array.from(reference.rgba) }
    near.rgba[0] = 108
    assert.equal(compareImages(reference, near).differing, 0)
    const far = { ...reference, rgba: Uint8Array.from(reference.rgba) }
    far.rgba[5] = 109
    const result = compareImages(reference, far)
    assert.equal(result.differing, 1)
    assert.equal(result.ratio, 1 / 16)
    assert.equal(result.pass, false)
    assert.equal(compareImages(reference, far, { maxDifferentRatio: 0.1 }).pass, true)
    const diff = decodePng(result.diff)
    assert.deepEqual([...diff.rgba.slice(4, 8)], [255, 0, 0, 255])
    assert.deepEqual([...diff.rgba.slice(0, 4)], [30, 30, 30, 255])
  })

  test('alpha is not compared', () => {
    const reference: Image = { width: 1, height: 1, rgba: new Uint8Array([1, 2, 3, 255]) }
    const other: Image = { width: 1, height: 1, rgba: new Uint8Array([1, 2, 3, 0]) }
    assert.equal(compareImages(reference, other).differing, 0)
  })

  test('a size mismatch is an error', () => {
    assert.throws(() => compareImages(noise(2, 2), noise(3, 2)), /size mismatch: the reference is 2x2, the frame 3x2/)
  })

  test('comparePng decodes both files', () => {
    const a = encodePng(noise(5, 5, 1))
    const b = encodePng(noise(5, 5, 2))
    assert.ok(comparePng(a, b).differing > 0)
    assert.equal(comparePng(a, a).differing, 0)
  })
})
