// url=<DCL_FIGMA_FILE>?node-id=15721-13657
// source=components/Button/Button.tsx
// component=Button
// contract=1
// states=label#15721:0,icon-start#15721:321,icon-end#15721:642,icon-start-swap#15721:963,icon-end-swap#15721:1284,state
import figma from 'figma'
const instance = figma.selectedInstance
const property0 = instance.getEnum('variant', { 'primary': ' variant={"primary"}', 'secondary': ' variant={"secondary"}', 'outline': ' variant={"outline"}', 'dark': ' variant={"dark"}', 'text': ' variant={"text"}', 'outline-on-light': ' variant={"outlineOnLight"}' })
const property1 = instance.getEnum('size', { 'xl': ' size={"xl"}', 'lg': ' size={"lg"}', 'md': ' size={"md"}', 'sm': ' size={"sm"}' })
const property2 = instance.getEnum('viewport', { 'desktop': ' viewport={"desktop"}', 'mobile': ' viewport={"mobile"}' })
export default {
  example: figma.tsx`<Button${property0}${property1}${property2} />`,
  imports: ["import { Button } from '@dcl/one-ui'"],
  id: "Button",
  metadata: { nestable: true }
}
