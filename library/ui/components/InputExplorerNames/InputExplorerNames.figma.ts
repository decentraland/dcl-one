// url=<DCL_FIGMA_FILE>?node-id=16822-113
// source=components/InputExplorerNames/InputExplorerNames.tsx
// component=InputExplorerNames
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance
const mode = instance.getEnum('state', {
  default: '',
  hover: '',
  focus: ' counter={counter}',
  editing: ' counter={counter} suffix={suffix}',
  highlighted: '',
  completed: '',
  error: ' counter={counter} suffix={suffix} error={error}',
  disabled: ' disabled'
})
const password = instance.getEnum('type', {
  text: '',
  'password-hidden': ' type="password" allowPasswordReveal',
  'password-shown': ' type="password" allowPasswordReveal'
})

export default {
  example: figma.tsx`<InputExplorerNames label={label} value={value} onChange={onChange}${mode}${password} />`,
  imports: ["import { InputExplorerNames } from '@dcl/one-ui'"],
  id: 'InputExplorerNames',
  metadata: { nestable: true }
}
