import { beforeEach, describe, expect, it } from 'vitest'
import type { FigmaClient } from '../figma/client.js'
import {
  readLiveFile,
  readLivePage,
  readReadiness,
  type LiveFile,
  type LiveNode
} from './live-file.js'

describe('when reading the live page readiness control', () => {
  let file: LiveFile
  let marker: LiveNode
  beforeEach(() => {
    marker = {
      id: 'marker',
      name: 'Sync status',
      type: 'INSTANCE',
      componentId: 'true',
      componentProperties: { ready: { type: 'VARIANT', value: 'true' } }
    }
    file = {
      version: '1',
      styles: {},
      components: { true: { componentSetId: '15973:15' } },
      document: {
        id: 'root',
        name: 'root',
        type: 'DOCUMENT',
        children: [{ id: 'page', name: ' Nav bar ', type: 'CANVAS', children: [marker] }]
      }
    }
  })
  it('should accept the ready variant of the authoritative marker', () => {
    expect(readReadiness(file)).toEqual([{ id: 'page', name: 'Nav bar', ready: true }])
  })
  describe('and the toggle is off', () => {
    beforeEach(() => {
      marker.componentProperties = { ready: { type: 'VARIANT', value: 'false' } }
    })
    it('should report false', () => {
      expect(readReadiness(file)[0]?.ready).toBe(false)
    })
  })
  describe('and the marker is from another component set', () => {
    beforeEach(() => {
      file.components.true!.componentSetId = 'other'
    })
    it('should refuse readiness', () => {
      expect(readReadiness(file)[0]?.ready).toBeNull()
    })
  })
  describe('and there are duplicate controls', () => {
    beforeEach(() => {
      file.document.children![0]!.children!.push(marker)
    })
    it('should refuse readiness', () => {
      expect(readReadiness(file)[0]?.ready).toBeNull()
    })
  })
  describe('and the control is absent', () => {
    beforeEach(() => {
      file.document.children![0]!.children = []
    })
    it('should refuse readiness', () => {
      expect(readReadiness(file)[0]?.ready).toBeNull()
    })
  })
  describe('and the value is a boolean instead of the declared variant', () => {
    beforeEach(() => {
      marker.componentProperties = { ready: { type: 'BOOLEAN', value: true } }
    })
    it('should refuse readiness', () => {
      expect(readReadiness(file)[0]?.ready).toBeNull()
    })
  })
})

describe('when reading the whole live file or a page', () => {
  it('should give each attempt 240s', async () => {
    const timeouts: (number | undefined)[] = []
    const client = {
      get: async (_path: string, options?: { timeoutMs?: number }) => {
        timeouts.push(options?.timeoutMs)
        throw new Error('stop')
      }
    } as unknown as FigmaClient

    await expect(readLiveFile(client, 'key')).rejects.toThrow('stop')
    await expect(readLivePage(client, 'key', '1:1', 'v')).rejects.toThrow('stop')
    expect(timeouts).toEqual([240000, 240000])
  })
})
