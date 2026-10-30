// Relays HUD-relevant engine system actions to React (menu/panel hotkeys, quick emotes, Cancel,
// and the Scroll edges that drive HUD panel scrolling from non-wheel bindings).
// Read authoritatively from the engine's input stream so it fires even while the engine holds
// keyboard focus (a plain DOM keydown wouldn't see it), and so user rebinds are respected.
// The camera/pointer axis families are not forwarded — they edge repeatedly during normal play
// (mouse motion, gamepad sticks) and the HUD has no use for them. Scroll IS forwarded: the HUD
// scrolls the hovered scrollable panel on its press/release edges (see useEngineSession).
import { BevyApi } from '../bevy-api'
import { relay } from '../system-helpers'
import type { Ctx } from '../bridge'

const AXIS_ACTION = /^(Camera|Pointer)(Zoom)?(In|Out|Up|Down|Left|Right)$/

export function registerSystemAction(ctx: Ctx): void {
  relay('systemAction', async () => await BevyApi.getSystemActionStream(), (ev) => {
    if (!AXIS_ACTION.test(ev.action)) {
      ctx.send({ kind: 'systemAction', action: ev.action, pressed: ev.pressed })
    }
  })
}
