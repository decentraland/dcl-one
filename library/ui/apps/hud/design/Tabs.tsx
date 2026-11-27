// Tabs — the HUD's tab strip, drawn by the shared TabsBar and Tabs (`appearance="hud"`). Two families of
// looks: pills (`pill`, `section`, `shop`) and labels on a rule (`underline`, `subtab`). Controlled: `value`
// + `onChange`. WAI-ARIA tablist/tab with a roving tabindex — Left/Right/Home/End move focus AND select, so a
// strip is keyboard-operable (the panels it heads are not `tabpanel`s; the strip only owns selection).

import { CountBadge, Tabs as SharedTab, TabsBar } from '@dcl/one-ui'

export interface TabItem<T extends string = string> {
  id: T
  label: React.ReactNode
  /** Rendered before the label. */
  icon?: React.ReactNode
  /** Rendered after the label. */
  iconAfter?: React.ReactNode
  /** Count pill after the label; hidden when 0/undefined. */
  badge?: number
  /** Above this the pill reads "+max". */
  badgeMax?: number
  /** Shown but not selectable — e.g. a static heading standing in the strip. */
  disabled?: boolean
}

export interface TabsProps<T extends string> {
  items: readonly TabItem<T>[]
  value: T
  onChange: (id: T) => void
  variant?: 'pill' | 'underline' | 'section' | 'shop' | 'subtab'
  /** Equal-width tabs filling the strip, labels centred (the friends panel header). */
  fill?: boolean
  /** Positional overrides only (margins, padding, gap); the look stays the variant's. */
  className?: string
  'aria-label'?: string
}

export function Tabs<T extends string>({
  items,
  value,
  onChange,
  variant = 'pill',
  fill = false,
  className,
  'aria-label': ariaLabel
}: TabsProps<T>): React.JSX.Element {
  return (
    <TabsBar appearance="hud" variant={variant} fill={fill} className={className} label={ariaLabel ?? ''}>
      {items.map((it) => (
        <SharedTab
          key={it.id}
          appearance="hud"
          selected={it.id === value}
          disabled={it.disabled}
          onClick={(event) => {
            // A pointer click (`detail` 1 or more) leaves no focus on the tab: the next key press is the game's, and
            // Chrome would ring the tab on it. Enter, Space and the arrows (`detail` 0) keep the keyboard's focus.
            if (event.detail > 0) event.currentTarget.blur()
            onChange(it.id)
          }}
          label={
            <>
              {it.icon}
              {it.label}
              {it.iconAfter}
              {it.badge ? <CountBadge count={it.badge} max={it.badgeMax} size={fill ? 'xs' : 'sm'} announce /> : null}
            </>
          }
        />
      ))}
    </TabsBar>
  )
}
