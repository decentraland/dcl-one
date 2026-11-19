// url=<DCL_FIGMA_FILE>?node-id=16713-28842
// source=components/CheckboxLabel/CheckboxLabel.tsx
// component=CheckboxLabel
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance
const position = instance.getEnum('Property 1', {
  End: '',
  Start: ' labelPosition="start"'
})
const labelLayer = instance.findText('Open Debug Console')
const label = labelLayer.type === 'TEXT' ? labelLayer.textContent : ''
export default {
  // Both variants draw the help icon; its explanation is the caller's text.
  example: figma.tsx`<CheckboxLabel label="${label}" help={help}${position} />`,
  imports: ["import { CheckboxLabel } from '@dcl/one-ui'"],
  id: 'checkbox-label',
  metadata: { nestable: true }
}
