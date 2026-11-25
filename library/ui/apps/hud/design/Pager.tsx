// Pager — the HUD's pager, drawn by the shared Pager with the HUD's arrow glyph.
import { Pager as SharedPager, type PagerProps } from '@dcl/one-ui'
import { MaskIcon } from './MaskIcon'
import arrowLeftIcon from '../assets/backpack/icon-arrow-left.webp'

export function Pager(props: Omit<PagerProps, 'arrowIcon'>): React.JSX.Element | null {
  return <SharedPager {...props} arrowIcon={<MaskIcon src={arrowLeftIcon} size={12} />} />
}
