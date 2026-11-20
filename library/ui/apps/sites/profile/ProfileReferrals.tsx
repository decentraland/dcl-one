import { useEffect, useId, useState, type CSSProperties } from 'react'

import { Button, CopyFeedback, InputWebCreatorTools, Spinner } from '../../../index'
import envelopeImage from '../assets/referrals/referral-envelope.webp'
import logoPointerImage from '../assets/referrals/logo-with-pointer.webp'
import medalImage from '../assets/referrals/sports-medal.webp'
import tier1 from '../assets/referrals/tier_1.webp'
import tier2 from '../assets/referrals/tier_2.webp'
import tier3 from '../assets/referrals/tier_3.webp'
import tier4 from '../assets/referrals/tier_4.webp'
import tier5 from '../assets/referrals/tier_5.webp'
import tier6 from '../assets/referrals/tier_6.webp'
import tier7 from '../assets/referrals/tier_7.webp'
import tier8 from '../assets/referrals/tier_8.webp'
import tier9 from '../assets/referrals/tier_9.webp'
import {
  GeneralCheck,
  GeneralCopy,
  GeneralDownArrow,
  GeneralFavoritesFilled,
  GeneralInfoButton,
  GeneralLock,
  GeneralUpArrow,
  SocialX
} from '../../../foundations/icons'

import { ProfileMoreActions } from './ProfileMoreActions'
import styles from './ProfileReferrals.module.css'

type ReferralRarity = 'epic' | 'legendary' | 'exotic' | 'mythic' | 'swag'
type ReferralTier = { invites: number; reward: string; rarity: ReferralRarity }
type Journey = {
  accepted: number
  viewed: number
  rewardImages: Readonly<Record<number, string | undefined>>
  tiers: readonly ReferralTier[]
  goal: number
  remaining: number
  fraction: number
}

/** Each tier's own picture, shown until the node sends the granted reward's. */
const TIER_IMAGES: Readonly<Record<number, string>> = {
  5: tier1,
  10: tier2,
  20: tier3,
  25: tier4,
  30: tier5,
  50: tier6,
  60: tier7,
  75: tier8,
  100: tier9
}

const RARITY_LABEL: Record<ReferralRarity, string> = {
  epic: 'Epic',
  legendary: 'Legendary',
  exotic: 'Exotic',
  mythic: 'Mythic',
  swag: 'IRL Swag Pack'
}

const STEPS = [
  {
    text: 'Invite friends to hang out in-world by sharing your referral link.',
    image: envelopeImage
  },
  {
    text: 'Your friends download Decentraland and jump in on 3 separate days.',
    image: logoPointerImage
  },
  { text: 'You earn exclusive rewards for helping grow our community!', image: medalImage }
]

function friendsJoined(count: number): string {
  return count === 1 ? '1 Friend Joined' : `${count} Friends Joined`
}

/** The invite link with its copy and share actions, and the three steps of the program. */
function ReferralHero({
  inviteUrl,
  shareHref,
  termsHref
}: {
  inviteUrl: string
  shareHref: string
  termsHref: string
}) {
  const stepsId = useId()
  const [stepsOpen, setStepsOpen] = useState(false)
  const [termsOpen, setTermsOpen] = useState(false)
  const termsId = useId()
  return (
    <section className={styles.hero} aria-labelledby={`${stepsId}-title`}>
      <img className={styles.envelope} src={envelopeImage} alt="" />
      <div className={styles.heroBody}>
        <h2 id={`${stepsId}-title`} className={styles.heroTitle}>
          Decentraland is Better With Friends
        </h2>
        <div className={styles.subtitle}>
          <p className={styles.heroSubtitle}>Invite yours and get rewards!</p>
          <Button
            variant="iconOnly"
            size="sm"
            aria-label="Referral program details"
            aria-expanded={termsOpen}
            aria-controls={termsId}
            onClick={() => setTermsOpen((open) => !open)}
          >
            <GeneralInfoButton size={16} aria-hidden="true" />
          </Button>
          <p id={termsId} className={styles.terms} hidden={!termsOpen}>
            Invitees must be new to Decentraland and must jump in-world on 3 separate days to count
            toward reward goals. See{' '}
            <a
              className={styles.termsLink}
              href={termsHref}
              target="_blank"
              rel="noopener noreferrer"
            >
              Terms &amp; Conditions
            </a>
            .
          </p>
        </div>
        <CopyFeedback
          as="div"
          value={inviteUrl}
          successAnnouncement="Link copied successfully"
          errorAnnouncement="Couldn't copy the link"
        >
          {(copy) => (
            <div className={styles.invite}>
              <div className={styles.inviteField}>
                <InputWebCreatorTools
                  className={styles.inviteInput}
                  label="Your invite link"
                  labelHidden
                  value={inviteUrl}
                  readOnly
                  onFocus={(event) => event.currentTarget.select()}
                />
                <Button variant="iconOnly" size="sm" aria-label="Copy link" onClick={copy}>
                  <GeneralCopy size={18} aria-hidden="true" />
                </Button>
              </div>
              <ProfileMoreActions
                label="Share"
                actions={[
                  {
                    label: 'Copy link',
                    icon: <GeneralCopy size={18} aria-hidden="true" />,
                    onSelect: copy
                  },
                  {
                    label: 'Share on X',
                    icon: <SocialX size={18} aria-hidden="true" />,
                    onSelect: () => window.open(shareHref, '_blank', 'noopener,noreferrer')
                  }
                ]}
              />
            </div>
          )}
        </CopyFeedback>
        <Button
          variant="text"
          size="md"
          aria-expanded={stepsOpen}
          aria-controls={stepsId}
          trailingIcon={
            stepsOpen ? (
              <GeneralUpArrow size={16} aria-hidden="true" />
            ) : (
              <GeneralDownArrow size={16} aria-hidden="true" />
            )
          }
          onClick={() => setStepsOpen((open) => !open)}
        >
          How it works
        </Button>
        <ol id={stepsId} className={styles.steps} hidden={!stepsOpen}>
          {STEPS.map((step, index) => (
            <li key={step.text} className={styles.step}>
              <span className={styles.stepNumber} aria-hidden="true">
                {index + 1}
              </span>
              <span className={styles.stepText}>{step.text}</span>
              <img className={styles.stepImage} src={step.image} alt="" loading="lazy" />
            </li>
          ))}
        </ol>
      </div>
    </section>
  )
}

