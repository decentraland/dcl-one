// SystemPanel — the panel the System button opens: the settings and, under a rule, the ways to get
// help. The bar places it and closes it; the page actions and their keys come in by props, and a
// row whose action the bar does not pass is not drawn. The help rows open their page in a new tab.

import { Icon, MenuDivider, MenuList, MenuPanel, MenuRow } from '../../../design'
import { bugReportUrl } from '@explorer-core/lib/bugReport'
import { siteUrl } from '@hud-host'
import settingsIcon from '../../../assets/sidebar-rail/system-settings.svg?no-inline'
import controlsIcon from '../../../assets/sidebar-rail/system-controls.svg?no-inline'
import faqIcon from '../../../assets/sidebar-rail/system-faq.svg?no-inline'
import reportBugIcon from '../../../assets/sidebar-rail/system-report-bug.svg?no-inline'
import supportIcon from '../../../assets/sidebar-rail/system-support.svg?no-inline'
import discordIcon from '../../../assets/sidebar-rail/system-discord.svg?no-inline'

export interface SystemPanelProps {
  /** The key bound to the Settings page, drawn as "[P]" after the label. */
  settingsKey?: string
  /** The key bound to the key and mouse controls. */
  controlsKey?: string
  onSettings: () => void
  /** Open the settings on the key and mouse controls. */
  onControls?: () => void
  /** Open the scene editor, where this build offers it. */
  onCreate?: () => void
  /** Called after a row's action, so the bar closes the panel. */
  onClose: () => void
}

/** The community's Discord server, as the web pages link it. */
export const DISCORD_URL = 'https://dcl.gg/discord'

// Each picture is the design's icon centred in the row's 28px box.
const art = (src: string): React.JSX.Element => <img src={src} alt="" width={28} height={28} draggable={false} />

const openTab = (url: string): void => {
  window.open(url, '_blank', 'noopener')
}

export function SystemPanel({ settingsKey, controlsKey, onSettings, onControls, onCreate, onClose }: SystemPanelProps): React.JSX.Element {
  const pick = (act: () => void) => (): void => {
    act()
    onClose()
  }
  return (
    <MenuPanel label="System">
      <MenuList>
        <MenuRow icon={art(settingsIcon)} label="Settings" hint={settingsKey} onClick={pick(onSettings)} />
        {onControls != null && <MenuRow icon={art(controlsIcon)} label="Mouse / Key Controls" hint={controlsKey} onClick={pick(onControls)} />}
        {onCreate != null && <MenuRow icon={<Icon name="create" size={22} />} label="Create" onClick={pick(onCreate)} />}
      </MenuList>
      <MenuDivider />
      <MenuList>
        <MenuRow icon={art(faqIcon)} label="FAQ" onClick={pick(() => openTab(siteUrl('/help/')))} />
        <MenuRow icon={art(reportBugIcon)} label="Report Bug" onClick={pick(() => openTab(bugReportUrl()))} />
        <MenuRow icon={art(supportIcon)} label="Contact Support" onClick={pick(() => openTab(siteUrl('/help/')))} />
        <MenuRow icon={art(discordIcon)} label="Discord" onClick={pick(() => openTab(DISCORD_URL))} />
      </MenuList>
    </MenuPanel>
  )
}
