import { type CSSProperties } from 'react'
import { useTranslation } from '@/data/lib/creators/intl'
import { BodyShapeType } from '@/data/lib/creators/items'
import { EmotePlayMode } from '@/data/lib/creators/itemFactory'
import { LoopIcon, PlayOnceIcon } from '@/features/creators/components/Icons'
import catAccessories from '@/features/creators/assets/icons/cat-accessories.svg'
import catEarring from '@/features/creators/assets/icons/cat-earring.svg'
import catEyebrows from '@/features/creators/assets/icons/cat-eyebrows.svg'
import catEyes from '@/features/creators/assets/icons/cat-eyes.svg'
import catEyewear from '@/features/creators/assets/icons/cat-eyewear.svg'
import catFacialHair from '@/features/creators/assets/icons/cat-facial-hair.svg'
import catFeet from '@/features/creators/assets/icons/cat-feet.svg'
import catHair from '@/features/creators/assets/icons/cat-hair.svg'
import catHandwear from '@/features/creators/assets/icons/cat-handwear.svg'
import catHat from '@/features/creators/assets/icons/cat-hat.svg'
import catHead from '@/features/creators/assets/icons/cat-head.svg'
import catHelmet from '@/features/creators/assets/icons/cat-helmet.svg'
import catLower from '@/features/creators/assets/icons/cat-lower.svg'
import catMask from '@/features/creators/assets/icons/cat-mask.svg'
import catMouth from '@/features/creators/assets/icons/cat-mouth.svg'
import catSkins from '@/features/creators/assets/icons/cat-skins.svg'
import catTiara from '@/features/creators/assets/icons/cat-tiara.svg'
import catTopHead from '@/features/creators/assets/icons/cat-top-head.svg'
import catUpper from '@/features/creators/assets/icons/cat-upper.svg'
import emoteDance from '@/features/creators/assets/icons/emote-dance.svg'
import emoteFun from '@/features/creators/assets/icons/emote-fun.svg'
import emoteGreetings from '@/features/creators/assets/icons/emote-greetings.svg'
import emoteHorror from '@/features/creators/assets/icons/emote-horror.svg'
import emoteMisc from '@/features/creators/assets/icons/emote-misc.svg'
import emotePoses from '@/features/creators/assets/icons/emote-poses.svg'
import emoteReactions from '@/features/creators/assets/icons/emote-reactions.svg'
import emoteStunt from '@/features/creators/assets/icons/emote-stunt.svg'
import genderFemale from '@/features/creators/assets/icons/gender-female.svg'
import genderMale from '@/features/creators/assets/icons/gender-male.svg'
import genderUnisex from '@/features/creators/assets/icons/gender-unisex.svg'
import styles from './ItemIcons.module.css'

// Wearable categories + emote categories share one namespace, like the i18n labels.
const CATEGORY_ICONS: Record<string, string> = {
  eyebrows: catEyebrows,
  eyes: catEyes,
  facial_hair: catFacialHair,
  hair: catHair,
  body_shape: catSkins,
  mouth: catMouth,
  upper_body: catUpper,
  lower_body: catLower,
  feet: catFeet,
  earring: catEarring,
  eyewear: catEyewear,
  hat: catHat,
  helmet: catHelmet,
  mask: catMask,
  tiara: catTiara,
  top_head: catTopHead,
  skin: catSkins,
  hands_wear: catHandwear,
  head: catHead,
  accessories: catAccessories,
  dance: emoteDance,
  stunt: emoteStunt,
  greetings: emoteGreetings,
  fun: emoteFun,
  poses: emotePoses,
  reactions: emoteReactions,
  horror: emoteHorror,
  miscellaneous: emoteMisc
}

const BODY_SHAPE_ICONS: Record<BodyShapeType, string> = {
  [BodyShapeType.MALE]: genderMale,
  [BodyShapeType.FEMALE]: genderFemale,
  [BodyShapeType.BOTH]: genderUnisex
}

function iconStyle(url: string): CSSProperties {
  return { '--icon-url': `url("${url}")` } as CSSProperties
}

type CategoryProps = {
  category: string
  /** Renders the translated category name next to the glyph. */
  withLabel?: boolean
  testId?: string
}

/** The category glyph (upper body, hat, dance, ...), optionally with its label. */
export function CategoryIcon({ category, withLabel = false, testId = 'category-icon' }: CategoryProps) {
  const { t } = useTranslation()
  const url = CATEGORY_ICONS[category]
  const label = t(`collection_detail_page.category.${category}`)
  const icon = url ? <span className={styles.maskIcon} role="img" aria-label={label} style={iconStyle(url)} /> : null
  if (!withLabel) return icon
  return (
    <span className={styles.labeled} data-testid={testId} data-category={category}>
      {icon}
      <span>{label}</span>
    </span>
  )
}

type BodyShapeProps = {
  bodyShape: BodyShapeType
  withLabel?: boolean
  testId?: string
}

/** The body-shape glyph (male / female / unisex), optionally with its label. */
export function BodyShapeIcon({ bodyShape, withLabel = false, testId = 'body-shape-icon' }: BodyShapeProps) {
  const { t } = useTranslation()
  const label = t(`collection_detail_page.body_type.${bodyShape}`)
  const icon = (
    <span className={styles.maskIcon} role="img" aria-label={label} style={iconStyle(BODY_SHAPE_ICONS[bodyShape])} />
  )
  if (!withLabel) return icon
  return (
    <span className={styles.labeled} data-testid={testId} data-body-shape={bodyShape}>
      {icon}
      <span>{label}</span>
    </span>
  )
}

type PlayModeProps = {
  playMode: EmotePlayMode
  withLabel?: boolean
  testId?: string
}

/** The emote play-mode glyph (loop / play once), optionally with its label. */
export function PlayModeIcon({ playMode, withLabel = false, testId = 'play-mode-icon' }: PlayModeProps) {
  const { t } = useTranslation()
  const label = t(`collection_detail_page.play_mode.${playMode}`)
  const Glyph = playMode === EmotePlayMode.LOOP ? LoopIcon : PlayOnceIcon
  const icon = <Glyph role="img" aria-label={label} aria-hidden={false} />
  if (!withLabel) return icon
  return (
    <span className={styles.labeled} data-testid={testId} data-play-mode={playMode}>
      {icon}
      <span>{label}</span>
    </span>
  )
}
