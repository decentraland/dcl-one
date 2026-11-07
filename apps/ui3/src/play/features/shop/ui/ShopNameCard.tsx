// ShopNameCard — a NAME's tile in the Shop's RESALE NAME grid: the design system's shop card in its NAMEs
// variant. Its art is drawn, not a picture: the NAME's mark, "Name" and "DCL.ETH" on Brand/Purple under a
// warm and a cool glow, with the corner flap holding the claimed badge. Below it, the NAME, "Decentraland
// Name" and its price. Hovered or focused it takes the shop card's ring; clicking it is the page's to say.

import nameIcon from '../../../assets/shop/names/card/name-icon.svg'
import glowWarm from '../../../assets/shop/names/card/glow-warm.svg'
import glowCool from '../../../assets/shop/names/card/glow-cool.svg'
import flap from '../../../assets/shop/names/card/flap.svg'
import flapShadow from '../../../assets/shop/names/card/flap-shadow.svg'
import verifiedOutline from '../../../assets/shop/names/card/verified-outline.svg'
import styles from './ShopNameCard.module.css'

export interface ShopNameCardProps {
  /** The NAME, without `.dcl.eth`. */
  name: string
  /** Its price line (a resale's MANA). */
  price?: React.ReactNode
  /** Without it the tile isn't clickable. */
  onSelect?: () => void
}

export function ShopNameCard({ name, price, onSelect }: ShopNameCardProps): React.JSX.Element {
  return (
    <div className={styles.card}>
      {onSelect != null && <button type="button" className={styles.hit} aria-label={`${name}.dcl.eth`} onClick={onSelect} />}
      {/* The design's layer order: the shine border and the mark under the glows, the words over them. */}
      <span className={styles.art} aria-hidden="true">
        <span className={styles.shine} />
        <img className={styles.mark} src={nameIcon} alt="" />
        <img className={styles.glowWarm} src={glowWarm} alt="" />
        <img className={styles.glowCool} src={glowCool} alt="" />
        <span className={styles.label}>Name</span>
        <span className={styles.domain}>DCL.ETH</span>
        <img className={styles.flap} src={flap} alt="" />
        <img className={styles.flapShadow} src={flapShadow} alt="" />
        <img className={styles.badge} src={verifiedOutline} alt="" />
      </span>
      <span className={styles.info}>
        <span className={styles.name} title={name}>
          {name}
        </span>
        <span className={styles.kind}>Decentraland Name</span>
        {price != null && <span className={styles.price}>{price}</span>}
      </span>
    </div>
  )
}
