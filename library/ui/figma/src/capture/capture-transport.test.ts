import { beforeEach, describe, expect, it } from 'vitest'
import { checksum, decodeCaptureChunks } from './ingest-chunks.js'

describe('when a complete capture is transported in bounded text parts', () => {
  let capture: { pages: unknown[]; resume: null }
  let parts: { transport: { offset: number; total: number; checksum: string; part: string } }[]

  beforeEach(() => {
    capture = { pages: [], resume: null }
    const raw = JSON.stringify(capture)
    parts = [0, 10].map((offset, index) => ({
      transport: {
        offset,
        total: raw.length,
        checksum: checksum(raw),
        part: raw.slice(offset, index === 0 ? 10 : undefined)
      }
    }))
  })

  it('should reconstruct a byte-identical capture even if input files arrive out of order', () => {
    expect(decodeCaptureChunks([...parts].reverse())).toEqual([capture])
  })

  it('should accept a legacy unsplit capture alongside transported captures', () => {
    expect(decodeCaptureChunks([capture, ...parts])).toEqual([capture, capture])
  })

  it('should reject a missing part', () => {
    expect(() => decodeCaptureChunks(parts.slice(1))).toThrow('missing, duplicate or overlapping')
  })

  it('should reject duplicate parts', () => {
    expect(() => decodeCaptureChunks([...parts, parts[0]])).toThrow(
      'missing, duplicate or overlapping'
    )
  })

  describe('and a part is corrupted', () => {
    beforeEach(() => {
      parts[1]!.transport.part = parts[1]!.transport.part.replace('null', 'true')
    })

    it('should reject a checksum mismatch before ingesting any page', () => {
      expect(() => decodeCaptureChunks(parts)).toThrow('length or checksum mismatch')
    })
  })
})
