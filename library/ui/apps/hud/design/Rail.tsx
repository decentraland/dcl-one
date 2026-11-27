// Rail — the shared PagedRail with the HUD's arrow art.

import { MaskIcon, PagedRail, type PagedRailProps } from '@dcl/one-ui'
import arrowLeft from '../assets/rail/arrow-left.png'
import arrowRight from '../assets/rail/arrow-right.png'

const previous = <MaskIcon src={arrowLeft} size={15} />
const next = <MaskIcon src={arrowRight} size={15} />

export function Rail(props: Omit<PagedRailProps, 'previousIcon' | 'nextIcon' | 'previousLabel' | 'nextLabel' | 'pageLabel'>): React.JSX.Element {
  return <PagedRail {...props} previousIcon={previous} nextIcon={next} />
}
