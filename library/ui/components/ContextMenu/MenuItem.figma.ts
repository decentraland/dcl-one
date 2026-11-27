// url=<DCL_FIGMA_FILE>?node-id=16673-640
// source=components/ContextMenu/ContextMenu.tsx
// component=ContextMenu
// contract=1
// states=state
import figma from 'figma'
const instance = figma.selectedInstance

// A row is one entry of ContextMenu's items, not a component of its own: the entry takes the row's label, its icon
// as `icon` and its action as `onClick`. The row draws its hover and pressed looks; it has no disabled or selected
// look, no shortcut and no 44px mobile height, so those values stay unmapped. The check of a checked or unchecked
// row sits in the icon's place, so those rows take no icon.
const type = instance.getEnum('type', {
  option: 'label',
  'section-title': "kind: 'title', label",
  divider: "kind: 'separator'"
})
const trailing = instance.getEnum('trailing', {
  none: '',
  chevron: ", kind: 'submenu', items",
  switch: ", kind: 'toggle', checked, onChange",
  checked: ", kind: 'option', checked: true",
  unchecked: ", kind: 'option', checked: false"
})
const tone = instance.getEnum('tone', { default: '', destructive: ', danger: true' })
const state = instance.getEnum('state', { default: '', hover: '', pressed: '' })
const viewport = instance.getEnum('viewport', { desktop: '' })

export default {
  example: figma.tsx`<ContextMenu items={[{ ${type}${trailing}${tone}${state}${viewport} }]} />`,
  imports: ["import { ContextMenu } from '@dcl/one-ui'"],
  id: 'bold/menu-item'
}
