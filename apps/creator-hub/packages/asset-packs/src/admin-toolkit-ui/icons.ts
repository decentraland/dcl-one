import { getAssetsUrl } from './constants';

// Line icons for the admin toolkit, rasterized to white glyphs (tinted via
// uiBackground.color). Sources live in
// packs/smart_items/assets/admin_toolkit/assets/icons/reskin.
export type IconName =
  | 'users'
  | 'tv'
  | 'bolt'
  | 'message'
  | 'chevron'
  | 'play'
  | 'broadcast'
  | 'clock'
  | 'mic'
  | 'copy'
  | 'eye'
  | 'volume'
  | 'presentation'
  | 'star'
  | 'power'
  | 'refresh'
  | 'plus'
  | 'shield'
  | 'ban'
  | 'eyeoff'
  | 'trash'
  | 'send'
  | 'help'
  | 'pause'
  | 'loop'
  | 'arrowL'
  | 'arrowR'
  | 'stop';

export function icon(name: IconName): string {
  return `${getAssetsUrl()}/icons/reskin/${name}.png`;
}
