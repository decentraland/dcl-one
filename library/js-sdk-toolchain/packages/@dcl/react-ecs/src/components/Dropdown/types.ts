import type { PBUiDropdown } from '@dcl/ecs'
import type { TextAlignType, UiFontType } from '../Label/types'
import type { EntityPropTypes, ScaleUnit } from '../types'

/**
 * @public
 * Dropdown Props
 */
export interface UiDropdownProps
  extends EntityPropTypes, Omit<Partial<PBUiDropdown>, 'textAlign' | 'font' | 'fontSize'> {
  onChange?(value: number): void
  font?: UiFontType
  textAlign?: TextAlignType
  fontSize?: ScaleUnit
}
