// url=<DCL_FIGMA_FILE>?node-id=15819-11187
// source=components/InputWebCreatorTools/InputWebCreatorTools.tsx
// component=InputWebCreatorTools
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance
const mode = instance.getEnum('state', {
  default: '',
  focus: '',
  editing: '',
  highlighted: '',
  completed: '',
  error: ' error={error}',
  disabled: ' disabled'
})
export default {
  example: figma.tsx`<InputWebCreatorTools label={label} value={value} onChange={onChange}${mode} />`,
  imports: ["import { InputWebCreatorTools } from '@dcl/one-ui'"],
  id: 'InputWebCreatorTools',
  metadata: { nestable: true }
}
