// explorer-automation `NavmapTests` (1): every category chip on the Map can be clicked.
//
// | Unity test                  | Here                                         | Class |
// | TestClickAllCategoryFilters | M opens the Map, then each of the 11 chips   | Port  |
//
// Port: Unity's chip `Sport` is `Sports` here. `Shop` is also a top-bar tab, so its chip is the
// match on the row of `All`. Like Unity's, it is a smoke test: a chip is found, actionable and
// clicked, and never read back (a chip carries no pressed state).
import { spec, type Explorer, type Selector } from '../driver/index.ts'
import { PAGE_OPEN_TIMEOUT_MS, closePage, openWithKey } from './pages.ts'

const scenario = spec(import.meta.url)

const CONTROL_TIMEOUT_MS = 20_000
const ROW_TOLERANCE_PX = 4

const CHIPS = ['All', 'Favorites', 'Social', 'Music', 'Art', 'Game', 'Fashion', 'Education', 'Shop', 'Sports', 'Business']

/** A chip's selector. When its name is not unique, the chip is the match on the row of `All`. */
async function chipSelector(explorer: Explorer, name: string): Promise<Selector> {
  const matches = (await explorer.hud.list('button')).filter((entry) => entry.name.toLowerCase() === name.toLowerCase()).length
  if (matches <= 1) return { name, role: 'button' }
  const row = (await explorer.hud.get({ name: 'All', role: 'button' })).y
  for (let index = 0; index < matches; index++) {
    const candidate = await explorer.hud.get({ name, role: 'button', index })
    if (Math.abs(candidate.y - row) < ROW_TOLERANCE_PX) return { name, role: 'button', index }
  }
  throw new Error(`none of the ${matches} "${name}" buttons is on the chip row`)
}

scenario('every category chip on the map can be clicked', async ({ explorer }) => {
  await openWithKey(explorer, 'map')
  await explorer.hud.wait({ name: 'All', role: 'button' }, { timeoutMs: PAGE_OPEN_TIMEOUT_MS })
  for (const name of CHIPS) {
    const chip = await chipSelector(explorer, name)
    await explorer.hud.wait(chip, { state: 'actionable', timeoutMs: CONTROL_TIMEOUT_MS })
    await explorer.hud.click(chip)
  }
  await closePage(explorer, 'map')
})
