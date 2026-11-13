import { TitleBar, type TitleBarProps, type TitleBarSection } from '@dcl/one-ui'
const sections: readonly TitleBarSection[] = [{ id: 'graphics', label: 'Graphics', icon: <svg /> }]
const settings: TitleBarProps = {
  variant: 'settings',
  sections,
  activeId: 'graphics',
  onSelect: () => {}
}
const backpack: TitleBarProps = {
  variant: 'backpack',
  sections,
  activeId: 'graphics',
  onSelect: () => {},
  controls: { collection: null, filters: null, search: null }
}
// @ts-expect-error Search/filter controls are required for the captured Places composition.
const missing: TitleBarProps = {
  variant: 'places',
  sections,
  activeId: 'graphics',
  onSelect: () => {}
}
const invented: TitleBarProps = {
  variant: 'settings',
  sections,
  activeId: 'graphics',
  onSelect: () => {},
  // @ts-expect-error Settings has no captured right-side control region.
  controls: { search: null }
}
// @ts-expect-error Native section identity and visible name are required.
const unnamed: TitleBarSection = { id: 'empty', icon: null }
export const fixtures = { settings, backpack, missing, invented, unnamed }
export const rendered = <TitleBar {...settings} />
