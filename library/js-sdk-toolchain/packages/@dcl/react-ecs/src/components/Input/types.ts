import type { PBUiInput } from '@dcl/ecs'
import type { TextAlignType, UiFontType } from '../Label/types'
import type { ScaleUnit } from '../types'

/**
 * @public
 */
export interface UiInputProps extends Omit<PBUiInput, 'font' | 'textAlign' | 'fontSize'> {
  /** function to be called on value change  */
  onChange?(value: string): void
  /** function to be called on text field submit  */
  onSubmit?(value: string): void
  font?: UiFontType
  textAlign?: TextAlignType
  fontSize?: ScaleUnit
}
