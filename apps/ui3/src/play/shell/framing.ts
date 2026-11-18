// Whether a page of another origin frames the shell. The shell keeps the sign-in key and draws BUY ITEM, SEND GIFT
// and SEND TIP, which an in-app wallet signs with no window of its own, so a page of another site that framed it
// could lay its own buttons over them (clickjacking). Sites sends `frame-ancestors 'self'` and `X-Frame-Options:
// SAMEORIGIN` for /play (apps/sites routes/lib/play-framing); this is the same rule in the page, for a response
// that lost those headers on the way: every frame above the shell must be of its origin, as `frame-ancestors 'self'`
// requires. A frame of this origin stays allowed: the scene editor frames /play.

/** A frame above the shell: its parent (itself at the top) and its location, which throws for another origin. */
export interface FrameAbove {
  parent: FrameAbove | null
  location: { origin: string }
}

export interface ShellWindow extends FrameAbove {
  self: unknown
  top: unknown
  location: { origin: string; ancestorOrigins?: ArrayLike<string> }
}

/** More frames than any page nests: a chain this long is not one to trust. */
const MAX_FRAMES = 32

export function framedByAnotherOrigin(win: ShellWindow = window as unknown as ShellWindow): boolean {
  if (win.top === win.self) return false
  const origin = win.location.origin
  // The browser's own list of every frame up the chain, where it keeps one (not Firefox). An empty list in a
  // framed window says nothing, so it falls to the walk below.
  const listed = win.location.ancestorOrigins
  if (listed && listed.length > 0) return Array.from(listed).some((ancestor) => ancestor !== origin)
  return !everyFrameAboveIs(win, origin)
}

/** Walks up from the shell to the top: each frame's origin must be readable and the shell's. Reading another
 *  origin's location throws, and anything that can't be checked (a missing parent, a chain past MAX_FRAMES) fails. */
function everyFrameAboveIs(win: FrameAbove, origin: string): boolean {
  let frame = win
  for (let hops = 0; hops < MAX_FRAMES; hops++) {
    let parent: FrameAbove | null
    try {
      parent = frame.parent
    } catch {
      return false
    }
    if (parent == null) return false
    if (parent === frame) return true
    try {
      if (parent.location.origin !== origin) return false
    } catch {
      return false
    }
    frame = parent
  }
  return false
}
