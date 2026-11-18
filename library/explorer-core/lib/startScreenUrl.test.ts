import { beforeEach, describe, expect, it } from 'vitest'
import { dropDestination, withoutDestination } from './startScreenUrl'

// DOMAIN: the address of the start screen — a page address without a place.
describe('an address without a place', () => {
  it.each([
    ['?realm=main&position=10%2C20', ''],
    ['?world=name.dcl.eth', ''],
    ['?preview=1&position=0%2C0&systemScene=x', '?preview=1&systemScene=x'],
    ['?preview=1', '?preview=1'],
    ['', '']
  ])('%s -> %s', (search, expected) => {
    expect(withoutDestination(search)).toBe(expected)
  })
})

describe('taking the place off the address bar', () => {
  beforeEach(() => history.replaceState({ kept: true }, '', '/play/'))

  it('removes the place and keeps the path, the other parameters and the hash', () => {
    history.replaceState(null, '', '/play/?realm=main&position=10%2C20&preview=1#x')
    dropDestination()
    expect(location.pathname + location.search + location.hash).toBe('/play/?preview=1#x')
  })

  it('leaves an address with no place as it is', () => {
    history.replaceState({ kept: true }, '', '/play/?preview=1')
    dropDestination()
    expect(location.search).toBe('?preview=1')
    expect(history.state).toEqual({ kept: true })
  })
})
