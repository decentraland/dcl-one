import { useState } from 'react'
import {
  Button,
  ConnectionStatus,
  ItemLabel,
  Modal,
  ProgressBar,
  ProfilePic,
  Skeleton,
  Spinner,
  Tag,
  Toast,
  Tooltip,
  type ModalProps,
  type ProfilePicSize,
  type TagVariant,
  type ToastVariant
} from '@dcl/one-ui'
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

// A dialog is rendered only while it is open; the trigger opens it again after it closes.
function ModalDemo({ open: initiallyOpen = false, trigger, actions, ...props }: ModalDemoProps) {
  const [open, setOpen] = useState(initiallyOpen)
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
    examples: toastVariants.map((variant) => ({
      id: variant,
      title: variant[0]!.toUpperCase() + variant.slice(1),
      render: () => <ToastDemo variant={variant} />
    }))
  },
  {
    name: 'Tooltip',
    group: 'Feedback',
    summary: 'A short label or note beside its anchor, shown on hover and keyboard focus.',
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
        title: 'Brand, neutral and overlay tones',
        render: () => (
          <div className={layout.stack}>
            <Skeleton style={{ display: 'block', width: 280, height: 160 }} />
            <Skeleton tone="neutral" style={{ display: 'block', width: 200, height: 20 }} />
            <Skeleton tone="overlay" style={{ display: 'block', width: 256, height: 146 }} />
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
