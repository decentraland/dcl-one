import {
  Button,
  BarButton,
  CardActionButton,
  Chip,
  CloseButton,
  CopyFeedback,
  FriendCTA,
  GeneralAdd,
  GeneralBell,
  GeneralCopy,
  GeneralRightArrow,
  HintsButton,
  KeyCap,
  MenusNotifications,
  MenusPlayers,
  MenusSettings,
  MenusShare,
  GeneralStar,
  JumpInCTA,
  JumpInIcon,
  ProfilePic,
  RailButton,
  type FriendCTAStatus
} from '@dcl/one-ui'
import type { GalleryEntry } from '../registry'
import { people } from './data'
import layout from './layout.module.css'

const variants = [
  'primary',
  'secondary',
  'secondaryLight',
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
        id: 'inline',
        title: 'Inline in running text',
        note: 'inline takes the surrounding font and colour, keeps its case and underlines on hover.',
        render: () => (
          <p style={{ margin: 0 }}>
            Created by <Button variant="inline">AudioHotLab</Button> on Genesis City.
          </p>
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
        id: 'hud',
        title: 'Explorer HUD',
        render: () => (
          <div className={layout.rows}>
            <div className={layout.strip}>
              {(
                ['primary', 'accent', 'secondary', 'ghost', 'dark', 'tint', 'darkOutline'] as const
              ).map((variant) => (
                <Button key={variant} appearance="hud" variant={variant}>
                  {variant}
                </Button>
              ))}
              <Button appearance="hud" disabled>
                Disabled
              </Button>
            </div>
            <div className={layout.strip}>
              {(
                [
                  'sm',
                  'md',
                  'lg',
                  'row',
                  'card',
                  'header',
                  'modal',
                  'dialog',
                  'cta',
                  'start'
                ] as const
              ).map((size) => (
                <Button key={size} appearance="hud" size={size}>
                  {size}
                </Button>
              ))}
            </div>
            <div className={layout.strip}>
              <Button appearance="hud" variant="light">
                light
              </Button>
              <Button appearance="hud" variant="outline">
                outline
              </Button>
              <Button appearance="hud" variant="whisper" size="start">
                whisper
              </Button>
              <Button appearance="hud" variant="method" size="start">
                method
              </Button>
              <Button appearance="hud" size="row" variant="secondary">
                row
              </Button>
              <Button appearance="hud" size="sm" radius="md">
                radius md
              </Button>
            </div>
            <div className={layout.strip}>
              {(
                [
                  'scrim',
                  'faint',
                  'veil',
                  'glass',
                  'shade',
                  'primary',
                  'clear',
                  'navigate',
                  'navigateExit'
                ] as const
              ).map((variant) => (
                <Button
                  key={variant}
                  appearance="hud"
                  size="bare"
                  variant={variant}
                  style={{ height: 46, padding: '0 20px' }}
                >
                  {variant}
                </Button>
              ))}
            </div>
          </div>
        )
      },
      {
        id: 'hud-controls',
        title: 'Explorer HUD controls',
        render: () => (
          <div className={layout.rows}>
            <div className={layout.strip}>
              {(['ghost', 'solid', 'dark', 'faint'] as const).map((variant) => (
                <Button
                  key={variant}
                  appearance="hud"
                  control="square"
                  variant={variant}
                  aria-label={`${variant} control`}
                >
                  ×
                </Button>
              ))}
              <Button appearance="hud" control="square" variant="solid" active aria-label="Active">
                ×
              </Button>
              <Button appearance="hud" control="square" disabled aria-label="Disabled">
                ×
              </Button>
            </div>
            <div className={layout.strip}>
              {(['sm', 'md', 'lg'] as const).map((size) => (
                <Button
                  key={size}
                  appearance="hud"
                  control="circle"
                  variant="solid"
                  size={size}
                  aria-label={`${size} circle`}
                >
                  ×
                </Button>
              ))}
              <Button appearance="hud" control="pill" variant="dark">
                12 online
              </Button>
            </div>
            <div className={layout.strip}>
              <Button appearance="hud" control="header" aria-label="Close">
                ×
              </Button>
              <Button appearance="hud" control="headerPill" style={{ padding: '0 16px' }}>
                Ada
              </Button>
            </div>
          </div>
        )
      },
      {
        id: 'content',
        title: 'Icons, links and icon-only',
        render: () => (
          <div className={layout.stack}>
            <div className={layout.row}>
              <Button
                variant="secondaryLight"
                size="md"
                leadingIcon={<GeneralBell size={20} aria-hidden="true" />}
              >
                Remind me
              </Button>
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
    name: 'Chip',
    group: 'Actions',
    summary: 'A filter chip: a toggle with an optional icon and an optional clear button.',
    components: ['Chip'],
    examples: [
      {
        id: 'variants',
        title: 'Light and dark, at rest, selected, and selected with a clear button',
        render: () => (
          <div className={layout.rows}>
            <div className={layout.strip}>
              <Chip label="All" />
              <Chip label="Hats" selected />
              <Chip label="Hats" selected onClear={() => {}} clearLabel="Clear Hats" />
              <Chip
                label="Starred"
                icon={<GeneralStar size={24} aria-hidden="true" />}
                selected
                onClear={() => {}}
                clearLabel="Clear Starred"
              />
            </div>
            <div className={layout.strip}>
              <Chip variant="dark" label="Wearables" />
              <Chip
                variant="dark"
                label="Emotes"
                icon={<GeneralStar size={24} aria-hidden="true" />}
                selected
              />
            </div>
          </div>
        )
      }
    ]
  },
  {
    name: 'BarButton',
    group: 'Actions',
    summary: 'A button of the Explorer HUD top bar, over a scene or a backdrop.',
    components: ['BarButton'],
    examples: [
      {
        id: 'shapes',
        title: 'Pill, round, open with a badge, large round and disabled',
        render: () => (
          <div className={layout.row}>
            <BarButton>Map</BarButton>
            <BarButton shape="round" aria-label="Share">
              <MenusShare size={24} />
            </BarButton>
            <BarButton
              shape="round"
              open
              badge={3}
              aria-label="Notifications, 3 unread"
              aria-expanded
            >
              <MenusShare size={24} />
            </BarButton>
            <BarButton shape="round" size="lg" aria-label="Close">
              <MenusShare size={24} />
            </BarButton>
            <BarButton disabled>Places</BarButton>
          </div>
        )
      }
    ]
  },
  {
    name: 'RailButton',
    group: 'Actions',
    summary:
      'A round button of the Explorer HUD navigation rail, with a selected ring, a count, a status dot and a tip.',
    components: ['RailButton'],
    examples: [
      {
        id: 'states',
        title:
          'Rest with a shortcut, a count, a lavender count above its maximum, selected, a status dot, a corner picture, a portrait and disabled',
        note: 'Point at a button for its tip. The selected one is quiet: its panel would stand where the tip does.',
        render: () => (
          <div
            className={`${layout.row} ${layout.tooltipHost} ${layout.tooltipRoom} ${layout.tooltipClip}`}
          >
            <RailButton icon={<MenusPlayers size={24} />} label="Friends" shortcut="L" />
            <RailButton icon={<MenusNotifications size={24} />} label="Notifications" badge={3} />
            <RailButton
              icon={<MenusShare size={24} />}
              label="Hangouts"
              badge={12}
              badgeMax={9}
              badgeTone="lavender"
            />
            <RailButton icon={<MenusSettings size={24} />} label="Settings" active quiet />
            <RailButton
              icon={<MenusPlayers size={24} />}
              label="Nearby voice"
              tooltip="Connected, 3 people nearby"
              indicator
            />
            <RailButton
              icon={<MenusShare size={24} />}
              label="Communities"
              cornerArt={{ src: people.grace.picture, label: 'A community stream is live' }}
            />
            <RailButton
              portrait
              icon={
                <ProfilePic
                  appearance="hud"
                  name={people.ada.name}
                  src={people.ada.picture}
                  nameColor={people.ada.color}
                  size={42}
                  framed
                />
              }
              label="Me"
            />
            <RailButton icon={<MenusSettings size={24} />} label="Shop" disabled />
          </div>
        )
      }
    ]
  },
  {
    name: 'HintsButton',
    group: 'Actions',
    summary:
      'A dark square button that opens a list of control hints above it, and the key outline its rows show.',
    components: ['HintsButton', 'KeyCap'],
    minHeight: 320,
    examples: [
      {
        id: 'hints',
        title: 'Press it for its hints; a press outside closes them',
        render: () => (
          <div className={layout.row} style={{ paddingTop: 140 }}>
            <HintsButton
              hints={[
                { icon: <KeyCap label="1" />, text: 'Press a number key to pick a slot.' },
                { icon: <KeyCap label="E" />, text: 'Press E to open the emote wheel.' }
              ]}
            />
            <HintsButton
              label="Camera hints"
              icon={<GeneralStar size={22} aria-hidden="true" />}
              hints={[{ icon: <KeyCap label="V" />, text: 'Press V to change the camera.' }]}
            />
          </div>
        )
      },
      {
        id: 'keycap',
        title: 'KeyCap alone, in a line of text',
        render: () => (
          <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
            Press <KeyCap label="U" /> to hide the interface.
          </span>
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
        title: 'Default glyph, custom glyph, disabled and small',
        render: () => (
          <div className={layout.row}>
            <CardActionButton aria-label="Like" />
            <CardActionButton aria-label="Share" icon={<MenusShare size={24} />} />
            <CardActionButton aria-label="Like" disabled />
            <CardActionButton aria-label="Share" size="sm" icon={<MenusShare size={24} />} />
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
        id: 'hud',
        title: 'Explorer HUD appearance, large and small',
        render: () => (
          <div className={layout.row}>
            <JumpInCTA appearance="hud" size="xl" label="JUMP IN" style={{ width: 240 }} />
            <JumpInCTA appearance="hud" size="md" label="JUMP IN" style={{ width: 200 }} />
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
