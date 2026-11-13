import { InputField, type InputFieldProps } from '../InputField/InputField'
export type InputExplorerNamesProps = Omit<InputFieldProps, 'labelHidden'>
export function InputExplorerNames(props: InputExplorerNamesProps) {
  return <InputField {...props} surface="explorer" />
}
