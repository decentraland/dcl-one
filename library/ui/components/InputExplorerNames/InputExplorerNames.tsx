import { InputField, type InputFieldProps } from '../InputField/InputField'
export type InputExplorerNamesProps = InputFieldProps
export function InputExplorerNames(props: InputExplorerNamesProps) {
  return <InputField {...props} surface="explorer" />
}
