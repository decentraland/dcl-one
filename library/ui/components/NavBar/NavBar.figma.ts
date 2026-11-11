// url=<DCL_FIGMA_FILE>?node-id=15599-30721
// source=components/NavBar/NavBar.tsx
// component=NavBar
// contract=1

// Hand-written component: viewport is CSS media queries and theme is not a prop, so the mapping keeps
// only the properties that change the API. Balances and the account come from the app's data.
import figma from 'figma'
const instance = figma.selectedInstance
const property0 = instance.getEnum('signed-in', {
  'false': ' onSignIn={signIn}',
  'true': ' account={account} onNotificationsClick={openNotifications} accountActions={accountActions}'
})
const property1 = instance.getEnum('balances', {
  'none': '',
  'credits-mana': ' balances={{ credits, mana, polygonMana }}',
  'credits': ' balances={{ credits }}'
})
export default {
  example: figma.tsx`<NavBar sections={sections}${property0}${property1} />`,
  imports: ["import { NavBar } from '@dcl/one-ui'"],
  id: "NavBar",
  metadata: { nestable: true }
}
