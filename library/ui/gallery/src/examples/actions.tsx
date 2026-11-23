import {
  Button,
  CardActionButton,
  CloseButton,
  CopyFeedback,
  FriendCTA,
  GeneralAdd,
  GeneralCopy,
  GeneralRightArrow,
  MenusShare,
  GeneralStar,
  JumpInCTA,
  JumpInIcon,
  type FriendCTAStatus
} from '@dcl/one-ui'
import type { GalleryEntry } from '../registry'
import layout from './layout.module.css'

const variants = [
  'primary',
  'secondary',
  'outline',
  'dark',
  'text',
  'tertiary',
  'quaternary'
] as const
const sizes = ['xl', 'lg', 'md', 'sm'] as const
const friendStatuses: FriendCTAStatus[] = ['add', 'sent', 'friend', 'reject', 'blocked']

export const actionEntries: GalleryEntry[] = [
  {
    name: 'Button',
    group: 'Actions',
    summary: 'Native button, or an anchor with href.',
    components: ['Button'],
    examples: [
      {
        id: 'variants',
        title: 'Variants',
        note: 'tertiary and quaternary are aliases of outline and dark.',
        render: () => (
          <div className={layout.row}>
            {variants.map((variant) => (
              <Button key={variant} variant={variant} size="md">
                {variant}
              </Button>
            ))}
          </div>
        )
      },
      {
        id: 'light',
        title: 'Light-surface variants',
        surface: 'light',
        render: () => (
          <div className={layout.row}>
            <Button variant="outlineOnLight" size="md">
              outlineOnLight
            </Button>
            <Button variant="inverse" size="md">
              inverse
            </Button>
          </div>
        )
      },
      {
        id: 'sizes',
        title: 'Sizes, desktop and mobile',
        render: () => (
          <div className={layout.stack}>
            {(['desktop', 'mobile'] as const).map((viewport) => (
              <div key={viewport} className={layout.row}>
                {sizes.map((size) => (
                  <Button key={size} size={size} viewport={viewport}>
                    {`${size} ${viewport}`}
                  </Button>
                ))}
              </div>
            ))}
          </div>
        )
      },
      {
        id: 'states',
        title: 'States',
        render: () => (
          <div className={layout.row}>
            <Button size="md" forceState="hover">
              Hover
            </Button>
            <Button size="md" forceState="pressed">
              Pressed
            </Button>
            <Button size="md" forceState="focus">
              Focus
            </Button>
            <Button size="md" disabled>
              Disabled
            </Button>
            <Button size="md" aria-disabled="true">
              aria-disabled
            </Button>
            <Button size="md" loading>
              Loading
            </Button>
          </div>
        )
      },
      {
        id: 'content',
        title: 'Icons, links and icon-only',
        render: () => (
          <div className={layout.stack}>
            <div className={layout.row}>
              <Button size="md" leadingIcon={<GeneralAdd size={20} aria-hidden="true" />}>
                Leading icon
              </Button>
              <Button
                size="md"
                variant="secondary"
                trailingIcon={<GeneralRightArrow size={20} aria-hidden="true" />}
              >
                Trailing icon
              </Button>
              <Button size="md" variant="outline" href="#docs">
                Link
              </Button>
              <Button size="md" variant="iconOnly" aria-label="Share">
                <MenusShare size={20} aria-hidden="true" />
              </Button>
            </div>
            <div className={layout.narrow}>
              <Button
                size="md"
                contentLayout="fill"
                style={{ width: '100%' }}
                leadingIcon={<GeneralStar size={20} aria-hidden="true" />}
                trailingIcon={<GeneralRightArrow size={20} aria-hidden="true" />}
              >
                Fill layout
              </Button>
            </div>
          </div>
        )
      }
    ]
  },
  {
    name: 'CardActionButton',
    group: 'Actions',
    summary: 'The round icon action on a card.',
    components: ['CardActionButton'],
    examples: [
      {
        id: 'states',
        title: 'Default glyph, custom glyph and disabled',
        render: () => (
          <div className={layout.row}>
            <CardActionButton aria-label="Like" />
            <CardActionButton aria-label="Share" icon={<MenusShare size={24} />} />
            <CardActionButton aria-label="Like" disabled />
          </div>
        )
      }
    ]
  },
  {
    name: 'CloseButton',
    group: 'Actions',
    summary: 'The icon-only close control.',
    components: ['CloseButton'],
    examples: [
      {
        id: 'variants',
        title: 'Variants',
        render: () => (
          <div className={layout.row}>
            <CloseButton variant="solid" />
            <CloseButton variant="filled" />
            <CloseButton variant="plain" aria-label="Dismiss" />
          </div>
        )
      },
      {
        id: 'light',
        title: 'On a light surface',
        surface: 'light',
        render: () => (
          <div className={layout.row}>
            <CloseButton variant="solid" />
            <CloseButton variant="filled" />
            <CloseButton variant="plain" />
          </div>
        )
      }
    ]
  },
  {
    name: 'JumpInCTA',
    group: 'Actions',
    summary: 'JUMP IN as a button or a link, and its icon.',
    components: ['JumpInCTA', 'JumpInIcon'],
    examples: [
      {
        id: 'sizes',
        title: 'Sizes',
        render: () => (
          <div className={layout.row}>
            {sizes.map((size) => (
              <JumpInCTA key={size} size={size} />
            ))}
          </div>
        )
      },
      {
        id: 'states',
        title: 'Link, disabled and loading',
        render: () => (
          <div className={layout.row}>
            <JumpInCTA size="md" href="#jump" label="Jump in" />
            <JumpInCTA size="md" disabled />
            <JumpInCTA size="md" loading />
          </div>
        )
      },
      {
        id: 'icon',
        title: 'JumpInIcon',
        render: () => (
          <div className={layout.row}>
            <JumpInIcon />
          </div>
        )
      }
    ]
  },
  {
    name: 'FriendCTA',
    group: 'Actions',
    summary: 'The friendship action, one look per status.',
    components: ['FriendCTA'],
    examples: [
      {
        id: 'statuses',
        title: 'Statuses (desktop)',
        render: () => (
          <div className={layout.row}>
            {friendStatuses.map((status) => (
              <FriendCTA key={status} status={status} />
            ))}
          </div>
        )
      },
      {
        id: 'mobile',
        title: 'Mobile size and loading',
        render: () => (
          <div className={layout.row}>
            <FriendCTA status="add" viewport="mobile" />
            <FriendCTA status="sent" viewport="mobile" loading />
          </div>
        )
      }
    ]
  },
  {
    name: 'CopyFeedback',
    group: 'Actions',
    summary: 'Copies a value and reports the outcome: a tooltip, an inline swap or the label.',
    components: ['CopyFeedback'],
    examples: [
      {
        id: 'tooltip',
        title: 'Tooltip feedback',
        note: 'Press the control; the result depends on the browser clipboard permission.',
        render: () => (
          <CopyFeedback value="0x0000000000000000000000000000000000000000">
            {(copy) => (
              <Button
                size="md"
                variant="secondary"
                onClick={copy}
                leadingIcon={<GeneralCopy size={20} aria-hidden="true" />}
              >
                Copy address
              </Button>
            )}
          </CopyFeedback>
        )
      },
      {
        id: 'inline',
        title: 'Inline value',
        render: () => (
          <CopyFeedback value="0x1234" inlineValue={<span>0x1234…abcd</span>}>
            {(copy) => (
              <Button size="sm" variant="iconOnly" aria-label="Copy user ID" onClick={copy}>
                <GeneralCopy size={20} aria-hidden="true" />
              </Button>
            )}
          </CopyFeedback>
        )
      },
      {
        id: 'label',
        title: 'Feedback through the label',
        render: () => (
          <CopyFeedback value="https://example.com/place" showTooltip={false}>
            {(copy, state) => (
              <Button size="md" variant="outline" onClick={copy}>
                {state === 'copied' ? 'Copied' : state === 'failed' ? 'Copy failed' : 'Copy link'}
              </Button>
            )}
          </CopyFeedback>
        )
      }
    ]
  }
]
