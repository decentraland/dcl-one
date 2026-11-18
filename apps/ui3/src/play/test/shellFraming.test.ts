// The shell refuses to start in another site's frame (shell/framing): the rule itself, on fake window chains, and
// what the page shows instead of the app. The rule is faked for the page; jsdom has no service worker, so an
// unframed shell would mount the app's iframe at once, which is what the page test checks is missing.

import { describe, expect, it, vi } from 'vitest'

const framing = vi.hoisted(() => ({ framed: true }))
vi.mock('../shell/framing', async (importOriginal) => {
  const real = await importOriginal<typeof import('../shell/framing')>()
  return { ...real, framedByAnotherOrigin: (win?: Parameters<typeof real.framedByAnotherOrigin>[0]) => (win ? real.framedByAnotherOrigin(win) : framing.framed) }
})

import { framedByAnotherOrigin, type FrameAbove, type ShellWindow } from '../shell/framing'
import { mountPlayShell } from '../shell/mount'

const ORIGIN = 'https://dcl.test'
const EVIL = 'https://evil.test'

/** A top page: its own parent. */
function topAt(origin: string): FrameAbove {
  const top: FrameAbove = { parent: null, location: { origin } }
  top.parent = top
  return top
}
/** A frame of another origin: reading its location throws, as a browser's does. */
function unreadable(parent: FrameAbove): FrameAbove {
  return {
    parent,
    get location(): { origin: string } {
      throw new DOMException('Blocked a frame from accessing a cross-origin frame.', 'SecurityError')
    }
  }
}
const frameAt = (origin: string, parent: FrameAbove): FrameAbove => ({ parent, location: { origin } })
/** The shell framed under `parent`, whose top is `top`; `ancestorOrigins` as the browser lists it, if it does. */
function shellUnder(parent: FrameAbove, top: FrameAbove, ancestorOrigins?: string[]): ShellWindow {
  return { self: {}, top, parent, location: { origin: ORIGIN, ...(ancestorOrigins ? { ancestorOrigins } : {}) } }
}

describe('whether another origin frames the shell', () => {
  it('is not framed as the top page', () => {
    const self = {}
    expect(framedByAnotherOrigin({ self, top: self, parent: null, location: { origin: ORIGIN } })).toBe(false)
  })

  it('reads the list of every frame up the chain where the browser keeps one', () => {
    const top = topAt(ORIGIN)
    expect(framedByAnotherOrigin(shellUnder(top, top, [ORIGIN]))).toBe(false)
    expect(framedByAnotherOrigin(shellUnder(top, top, [EVIL, ORIGIN]))).toBe(true)
  })

  describe('where the browser lists no frames, walking up to the top', () => {
    it('allows a chain of its own origin all the way up, as the scene editor frames /play', () => {
      const top = topAt(ORIGIN)
      expect(framedByAnotherOrigin(shellUnder(frameAt(ORIGIN, top), top))).toBe(false)
    })

    it('refuses an unreadable frame between it and a top page of its own origin', () => {
      const top = topAt(ORIGIN)
      expect(framedByAnotherOrigin(shellUnder(unreadable(top), top))).toBe(true)
    })

    it('refuses a readable frame of another origin, and a top page it cannot read', () => {
      const top = topAt(ORIGIN)
      expect(framedByAnotherOrigin(shellUnder(frameAt(EVIL, top), top))).toBe(true)
      const evilTop = unreadable(null as unknown as FrameAbove)
      evilTop.parent = evilTop
      expect(framedByAnotherOrigin(shellUnder(evilTop, evilTop))).toBe(true)
    })

    it('refuses a chain it cannot finish: a missing parent, or one longer than any page nests', () => {
      const top = topAt(ORIGIN)
      expect(framedByAnotherOrigin(shellUnder(frameAt(ORIGIN, null as unknown as FrameAbove), top))).toBe(true)
      const loop: FrameAbove = { parent: null, location: { origin: ORIGIN } }
      loop.parent = frameAt(ORIGIN, loop)
      expect(framedByAnotherOrigin(shellUnder(loop, top))).toBe(true)
    })

    it('walks the same way when a framed window lists no frames at all', () => {
      const top = topAt(ORIGIN)
      expect(framedByAnotherOrigin(shellUnder(unreadable(top), top, []))).toBe(true)
      expect(framedByAnotherOrigin(shellUnder(frameAt(ORIGIN, top), top, []))).toBe(false)
    })
  })
})

describe('the shell in another site’s frame', () => {
  it('starts no app, and offers to open Decentraland in a tab of its own', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.stubGlobal('crossOriginIsolated', false)
    mountPlayShell()
    expect(document.querySelector('iframe')).toBeNull()
    expect(document.body.textContent).toContain("Decentraland can't run inside another site.")
    const open = document.querySelector('a') as HTMLAnchorElement
    expect(open.href).toBe(location.href)
    expect(open.target).toBe('_blank')
    expect(open.rel).toBe('noopener noreferrer')
  })
})
