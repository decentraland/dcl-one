// url=<DCL_FIGMA_FILE>?node-id=15599-30721
// source=components/NavBar/NavBar.tsx
// component=NavBar
// contract=1

import figma from 'figma'
const instance = figma.selectedInstance
const property0 = instance.getEnum('viewport', { 'large-desktop': ' viewport={"largeDesktop"}', 'mobile': ' viewport={"mobile"}', 'desktop': ' viewport={"desktop"}' })
const property1 = instance.getEnum('signed-in', { 'false': ' signedIn={false}', 'true': ' signedIn={true}' })
const property2 = instance.getEnum('balances', { 'none': ' balances={"none"}', 'credits-mana': ' balances={"creditsMana"}', 'credits': ' balances={"credits"}' })
const property3 = instance.getEnum('theme', { 'dark': ' theme={"dark"}', 'light': ' theme={"light"}' })
export default {
  example: figma.tsx`<NavBar${property0}${property1}${property2}${property3} />`,
  imports: ["import { NavBar } from '@dcl/one-ui'"],
  id: "NavBar",
  metadata: { nestable: true }
}
