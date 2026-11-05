import { beforeEach, describe, expect, it } from 'vitest'
import { PREF, getPref, setPref } from '../../lib/prefs'
import { clearPingMode, pingModeFor, resetPingPrefs, setPingDefault, setPingMode, shouldPing } from './ping'

// DOMAIN: the chat notification ping — per-conversation All / Mentions Only / None, a default for
// the rest, and what it decides for a line that landed.
describe('chat ping prefs', () => {
  beforeEach(() => {
    localStorage.clear()
    resetPingPrefs()
  })

  it('defaults to all messages, per conversation on top', () => {
    expect(pingModeFor('Nearby')).toBe('all')
    setPingMode('community:c1', 'mentions')
    expect(pingModeFor('community:c1')).toBe('mentions')
    expect(pingModeFor('Nearby')).toBe('all')
    setPingDefault('none')
    expect(pingModeFor('Nearby')).toBe('none')
    expect(pingModeFor('community:c1')).toBe('mentions')
    clearPingMode('community:c1')
    expect(pingModeFor('community:c1')).toBe('none')
    // the settings tab's reset: every conversation back to the default
    setPingMode('Nearby', 'all')
    setPingMode('0xbob', 'mentions')
    clearPingMode()
    expect(pingModeFor('Nearby')).toBe('none')
    expect(pingModeFor('0xbob')).toBe('none')
    expect(JSON.parse(getPref(PREF.chatPingChannels) ?? '{}')).toEqual({})
  })

  it('reads what an earlier session stored and ignores junk', () => {
    setPref(PREF.chatPingChannels, JSON.stringify({ 'community:c1': 'none', Nearby: 'loud' }))
    setPref(PREF.chatPingDefault, 'mentions')
    expect(pingModeFor('community:c1')).toBe('none')
    expect(pingModeFor('Nearby')).toBe('mentions')
  })

  it('decides the sound from the mode and the mention', () => {
    setPingMode('a', 'all')
    setPingMode('m', 'mentions')
    setPingMode('n', 'none')
    expect(shouldPing('a', false)).toBe(true)
    expect(shouldPing('m', false)).toBe(false)
    expect(shouldPing('m', true)).toBe(true)
    expect(shouldPing('n', true)).toBe(false)
  })
})
