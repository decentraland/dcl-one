import { InfoOutlinedIcon } from '~/components/Icons'
import type { ComponentProps } from 'react'
import styles from './InfoTooltip.module.css'
import { Tooltip } from './Tooltip'

type Props = Omit<ComponentProps<typeof Tooltip>, 'children' | 'asChild'>

/** The (i) glyph that opens a tooltip on hover, focus or tap. */
export function InfoTooltip(props: Props) {
  return (
    <Tooltip {...props}>
      <InfoOutlinedIcon className={styles.icon} />
    </Tooltip>
  )
}
