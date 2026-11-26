import { useContext, useRef, useState } from 'react'
import {
  Button,
  ConnectionStatus,
  CountBadge,
  ItemLabel,
  Modal,
  ModalActions,
  ModalCard,
  ModalShell,
  ModalTitle,
  ProfilePic,
  ProgressBar,
  Skeleton,
  Spinner,
  Tag,
  Toast,
  Tooltip,
  type ModalProps,
  type ProfilePicSize,
  type TagVariant,
  type ToastVariant,
  EmptyState,
  Panel,
  SwapLabel,
  VoiceBars,
  DclLogo,
  MaskIcon,
  ScrollThumb
} from '@dcl/one-ui'
import { EmbeddedCanvas } from '../embedded'
import type { GalleryEntry } from '../registry'
import { covers, people } from './data'
import layout from './layout.module.css'

const picSizes: ProfilePicSize[] = [76, 72, 60, 48, 40, 32, 28, 24, 20]
const tagVariants: TagVariant[] = [
  'live',
  'online',
  'streaming',
  'listening',
  'voiceStream',
  'featured',
  'public',
  'private',
  'teen',
  'adult',
  'restricted'
]
const toastVariants: ToastVariant[] = ['text', 'confirmation', 'error']

type ModalDemoProps = Omit<ModalProps, 'onClose' | 'actions'> & {
  open?: boolean
  trigger: string
  /** Action labels, the main one last; each closes the dialog. */
  actions?: readonly string[]
}

// A dialog is rendered only while it is open; the trigger opens it again after it closes. Inside the Showcase it
// starts closed: opening on load would take the reviewer's focus partway down the page.
function ModalDemo({ open: initiallyOpen = false, trigger, actions, ...props }: ModalDemoProps) {
  const embedded = useContext(EmbeddedCanvas)
  const [open, setOpen] = useState(initiallyOpen && !embedded)
  const close = () => setOpen(false)
  return (
    <>
      <Button size="md" variant="secondary" onClick={() => setOpen(true)}>
        {trigger}
      </Button>
      {open ? (
        <Modal
          {...props}
          onClose={close}
          actions={actions?.map((label, index) => (
            <Button
              key={label}
              size="md"
              variant={index === actions.length - 1 ? 'primary' : 'secondary'}
              onClick={close}
            >
              {label}
            </Button>
          ))}
        />
      ) : null}
    </>
  )
}

function ToastDemo({ variant }: { variant: ToastVariant }) {
  const [shown, setShown] = useState(true)
  return shown ? (
    <Toast
      variant={variant}
      placement="inline"
      duration={0}
      closeButton
      onClose={() => setShown(false)}
      action={
        variant === 'error' ? (
          <Button size="sm" variant="text">
            Retry
          </Button>
        ) : undefined
      }
    >
      {variant === 'error' ? 'Could not save your changes.' : `A ${variant} message.`}
    </Toast>
  ) : (
    <Button size="sm" variant="secondary" onClick={() => setShown(true)}>
      Show again
    </Button>
  )
}

