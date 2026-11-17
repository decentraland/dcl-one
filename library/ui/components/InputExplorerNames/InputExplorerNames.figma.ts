// url=<DCL_FIGMA_FILE>?node-id=7522-2940
// source=components/InputExplorerNames/InputExplorerNames.tsx
// component=InputExplorerNames
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance
const mode = instance.getEnum('Property 1', {
  DefaultEmpty: '',
  FocusedEdit: ' counter={counter}',
  FocusedEditing: ' counter={counter} suffix={suffix}',
  Error: ' counter={counter} suffix={suffix} error={error}',
  IdleEdited: '',
  Password: ' type="password" allowPasswordReveal',
  'Password-FocusedEditing-hidden': ' type="password" allowPasswordReveal',
  'Password-FocusedEditing-shown': ' type="password" allowPasswordReveal'
})
export default {
  example: figma.tsx`<InputExplorerNames label={label} value={value} onChange={onChange}${mode} />`,
  imports: ["import { InputExplorerNames } from '@dcl/one-ui'"],
  id: 'InputExplorerNames',
  metadata: { nestable: true }
}
