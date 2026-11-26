// Chip — the HUD's filter chip, drawn by the shared Chip with the HUD's close glyph in its clear box.
import { Chip as SharedChip, MaskIcon, type ChipProps } from '@dcl/one-ui'
import closeIcon from '../assets/backpack/icon-close.webp'

export function Chip(props: Omit<ChipProps, 'clearIcon'>): React.JSX.Element {
  return <SharedChip {...props} clearIcon={<MaskIcon src={closeIcon} size={10} />} />
}