/** One tier: the invites it needs, its rarity, whether it is unlocked, and the reward's picture. */
function RewardCard({
  tier,
  image,
  unlocked,
  isNew
}: {
  tier: ReferralTier
  image?: string
  unlocked: boolean
  isNew: boolean
}) {
  return (
    <li className={styles.tier} data-unlocked={unlocked || undefined}>
      <p className={styles.tierInvites}>
        {tier.invites}
        <span className={styles.visuallyHidden}> invites</span>
      </p>
      <div className={styles.card} data-rarity={tier.rarity} data-unlocked={unlocked || undefined}>
        <div className={styles.cardHeader}>
          <span className={styles.rarity} data-rarity={tier.rarity}>
            {RARITY_LABEL[tier.rarity]}
          </span>
          <span className={styles.lockState}>
            {unlocked ? (
              <GeneralCheck size={16} aria-hidden="true" />
            ) : (
              <GeneralLock size={16} aria-hidden="true" />
            )}
            <span className={styles.visuallyHidden}>{unlocked ? 'Unlocked' : 'Locked'}</span>
          </span>
        </div>
        <img className={styles.cardImage} src={image} alt={tier.reward} loading="lazy" />
        {isNew ? <span className={styles.newMark}>New</span> : null}
      </div>
    </li>
  )
}

/**
 * The tiers on a track that fills with the friends who joined. `onShown` runs once the tiers are on
 * screen, and again when the count of friends changes.
 */
function ReferralJourney({ progress, onShown }: { progress: Journey; onShown: () => void }) {
  const titleId = useId()
  // The node marks the count as seen on every read, so the tiers new at this visit are kept from its first answer.
  const [viewedAtOpen] = useState(progress.viewed)
  // The cached answer still holds the count seen before this visit; now that the tiers are shown, it is the node's.
  useEffect(() => onShown(), [onShown, progress.accepted, progress.viewed])
  const joined = friendsJoined(progress.accepted)
  const toNext = progress.remaining
  const nextText =
    toNext > 0
      ? `${toNext} more ${toNext === 1 ? 'friend' : 'friends'} for your next reward`
      : 'Every reward unlocked'
  const track = { '--journey-progress': `${progress.fraction * 100}%` } as CSSProperties

  return (
    <section className={styles.journey} aria-labelledby={titleId}>
      <div className={styles.journeyHeader}>
        <h2 id={titleId} className={styles.journeyTitle}>
          Your Reward Journey
        </h2>
        <p className={styles.joined}>
          <GeneralFavoritesFilled size={18} aria-hidden="true" /> {joined}
        </p>
      </div>
      <p className={styles.nextReward}>{nextText}</p>
      <div className={styles.scroller}>
        <div className={styles.lane}>
          <div
            className={styles.track}
            style={track}
            role="progressbar"
            aria-label="Reward journey"
            aria-valuemin={0}
            aria-valuemax={progress.goal}
            aria-valuenow={Math.min(progress.accepted, progress.goal)}
            aria-valuetext={`${joined}. ${nextText}.`}
          >
            <span className={styles.trackFill} />
          </div>
          <ol className={styles.tiers}>
            {progress.tiers.map((tier) => (
              <RewardCard
                key={tier.invites}
                tier={tier}
                image={progress.rewardImages[tier.invites] ?? TIER_IMAGES[tier.invites]}
                unlocked={tier.invites <= progress.accepted}
                isNew={tier.invites <= progress.accepted && tier.invites > viewedAtOpen}
              />
            ))}
          </ol>
        </div>
      </div>
    </section>
  )
}

export function ProfileReferrals({
  journey,
  profileAddress,
  inviteUrl,
  shareHref,
  termsHref,
  onShown,
  requireSignIn,
  failed,
  retrying,
  onRetry
}: {
  journey?: Journey
  profileAddress: string
  inviteUrl: string
  shareHref: string
  termsHref: string
  onShown: () => void
  requireSignIn: boolean
  failed: boolean
  retrying: boolean
  onRetry: () => void
}) {
  return (
    <div className={styles.root}>
      {journey ? (
        <>
          <ReferralHero inviteUrl={inviteUrl} shareHref={shareHref} termsHref={termsHref} />
          <ReferralJourney key={profileAddress} progress={journey} onShown={onShown} />
        </>
      ) : requireSignIn ? (
        <p className={styles.note}>Sign in to invite friends and unlock rewards.</p>
      ) : failed ? (
        <div className={styles.note} role="alert">
          <p className={styles.noteText}>Referral rewards are unavailable right now.</p>
          <Button variant="secondary" size="md" loading={retrying} onClick={onRetry}>
            Try again
          </Button>
        </div>
      ) : (
        <p className={styles.note} role="status">
          <Spinner variant="light" aria-hidden="true" />
          <span className={styles.visuallyHidden}>Loading referral rewards</span>
        </p>
      )}
    </div>
  )
}
