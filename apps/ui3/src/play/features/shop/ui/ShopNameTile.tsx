// ShopNameTile — a NAME as the Shop's dialogs show it: the shop card's NAMEs art (the NAME mark, "Name" and
// "DCL.ETH" on the NAME purple under the warm and cool glows, the corner flap with the claimed badge) at the
// dialogs' 150px, with the NEW tag once bought. Static: nothing on it is interactive. The art is the same for
// every NAME, so it is hidden from assistive technology unless given the NAME as its `label`.

import nameIcon from '../../../assets/shop/names/card/name-icon.svg'
import glowWarm from '../../../assets/shop/names/card/glow-warm.svg'
import glowCool from '../../../assets/shop/names/card/glow-cool.svg'
import verifiedOutline from '../../../assets/shop/names/card/verified-outline.svg'
import flap from '../../../assets/shop/names/tile/flap.svg'
import flapShadow from '../../../assets/shop/names/tile/flap-shadow.svg'
import styles from './ShopNameTile.module.css'

export interface ShopNameTileProps {
  isNew?: boolean
  /** The NAME this tile stands for, where nothing beside it says (an image labelled with it). */
  label?: string
  className?: string
}

export function ShopNameTile({ isNew = false, label, className }: ShopNameTileProps): React.JSX.Element {
  return (
    <span className={`${styles.tile} ${className ?? ''}`.trim()} {...(label ? { role: 'img', 'aria-label': label } : { 'aria-hidden': true })}>
      {/* The design's layer order: the shine border and the mark under the glows, the words over them. */}
      <span className={styles.shine} />
      <img className={styles.mark} src={nameIcon} alt="" />
      <img className={styles.glowWarm} src={glowWarm} alt="" />
      <img className={styles.glowCool} src={glowCool} alt="" />
      <span className={styles.domain}>DCL.ETH</span>
      <span className={styles.label}>Name</span>
      <img className={styles.flap} src={flap} alt="" />
      <img className={styles.flapShadow} src={flapShadow} alt="" />
      <img className={styles.badge} src={verifiedOutline} alt="" />
      {isNew && <span className={styles.new}>New</span>}
    </span>
  )
}
