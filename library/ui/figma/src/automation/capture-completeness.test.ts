import { beforeEach, describe, expect, it } from 'vitest'
import type { RawComponentPage } from '../ir/raw-components.js'
import type { LiveNode } from './live-file.js'
import { capturePage } from './rest-capture.js'
import { assertCompleteCapture } from './capture-completeness.js'

describe('when checking a replacement capture', () => {
  let previous: RawComponentPage
  let current: LiveNode
  beforeEach(() => {
    current = {
      id: '1:1',
      name: 'Page',
      type: 'CANVAS',
      children: [
        {
          id: '2:1',
          name: 'Control',
          type: 'COMPONENT_SET',
          children: [
            {
              id: '3:1',
              name: 'State=Default',
              type: 'COMPONENT',
              absoluteBoundingBox: { x: 0, y: 0, width: 20, height: 20 }
            },
            {
              id: '3:2',
              name: 'State=Hover',
              type: 'COMPONENT',
              absoluteBoundingBox: { x: 0, y: 0, width: 20, height: 20 }
            }
          ]
        }
      ]
    }
    previous = capturePage(current, new Map())
  })
  it('should accept the same identities after a display-name change', () => {
    current.name = 'Renamed page'
    expect(() => assertCompleteCapture(previous, current)).not.toThrow()
  })
  describe('and an existing variant is missing', () => {
    beforeEach(() => {
      current.children![0]!.children!.pop()
    })
    it('should reject the partial capture', () => {
      expect(() => assertCompleteCapture(previous, current)).toThrow(
        'Incomplete capture: missing variant 3:2'
      )
    })
  })
  describe('and an existing component is missing', () => {
    beforeEach(() => {
      current.children = []
    })
    it('should reject the capture', () => {
      expect(() => assertCompleteCapture(previous, current)).toThrow(
        'Incomplete capture: missing component 2:1'
      )
    })
    it('should permit the omission of a permanently retired identity', () => {
      expect(() => assertCompleteCapture(previous, current, (id) => id === '2:1')).not.toThrow()
    })
  })
  describe('and the response contains a different page ID', () => {
    beforeEach(() => {
      current.id = 'other'
    })
    it('should reject the replacement identity', () => {
      expect(() => assertCompleteCapture(previous, current)).toThrow(
        'Capture page identity changed'
      )
    })
  })
})
