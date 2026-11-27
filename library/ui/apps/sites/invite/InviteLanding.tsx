import { useEffect, useRef, useState, type ReactNode } from 'react'
import { AnimatedBackground, Button } from '../../../index'
import envelopeImage from '../assets/referrals/referral-envelope.webp'
import clipPoster from '../assets/invite/referral-poster.webp'
import clipUrl from '../assets/invite/referral.webm'
import styles from './InviteLanding.module.css'

// The clip loads once it is in view on a desktop-wide screen (foundations breakpoints.desktop) and motion is
// welcome; until then, and on phones, the poster stands in for it. Width and motion stay live: when either stops
// allowing it, the clip stops and the poster returns, and it comes back only while it is in view.
const CLIP_WIDE = '(min-width: 1024px)'
const REDUCED_MOTION = '(prefers-reduced-motion: reduce)'

function useClipAllowed(target: { current: Element | null }): boolean {
  const [allowed, setAllowed] = useState(false)
  useEffect(() => {
    const el = target.current
    if (
      !el ||
      typeof window.matchMedia !== 'function' ||
      typeof IntersectionObserver === 'undefined'
    )
      return
    const wide = window.matchMedia(CLIP_WIDE)
    const reduced = window.matchMedia(REDUCED_MOTION)
    let inView = false
    const update = () => setAllowed((was) => wide.matches && !reduced.matches && (was || inView))
    // A box that only touches the viewport edge counts as intersecting with a zero ratio: it is not in view yet,
    // and the second threshold reports the moment it enters.
    const observer = new IntersectionObserver(
      (entries) => {
        inView = entries.some((entry) => entry.isIntersecting && entry.intersectionRatio > 0)
        update()
      },
      { threshold: [0, 0.01] }
    )
    observer.observe(el)
    wide.addEventListener('change', update)
    reduced.addEventListener('change', update)
    return () => {
      observer.disconnect()
      wide.removeEventListener('change', update)
      reduced.removeEventListener('change', update)
    }
  }, [target])
  return allowed
}

type Faq = { q: string; a: string }

// The FAQ text of decentraland.org's invite page.
const FAQS: Faq[] = [
  {
    q: 'What is Decentraland?',
    a: "Decentraland is a community-driven virtual world where you can connect, explore, and create. Built on the Unity engine, Decentraland streams and renders a vast, continuous environment filled with diverse, user-generated content. As you explore this immersive landscape, you encounter new creations daily, from games and puzzles to scenes and artworks, all crafted by Decentraland's vibrant community of creators.\n\nDecentraland goes beyond traditional gaming experiences. In Decentraland, you can attend live music events, conferences, exhibitions, dance parties, and more.\n\nYour New Friends Are Waiting -- Download Decentraland today and jump in!"
  },
  {
    q: 'How do I enter Decentraland?',
    a: "Step 1: Download and Install the Client\nFirst you will need to install the client, go to Decentraland and click download.\n\nStep 2: Create your Account\nWhen you enter Decentraland, you can use a standard web3 wallet such as Metamask, or log in with a Social account (Google, Discord, X, or Apple).\n\nStep 3: Customize Your Avatar\nOnce you enter Decentraland you'll first be asked to customize your avatar.\n\nStep 4: Explore!\nAfter completing a quick tutorial, Decentraland is yours to explore!\n\nStep 5: Have fun!\nWalk around, talk to others via the chat box, and pop into some events!"
  },
  {
    q: 'Do I need cryptocurrency or a digital wallet to use Decentraland?',
    a: "You do not need to own cryptocurrency to enjoy Decentraland as it is free to use. If you decide to purchase a community-made creation from the Marketplace, there are multiple payment options available, such as credit/debit card and bank transfer in addition to various cryptocurrencies.\n\nAs for owning a digital wallet, if you don't already have one, you don't need to get one yourself. When you sign in to Decentraland for the first time, a digital wallet will be created for you behind the scenes if you choose to use a Social account."
  },
  {
    q: 'How do I become a Decentraland Creator?',
    a: 'Decentraland is a world created by its users, meaning you can be a creator of everything from Wearables, Emotes, entire scenes with buildings and/or landscapes, interactive experiences, and games.\n\nMost Wearable and Emote designers use Blender to design their creations. Once complete, items are submitted for publishing in the Marketplace.\n\nYou can also download the Creator Hub which combines all of the ease of use of the Web Editor with the power of writing code.'
  },
  {
    q: 'What is a NAME?',
    a: 'Costing just 100 MANA, the currency of Decentraland, a Decentraland NAME is essentially a unique and personalized username for your avatar that represents your virtual identity within the Decentraland ecosystem. Getting a Decentraland NAME comes with the added bonus of granting the owner their own World.'
  },
  {
    q: 'What is the difference between LANDs and Worlds?',
    a: "Currently, Worlds can only admit up to 100 visitors at a time and scenes are limited to 100MB whereas LAND parcels have no such limitations. If you would like to build large scenes or host big events, buying or renting LAND will be a better option.\n\nScenes deployed on LANDs in Genesis City are also more easily discovered by users as they don't need to know your NAME to visit."
  },
  {
    q: 'How can I get help and contact the Support Team?',
    a: 'You can contact the Decentraland Support Team via live chat or you can join the official Decentraland Discord server. Please remember that Decentraland staff will never ask you for the private key (seed phrase) to your crypto wallet.'
  }
]

