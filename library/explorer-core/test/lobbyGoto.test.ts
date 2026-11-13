import { describe, expect, it } from 'vitest'
import { isDestination, isGenesisParcel, parseGotoTarget, randomParcel, worldName } from '../features/lobby/goto'

describe('lobby go to', () => {
  it.each(['3,-5', '3, -5', 'mana.dcl.eth/3,-5/spawn', 'eax/spawn', '3', 'random', 'crowd', 'crowd.dcl.eth'])('accepts %s', (text) => {
    expect(isDestination(text)).toBe(true)
  })

  it.each(['1000,-500', '3,-5,7', 'your.world.eth', 'my-world.dcl.eth', 'foo bar', 'mana.dcl.eth/'])('rejects %s', (text) => {
    expect(isDestination(text)).toBe(false)
  })

  it('reads every form', () => {
    expect(parseGotoTarget('3,-5')).toMatchObject({ world: null, parcel: { x: 3, y: -5 }, spawnPoint: null })
    expect(parseGotoTarget('3,-5/door')).toMatchObject({ world: null, parcel: { x: 3, y: -5 }, spawnPoint: 'door' })
    expect(parseGotoTarget('mana/1,2')).toMatchObject({ world: 'mana', parcel: { x: 1, y: 2 }, spawnPoint: null })
    expect(parseGotoTarget('mana/door')).toMatchObject({ world: 'mana', parcel: null, spawnPoint: 'door' })
    expect(parseGotoTarget('mana/1,2/door')).toMatchObject({ world: 'mana', parcel: { x: 1, y: 2 }, spawnPoint: 'door' })
    expect(parseGotoTarget('random').random).toBe(true)
    expect(parseGotoTarget('crowd').crowd).toBe(true)
  })

  it('bounds Genesis City with its extensions', () => {
    expect(isGenesisParcel(150, 150)).toBe(true)
    expect(isGenesisParcel(162, 158)).toBe(true)
    expect(isGenesisParcel(163, 150)).toBe(true)
    expect(isGenesisParcel(0, 160)).toBe(false)
    expect(isGenesisParcel(152, 0)).toBe(false)
  })

  it('completes short world names', () => {
    expect(worldName('mana')).toBe('mana.dcl.eth')
    expect(worldName('mana.dcl.eth')).toBe('mana.dcl.eth')
    expect(worldName('name.eth')).toBe('name.eth')
  })

  it('picks random parcels inside the square city', () => {
    expect(randomParcel(() => 0)).toEqual({ x: -150, y: -150 })
    expect(randomParcel(() => 0.9999)).toEqual({ x: 149, y: 149 })
  })
})
