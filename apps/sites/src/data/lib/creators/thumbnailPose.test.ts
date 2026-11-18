import { describe, expect, it } from 'vitest'
import { getThumbnailPose, isAutoThumbnailStale } from './thumbnailPose'

describe('getThumbnailPose', () => {
  it('poses upper body and skin wearables, nothing else', () => {
    expect(getThumbnailPose('upper_body')).toBeTruthy()
    expect(getThumbnailPose('skin')).toBe(getThumbnailPose('upper_body'))
    expect(getThumbnailPose('hat')).toBeNull()
    expect(getThumbnailPose(null)).toBeNull()
  })
})

describe('isAutoThumbnailStale', () => {
  it('flags an auto thumbnail once the category switches between posed and plain', () => {
    expect(isAutoThumbnailStale({ isAutoThumbnail: true, autoThumbnailCategory: 'hat', category: 'upper_body' })).toBe(
      true
    )
    expect(isAutoThumbnailStale({ isAutoThumbnail: true, autoThumbnailCategory: 'upper_body', category: 'hat' })).toBe(
      true
    )
  })

  it('keeps thumbnails whose pose did not change', () => {
    expect(isAutoThumbnailStale({ isAutoThumbnail: true, autoThumbnailCategory: 'hat', category: 'feet' })).toBe(false)
    expect(isAutoThumbnailStale({ isAutoThumbnail: true, autoThumbnailCategory: 'skin', category: 'upper_body' })).toBe(
      false
    )
  })

  it('never regenerates a zip-provided or user-picked thumbnail', () => {
    expect(isAutoThumbnailStale({ isAutoThumbnail: false, autoThumbnailCategory: null, category: 'upper_body' })).toBe(
      false
    )
  })
})