function GradientTitle({ text }: { text: string }) {
  const idx = text.toLowerCase().indexOf('decentraland')
  if (idx === -1) return <>{text}</>
  return (
    <>
      {text.slice(0, idx)}
      <span className={styles.gradient}>Decentraland</span>
      {text.slice(idx + 'Decentraland'.length)}
    </>
  )
}

function CircleAndArrow({ open }: { open: boolean }) {
  return (
    <svg
      className={styles.arrow}
      data-open={open || undefined}
      width="57"
      height="57"
      viewBox="0 0 72 72"
      fill="none"
      aria-hidden="true"
    >
      <circle
        opacity={open ? 1 : 0.2}
        cx="36"
        cy="36"
        r="35"
        stroke="white"
        fill={open ? 'white' : 'none'}
        strokeWidth="2"
      />
      <path
        d="M45 33.0022L42.885 30.8872L36 37.7572L29.115 30.8872L27 33.0022L36 42.0022L45 33.0022Z"
        fill={open ? '#242129' : 'white'}
      />
    </svg>
  )
}

function Envelope({ size }: { size: number }) {
  return (
    <div className={styles.envelope}>
      <span className={styles.envelopeGlow} />
      <img
        className={styles.envelopeImage}
        src={envelopeImage}
        alt="Envelope"
        loading="eager"
        width={size}
        height={size}
      />
    </div>
  )
}

type HeroProps = {
  title: string
  subtitle: string
  buttonLabel: string
  joinHref: string
  /** The first hero names the inviter and shows the envelope and the avatar; the second plays the clip. */
  primary?: { referrerName: string | null; avatar: ReactNode }
}

