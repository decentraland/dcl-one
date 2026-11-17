import { InputField, type InputFieldProps } from '../InputField/InputField'
export type InputWebCreatorToolsProps = InputFieldProps
export function InputWebCreatorTools(props: InputWebCreatorToolsProps) {
  return <InputField {...props} surface="web" />
}
