import type { NavBarBalances } from '@dcl/one-ui'
export type NativeNavBarCase = {
  id: string
  signedIn: boolean
  surface: 'solid' | 'overlay'
  balances?: NavBarBalances
  composition?: boolean
}
const balances: readonly (NavBarBalances | undefined)[] = [
  undefined,
  { credits: 101.1 },
  { credits: 101.1, mana: 30 },
  { credits: 101.1, mana: 30, polygonMana: 5 }
]
// The native public API owns this matrix. Add new layout modes here when that API changes.
export const nativeNavBarCases: NativeNavBarCase[] = [
  ...[false, true].flatMap((signedIn) =>
    (['solid', 'overlay'] as const).flatMap((surface) =>
      balances.map((value, index) => ({
        id: `${signedIn ? 'in' : 'out'}-${surface}-${index}`,
        signedIn,
        surface,
        balances: value
      }))
    )
  ),
  ...[false, true].map((signedIn) => ({
    id: `${signedIn ? 'in' : 'out'}-composition`,
    signedIn,
    surface: 'solid' as const,
    balances: balances[2],
    composition: true
  }))
]