// The invite hero as decentraland.org draws it. Both envelopes render and CSS shows the one for the width, where
// the original picks one with useDesktopMediaQuery.
function InviteHero({ title, subtitle, buttonLabel, joinHref, primary }: HeroProps) {
  const Title = primary ? 'h1' : 'h2'
  const clipRef = useRef<HTMLVideoElement>(null)
  const clipAllowed = useClipAllowed(clipRef)
  // The src attribute alone neither stops a playing clip nor starts one set after mount: drive the element.
  useEffect(() => {
    const video = clipRef.current
    if (!video) return
    if (clipAllowed) void video.play?.()?.catch(() => {})
    else if (video.currentSrc) {
      video.pause()
      video.load()
    }
  }, [clipAllowed])
  return (
    <section className={primary ? `${styles.hero} ${styles.primary}` : styles.hero}>
      <div className={styles.heroInner}>
        <div className={styles.rail}>
          <div className={styles.railText}>
            {primary && (
              <div className={styles.desktopOnly}>
                <Envelope size={115} />
              </div>
            )}
            <Title className={styles.title}>
              {primary?.referrerName ? `${primary.referrerName} ` : null}
              <GradientTitle text={title} />
            </Title>
            <p className={styles.subtitle}>{subtitle}</p>
          </div>
          <div className={styles.actions}>
            <Button variant="primary" size="lg" className={styles.cta} href={joinHref}>
              {buttonLabel}
            </Button>
          </div>
        </div>

        <div className={styles.media}>
          <AnimatedBackground />
          {!primary && <div className={styles.overlay} aria-hidden="true" />}
          {!primary && (
            <video
              ref={clipRef}
              className={styles.video}
              poster={clipPoster}
              src={clipAllowed ? clipUrl : undefined}
              preload="none"
              loop
              muted
              autoPlay={clipAllowed}
              playsInline
              aria-hidden="true"
            />
          )}
          <div className={styles.avatar}>
            {primary && (
              <div className={styles.mobileOnly}>
                <Envelope size={160} />
              </div>
            )}
            <div className={styles.avatarSlot} data-invite-avatar="">
              {primary?.avatar}
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}

function InviteFaqs({ moreHref }: { moreHref: string }) {
  const [expanded, setExpanded] = useState<number | false>(false)
  return (
    <section className={styles.faqs} aria-label="Frequently Asked Questions">
      <div className={styles.faqsBorder}>
        <div className={styles.faqsInner}>
          <p className={styles.faqsEyebrow}>Learn more about Decentraland</p>
          <h2 className={styles.faqsTitle}>
            Frequently Asked
            {'\n'}
            Questions
          </h2>
          {FAQS.map((faq, i) => {
            const open = expanded === i
            return (
              <div
                key={i}
                className={styles.faq}
                data-open={open || undefined}
                role="button"
                tabIndex={0}
                aria-expanded={open}
                onClick={() => setExpanded(open ? false : i)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault()
                    setExpanded(open ? false : i)
                  }
                }}
              >
                <div className={styles.faqRow}>
                  <span className={styles.faqQuestion}>{faq.q}</span>
                  <CircleAndArrow open={open} />
                </div>
                <div
                  className={styles.faqAnswer}
                  data-open={open || undefined}
                  role="region"
                  aria-hidden={!open}
                >
                  <p className={styles.faqAnswerText}>{faq.a}</p>
                </div>
              </div>
            )
          })}
          <a className={styles.faqsMore} href={moreHref}>
            see more
          </a>
        </div>
      </div>
    </section>
  )
}

export type InviteLandingProps = {
  /** The inviter's name; none shows the title alone, as decentraland.org does. */
  referrerName?: string | null
  /** The sign-in both heroes' calls to action open. */
  joinHref: string
  /** The inviter's avatar, drawn by the host beside the envelope. */
  avatar?: ReactNode
  /** Where "see more" under the questions leads. */
  faqsHref: string
}

/** decentraland.org's invite landing: the invitation, the hang-out hero with its clip, and the questions. */
export function InviteLanding({
  referrerName = null,
  joinHref,
  avatar = null,
  faqsHref
}: InviteLandingProps) {
  return (
    <main className={styles.root}>
      <InviteHero
        title="invited you to join Decentraland"
        subtitle="A social virtual world where you can connect, create, and be yourself"
        buttonLabel="JOIN NOW"
        joinHref={joinHref}
        primary={{ referrerName, avatar }}
      />
      <InviteHero
        title="Hang out in Decentraland"
        subtitle="Meet people from around the world, attend live events, and customize your digital self -- all in a virtual world owned by its community."
        buttonLabel="JOIN FREE"
        joinHref={joinHref}
      />
      <InviteFaqs moreHref={faqsHref} />
    </main>
  )
}