export const feedbackEntries: GalleryEntry[] = [
  {
    name: 'ModalCard',
    group: 'Feedback',
    summary:
      "The Explorer HUD's dialog card and its parts: a title, a body and a row of actions. Its host owns the backdrop and the focus.",
    components: ['ModalCard', 'ModalTitle', 'ModalActions', 'ModalShell'],
    examples: [
      {
        id: 'shell',
        title: 'ModalShell: a title with a close button, a body and two equal actions',
        render: () => (
          <div className={layout.hudHost}>
            <ModalShell
              title="Leave this community?"
              subtitle="You can join again at any time."
              ariaLabel="Leave this community?"
              modal={false}
              onClose={() => {}}
              actionsEqual
              actions={
                <>
                  <Button appearance="hud" variant="secondary" size="bare" radius="md">
                    CANCEL
                  </Button>
                  <Button appearance="hud" size="bare" radius="md">
                    LEAVE
                  </Button>
                </>
              }
            >
              Your posts stay in the community.
            </ModalShell>
          </div>
        )
      },
      {
        id: 'centered',
        title: 'A centered title with a back button, and a lead note beside the action',
        render: () => (
          <div className={layout.hudHost}>
            <ModalShell
              title="Report submitted"
              ariaLabel="Report submitted"
              modal={false}
              centeredTitle
              onBack={() => {}}
              onClose={() => {}}
              actionsLead="Our moderators will review it."
              actions={
                <Button appearance="hud" size="sm">
                  OK
                </Button>
              }
            >
              Thanks for helping keep Decentraland safe.
            </ModalShell>
          </div>
        )
      },
      {
        id: 'card',
        title: 'ModalCard with its own layout: a title and a column of actions',
        render: () => (
          <div className={layout.hudHost}>
            <ModalCard ariaLabel="Unsaved changes" modal={false} width={360}>
              <ModalTitle title="Unsaved changes" subtitle="Your avatar has changes." />
              <ModalActions direction="column">
                <Button appearance="hud" size="sm">
                  SAVE
                </Button>
                <Button appearance="hud" variant="ghost" size="sm">
                  DISCARD
                </Button>
              </ModalActions>
            </ModalCard>
          </div>
        )
      }
    ]
  },
  {
    name: 'Modal',
    group: 'Feedback',
    summary:
      'A native modal dialog. The first example opens on load; the others open from their trigger.',
    components: ['Modal'],
    minHeight: 720,
    examples: [
      {
        id: 'message',
        title: 'Message with image, checkbox and actions',
        render: () => (
          <ModalDemo
            open
            trigger="Open message dialog"
            title="Leave this scene?"
            description="You will return to Genesis Plaza."
            image={<img src={covers.plaza} alt="" />}
            checkbox={{ label: "Don't ask again" }}
            divider
            actions={['Stay', 'Leave']}
          />
        )
      },
      {
        id: 'gallery',
        title: 'Gallery layout',
        render: () => (
          <ModalDemo
            trigger="Open gallery dialog"
            layout="gallery"
            title="Photo"
            titleHidden
            image={<img src={covers.festival} alt="Festival stage" />}
          />
        )
      },
      {
        id: 'busy',
        title: 'Not dismissible, without close button',
        render: () => (
          <ModalDemo
            trigger="Open busy dialog"
            title="Saving"
            description="Wait until the request ends; this example closes from its action."
            dismissible={false}
            closeButton={false}
            actions={['Close example']}
          />
        )
      }
    ]
  },
  {
    name: 'Toast',
    group: 'Feedback',
    summary: 'A status or alert message, shown inline here with no timeout.',
    components: ['Toast'],
    examples: [
      ...toastVariants.map((variant) => ({
        id: variant,
        title: variant[0]!.toUpperCase() + variant.slice(1),
        render: () => <ToastDemo variant={variant} />
      })),
      {
        id: 'hud',
        title: 'Explorer HUD: a notice, an error and a notice with an action',
        render: () => (
          <div className={layout.stack}>
            <Toast
              appearance="hud"
              placement="inline"
              duration={0}
              icon={
                <ProfilePic
                  appearance="hud"
                  name={people.ada.name}
                  src={people.ada.picture}
                  nameColor={people.ada.color}
                  size={28}
                  alt=""
                />
              }
            >
              <b>{people.ada.name}</b> is online
            </Toast>
            <Toast appearance="hud" placement="inline" variant="error" duration={0}>
              Something went wrong.
            </Toast>
            <Toast
              appearance="hud"
              placement="inline"
              duration={0}
              action={
                <Button appearance="hud" variant="secondary" size="sm">
                  Reload
                </Button>
              }
            >
              Textures optimized
            </Toast>
          </div>
        )
      }
    ]
  },
  {
    name: 'Tooltip',
    group: 'Feedback',
    summary:
      'A short label or note beside its anchor, shown on hover and keyboard focus; the HUD tip opens with the pointer alone.',
    components: ['Tooltip'],
    examples: [
      {
        id: 'sides',
        title: 'Sides',
        render: () => (
          <div className={`${layout.row} ${layout.tooltipHost} ${layout.tooltipRoom}`}>
            {(['top', 'right', 'bottom', 'left'] as const).map((side) => (
              <Tooltip key={side} label={`On the ${side}`} side={side} arrow>
                <Button size="md" variant="secondary">
                  {side}
                </Button>
              </Tooltip>
            ))}
          </div>
        )
      },
      {
        id: 'rich',
        title: 'Rich content in a portal',
        render: () => (
          <div className={`${layout.row} ${layout.tooltipHost} ${layout.tooltipRoom}`}>
            <Tooltip
              portal
              keepInViewport
              side="top"
              maxWidth={240}
              tipClassName={layout.tooltipHost}
              content={<span>Wraps at 240px and stays inside the window.</span>}
            >
              <Button size="md" variant="secondary">
                Details
              </Button>
            </Tooltip>
          </div>
        )
      },
      {
        id: 'hud',
        title:
          'Explorer HUD: a name with a shortcut, the sidebar rail, a paragraph and an end-aligned tip',
        render: () => (
          <div
            className={`${layout.row} ${layout.tooltipHost} ${layout.tooltipRoom} ${layout.tooltipClip}`}
          >
            <Tooltip appearance="hud" label="Friends" shortcut="L" side="top">
              <Button size="md" variant="secondary">
                top
              </Button>
            </Tooltip>
            <Tooltip appearance="hud" variant="rail" label="Settings" shortcut="P">
              <Button size="md" variant="secondary">
                rail
              </Button>
            </Tooltip>
            <Tooltip
              appearance="hud"
              side="bottom"
              multiline
              label={'A longer hint that wraps.\n\nParagraph breaks are kept.'}
            >
              <Button size="md" variant="secondary">
                multiline
              </Button>
            </Tooltip>
            <Tooltip appearance="hud" side="bottom" align="end" label="End stream for everyone">
              <Button size="md" variant="secondary">
                bottom, end
              </Button>
            </Tooltip>
          </div>
        )
      }
    ]
  },
  {
    name: 'Spinner',
    group: 'Feedback',
    summary: 'The busy indicator.',
    components: ['Spinner'],
    examples: [
      { id: 'light', title: 'Light (dark surface)', render: () => <Spinner variant="light" /> },
      {
        id: 'thin',
        title: 'Thin, sized (Explorer HUD)',
        render: () => <Spinner variant="thin" size={26} />
      },
      {
        id: 'thick',
        title: 'Thick, sized (in-world Shop)',
        render: () => <Spinner variant="thick" size={64} />
      },
      {
        id: 'chase',
        title: 'Chase, sized (sign-in)',
        render: () => <Spinner variant="chase" size={40} />
      },
      {
        id: 'dark',
        title: 'Dark (light surface)',
        surface: 'light',
        render: () => <Spinner variant="dark" label="Loading results" />
      }
    ]
  },
  {
    name: 'Skeleton',
    group: 'Feedback',
    summary: 'A loading placeholder; the caller gives it its size.',
    components: ['Skeleton'],
    examples: [
      {
        id: 'tones',
        title: 'Brand, neutral, overlay, sweep and band tones',
        render: () => (
          <div className={layout.stack}>
            <Skeleton style={{ display: 'block', width: 280, height: 160 }} />
            <Skeleton tone="neutral" style={{ display: 'block', width: 200, height: 20 }} />
            <Skeleton tone="overlay" style={{ display: 'block', width: 256, height: 146 }} />
            <Skeleton tone="sweep" style={{ width: 126, height: 126 }} />
            <Skeleton tone="band" style={{ width: 200, height: 22 }} />
          </div>
        )
      }
    ]
  },
  {
    name: 'CountBadge',
    group: 'Feedback',
    summary: 'A small pill with a count, beside a label or inside a tab.',
    components: ['CountBadge'],
    examples: [
      {
        id: 'sizes',
        title: 'The three sizes, and a count above its maximum',
        render: () => (
          <div className={layout.row}>
            <CountBadge count={5} />
            <CountBadge count={5} size="sm" />
            <CountBadge count={5} size="xs" />
            <CountBadge count={140} max={99} />
          </div>
        )
      }
    ]
  },
  {
    name: 'EmptyState',
    group: 'Feedback',
    summary:
      'A centred icon, title and subtitle with optional actions, for an empty or failed surface.',
    components: ['EmptyState'],
    examples: [
      {
        id: 'variants',
        title: 'Inline, an error with an action, and a card with two actions',
        render: () => (
          <div className={layout.stack}>
            <EmptyState
              variant="inline"
              title="Nothing matched your search"
              subtitle="Try another name or clear the filters."
            />
            <EmptyState
              variant="inline"
              tone="error"
              title="Couldn't load the shop"
              subtitle="The request timed out."
              actions={[{ label: 'Retry', onClick: () => {} }]}
            />
            <EmptyState
              variant="card"
              iconWash
              icon={<span aria-hidden="true">☺</span>}
              title="No friends yet"
              subtitle="Add the people you meet in-world."
              actions={[
                { label: 'Find people', onClick: () => {} },
                { label: 'Later', variant: 'ghost', onClick: () => {} },
                { label: 'Invite', variant: 'outline', onClick: () => {} }
              ]}
            />
          </div>
        )
      }
    ]
  },
  {
    name: 'Panel',
    group: 'Feedback',
    summary: 'The dark surface of a HUD widget or menu.',
    components: ['Panel'],
    examples: [
      {
        id: 'surfaces',
        title: 'Solid and frosted',
        render: () => (
          <div className={layout.row}>
            <Panel style={{ padding: 18, width: 220 }}>A surface for widgets and menus.</Panel>
            <Panel blur style={{ padding: 18, width: 220 }}>
              A frosted surface over the world.
            </Panel>
          </div>
        )
      }
    ]
  },
  {
    name: 'SwapLabel',
    group: 'Feedback',
    summary: 'A button label that swaps while its button is hovered or focused.',
    components: ['SwapLabel'],
    examples: [
      {
        id: 'button',
        title: 'Hover or focus the button',
        render: () => (
          <Button appearance="hud" variant="secondary" aria-label="Cancel request">
            <SwapLabel idle="Request Sent" hover="Cancel Request" />
          </Button>
        )
      }
    ]
  },
  {
    name: 'VoiceBars',
    group: 'Feedback',
    summary: 'The speaking badge of a nametag.',
    components: ['VoiceBars'],
    examples: [
      {
        id: 'badge',
        title: 'Speaking',
        render: () => (
          <div className={layout.row}>
            <VoiceBars />
          </div>
        )
      }
    ]
  },
  {
    name: 'DclLogo',
    group: 'Feedback',
    summary: 'The Decentraland logo mark.',
    components: ['DclLogo'],
    examples: [
      {
        id: 'sizes',
        title: '24, 32 and 48 pixels',
        render: () => (
          <div className={layout.row}>
            <DclLogo />
            <DclLogo size={32} />
            <DclLogo size={48} />
          </div>
        )
      }
    ]
  },
  {
    name: 'MaskIcon',
    group: 'Feedback',
    summary: 'An image drawn in the text colour.',
    components: ['MaskIcon'],
    examples: [
      {
        id: 'colours',
        title: 'One image in three text colours and two sizes',
        render: () => (
          <div className={layout.row}>
            <MaskIcon src={MASK_EXAMPLE} />
            <span style={{ color: 'var(--dcl-brand-ruby)', display: 'inline-flex' }}>
              <MaskIcon src={MASK_EXAMPLE} />
            </span>
            <span style={{ color: 'var(--dcl-brand-orange)', display: 'inline-flex' }}>
              <MaskIcon src={MASK_EXAMPLE} size={40} />
            </span>
          </div>
        )
      }
    ]
  },
  {
    name: 'ScrollThumb',
    group: 'Feedback',
    summary: 'A pill that follows a scroller whose own scrollbar is hidden.',
    components: ['ScrollThumb'],
    examples: [
      {
        id: 'tones',
        title: 'Outline, dark and solid, beside a list that scrolls',
        render: () => (
          <div className={layout.row}>
            <ScrollThumbExample tone="outline" />
            <ScrollThumbExample tone="dark" />
            <ScrollThumbExample tone="solid" />
          </div>
        )
      }
    ]
  },
  {
    name: 'Tag',
    group: 'Feedback',
    summary: 'A short status or attribute label; some variants take a count.',
    components: ['Tag'],
    examples: [
      {
        id: 'defaults',
        title: 'Variants with their default text',
        render: () => (
          <div className={layout.row}>
            {tagVariants.map((variant) => (
              <Tag key={variant} variant={variant} />
            ))}
          </div>
        )
      },
      {
        id: 'counts',
        title: 'Counts and caller text',
        render: () => (
          <div className={layout.row}>
            <Tag variant="online" count={24} countLabel="24 players" />
            <Tag variant="streaming" count={3} countLabel="3 players" />
            <Tag variant="realm" count={128} countLabel="128 players">
              main
            </Tag>
            <Tag variant="location">Genesis Plaza</Tag>
            <Tag variant="role">Member</Tag>
            <Tag variant="role" appearance="emphasized">
              Owner
            </Tag>
          </div>
        )
      }
    ]
  },
  {
    name: 'ItemLabel',
    group: 'Feedback',
    summary: 'The small label on an item: new, smart or an amount.',
    components: ['ItemLabel'],
    examples: [
      {
        id: 'variants',
        title: 'Variants',
        render: () => (
          <div className={layout.row}>
            <ItemLabel variant="new" />
            <ItemLabel variant="smart" />
            <ItemLabel variant="amount">x2</ItemLabel>
          </div>
        )
      }
    ]
  },
  {
    name: 'ProgressBar',
    group: 'Feedback',
    summary: 'Progress towards a goal, with an optional value label and completed state.',
    components: ['ProgressBar'],
    examples: [
      {
        id: 'values',
        title: 'Empty, partial, non-linear and complete',
        render: () => (
          <div className={`${layout.stack} ${layout.field}`}>
            <ProgressBar aria-label="Invites accepted" value={0} max={3} valueLabel="0/3" />
            <ProgressBar aria-label="Invites accepted" value={1} max={3} valueLabel="1/3" />
            <ProgressBar
              aria-label="Tier progress"
              value={20}
              max={100}
              fraction={0.6}
              valueLabel="20"
            />
            <ProgressBar
              aria-label="Invites accepted"
              value={3}
              max={3}
              completedLabel="Reward unlocked"
            />
          </div>
        )
      }
    ]
  },
  {
    name: 'ConnectionStatus',
    group: 'Feedback',
    summary: 'The presence dot; ProfilePic shows it from 32px up.',
    components: ['ConnectionStatus'],
    examples: [
      {
        id: 'statuses',
        title: 'Online and away',
        render: () => (
          <div className={layout.row}>
            <ConnectionStatus status="online" />
            <ConnectionStatus status="away" />
          </div>
        )
      }
    ]
  },
  {
    name: 'ProfilePic',
    group: 'Feedback',
    summary: 'A member picture, with the initial as fallback.',
    components: ['ProfilePic'],
    examples: [
      {
        id: 'sizes',
        title: 'Sizes',
        render: () => (
          <div className={layout.row}>
            {picSizes.map((size) => (
              <ProfilePic
                key={size}
                name={people.ada.name}
                src={people.ada.picture}
                nameColor={people.ada.color}
                size={size}
              />
            ))}
          </div>
        )
      },
      {
        id: 'hud',
        title:
          'Explorer HUD: any diameter, a status dot at either corner, a framed picture and the initial',
        render: () => (
          <div className={layout.row}>
            <ProfilePic
              appearance="hud"
              name={people.ada.name}
              src={people.ada.picture}
              nameColor={people.ada.color}
              size={53}
              status="online"
            />
            <ProfilePic
              appearance="hud"
              name={people.ada.name}
              src={people.ada.picture}
              nameColor={people.ada.color}
              size={40}
              status="offline"
              dotPosition="top"
            />
            <ProfilePic
              appearance="hud"
              name={people.ada.name}
              src={people.ada.picture}
              nameColor={people.ada.color}
              size={42}
              framed
            />
            <ProfilePic
              appearance="hud"
              name="@mojito"
              nameColor={people.ada.color}
              size={36}
              status="away"
            />
          </div>
        )
      },
      {
        id: 'fallback',
        title: 'Initial fallback, compact on mobile and connection status',
        render: () => (
          <div className={layout.row}>
            <ProfilePic name={people.grace.name} nameColor={people.grace.color} size={60} />
            <ProfilePic name="Ada" src={people.ada.picture} size={76} compactOnMobile />
            <ProfilePic name="Ada" src={people.ada.picture} size={48} status="online" />
            <ProfilePic name={people.grace.name} size={32} status="away" />
          </div>
        )
      }
    ]
  }
]

/** A white-on-transparent star, as a data URI: the gallery has no image asset of its own to mask. */
const MASK_EXAMPLE =
  "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'%3E%3Cpath fill='%23fff' d='M12 2l3 7 7 .6-5.3 4.6 1.6 7L12 17.5 5.7 21.2l1.6-7L2 9.6l7-.6z'/%3E%3C/svg%3E"

function ScrollThumbExample({ tone }: { tone: 'outline' | 'dark' | 'solid' }) {
  const scroller = useRef<HTMLDivElement>(null)
  return (
    <div style={{ position: 'relative', width: 140, height: 120 }}>
      <div
        ref={scroller}
        tabIndex={0}
        role="group"
        aria-label={`A list that scrolls, ${tone} handle`}
        style={{ height: '100%', overflowY: 'auto', scrollbarWidth: 'none', paddingRight: 24 }}
      >
        {Array.from({ length: 12 }, (_, index) => (
          <p key={index} style={{ margin: 0, lineHeight: '28px' }}>
            Row {index + 1}
          </p>
        ))}
      </div>
      <ScrollThumb target={scroller} tone={tone} length={40} className={layout.thumbRight} />
    </div>
  )
}
