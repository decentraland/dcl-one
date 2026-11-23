// The Showcase's sections, in page order: the name a section opens by (the UI gallery's ?hud=<name>),
// its heading, and the components of design/index.ts it renders. Showcase.tsx renders each one by
// name. Kept apart from those renders, so a page can list the sections without loading the HUD (the
// gallery's sidebar), and the gallery's coverage test (library/ui/gallery) checks that every
// component design/index.ts exports is named by a section that mounts it, and that every other
// export has its reason in SHOWCASE_NOT_COMPONENTS below. The glyphs (Glyphs.tsx)
// are the HUD's icon set, named nowhere: the gallery draws every one on a page of its own, so the
// Showcase keeps none the HUD does not use in the HUD's design chunk.

export interface ShowcaseEntry {
  name: string
  title: string
  /** The components of design/index.ts the section renders, glyphs aside. */
  components: readonly string[]
}

export const SHOWCASE_INDEX = [
  { name: 'Palette', title: 'Palette (tokens)', components: [] },
  { name: 'RarityColors', title: 'Rarity name colors', components: [] },
  { name: 'Typography', title: 'Typography (Inter)', components: [] },
  { name: 'Radii', title: 'Radii', components: [] },
  { name: 'Button', title: 'Button', components: ['Button'] },
  { name: 'ArmedButton', title: "ArmedButton (a confirm that spends or gives away what can't be taken back: SEND GIFT, SEND TIP, BUY ITEM)", components: ['ArmedButton', 'Button'] },
  { name: 'StartButton', title: "Button (start: the sign-in screens' CTA)", components: ['Button'] },
  { name: 'SignIn', title: 'Sign-in: SubmitField, method buttons, CodeInput, chase spinner', components: ['SubmitField', 'Button', 'CodeInput', 'Spinner', 'Checkbox'] },
  { name: 'LogoPattern', title: 'Logo pattern (the loading and sign-in backdrop)', components: ['LogoPattern'] },
  { name: 'BareButton', title: 'Button (bare: a translucent fill, sized by the caller)', components: ['Button'] },
  { name: 'Skeleton', title: 'Skeleton', components: ['Skeleton'] },
  { name: 'Dialogs', title: 'Dialogs (imperative popup layer — showDialog / showConfirm)', components: ['PopupHost', 'Button'] },
  { name: 'Modal', title: 'Modal (the dialog card: ModalShell, or Modal with ModalTitle and ModalActions)', components: ['Modal', 'ModalTitle', 'ModalActions', 'ModalShell', 'Button'] },
  { name: 'Tooltip', title: 'Tooltip (hover the chips)', components: ['Tooltip'] },
  { name: 'ControlButton', title: 'ControlButton', components: ['ControlButton'] },
  { name: 'Tabs', title: 'Tabs (pill · underline — arrows/Home/End move the selection)', components: ['Tabs'] },
  { name: 'FormControls', title: 'Form controls', components: ['Toggle', 'Slider', 'TextInput', 'TextArea', 'DateField', 'Select'] },
  { name: 'FieldLabel', title: 'FieldLabel and CharCounter (a labelled field and its count)', components: ['FieldLabel', 'CharCounter', 'TextInput'] },
  { name: 'SearchField', title: 'SearchField (dark · light · small, clearable)', components: ['SearchField'] },
  { name: 'Dropdown', title: 'Dropdown (a sort or filter list)', components: ['Dropdown'] },
  { name: 'EmoteSlot', title: 'Emote Wheel Slot (Figma 10386-4701)', components: [] },
  { name: 'HudScale', title: 'DPI · HUD scale', components: ['Select'] },
  { name: 'MenuPanel', title: "MenuPanel (a navigation button's panel)", components: ['MenuPanel', 'MenuList', 'MenuRow', 'MenuDivider', 'Icon', 'CountBadge'] },
  { name: 'IconButton', title: 'IconButton (navigation bar)', components: ['IconButton', 'Panel'] },
  { name: 'VoiceBars', title: 'VoiceBars (speaking)', components: ['VoiceBars'] },
  { name: 'Chip', title: 'Chip (breadcrumb filter)', components: ['Chip'] },
  { name: 'OptionMenu', title: 'OptionMenu', components: ['OptionMenu'] },
  { name: 'ContextMenu', title: 'ContextMenu (items · submenu · exclusive options)', components: ['ContextMenu'] },
  { name: 'Submenu', title: 'Submenu (a menu row that opens a list beside it: hover or press it)', components: ['Submenu'] },
  { name: 'AnchoredMenu', title: 'AnchoredMenu (a ContextMenu opened from a button, as the photo options use it)', components: ['AnchoredMenu', 'Button'] },
  { name: 'MaskIcon', title: 'MaskIcon (a picture drawn in the text colour)', components: ['MaskIcon'] },
  { name: 'SwapLabel', title: 'SwapLabel (hover the button)', components: ['SwapLabel', 'Button'] },
  { name: 'CountBadge', title: 'CountBadge (count · capped)', components: ['CountBadge'] },
  { name: 'KebabMenu', title: 'KebabMenu (md · sm)', components: ['KebabMenu'] },
  { name: 'HintsButton', title: 'HintsButton (opens upward) and KeyCap', components: ['HintsButton', 'KeyCap'] },
  { name: 'Rail', title: 'Rail', components: ['Rail'] },
  { name: 'HeaderButton', title: 'HeaderButton', components: ['HeaderButton'] },
  { name: 'CardButton', title: 'Button (card sizes)', components: ['Button'] },
  { name: 'VerifiedBadge', title: 'VerifiedBadge', components: [] },
  { name: 'Pager', title: 'Pager', components: ['Pager'] },
  { name: 'BrowsePage', title: 'Browse page (BrowseToolbar · BrowseControl · BrowsePanel · BrowseLoading)', components: ['BrowseToolbar', 'BrowseControl', 'BrowsePanel', 'BrowseLoading', 'Tabs', 'SearchField', 'Dropdown', 'EmptyState'] },
  { name: 'DiscoverCard', title: 'DiscoverCard (Places and Events) in a DiscoverGrid', components: ['DiscoverCard', 'DiscoverGrid'] },
  { name: 'EmptyState', title: 'EmptyState (inline · error with an action · card)', components: ['EmptyState'] },
  { name: 'PageHeader', title: 'PageHeader (default · hero · section)', components: ['PageHeader', 'Button'] },
  { name: 'ColorPicker', title: 'ColorPicker (avatar colors)', components: ['ColorPicker'] },
  { name: 'NameColorPicker', title: 'ColorPicker (swatch, hue only: name colour)', components: ['ColorPicker'] },
  { name: 'Panel', title: 'Panel', components: ['Panel'] },
  { name: 'Brand', title: 'Brand', components: ['DclLogo'] },
  { name: 'Avatar', title: 'Avatar', components: ['Avatar'] },
  { name: 'WearableCard', title: 'WearableCard (backpack)', components: ['WearableCard'] },
  { name: 'WearableCardPick', title: 'WearableCard pick (gift picker)', components: ['WearableCard'] },
  { name: 'EquippedItemCard', title: 'EquippedItemCard (passport)', components: ['EquippedItemCard', 'Icon'] },
  { name: 'ScrollThumb', title: 'ScrollThumb (fixed length, as the Shop draws it)', components: ['ScrollThumb'] },
  { name: 'Toast', title: 'Toast (plain · error · keyed progress · action + dismiss)', components: ['ToastHost', 'Button'] },
  { name: 'Chat', title: 'Chat components', components: [] }
] as const satisfies readonly ShowcaseEntry[]

export type ShowcaseName = (typeof SHOWCASE_INDEX)[number]['name']

/** A component design/index.ts exports that no section renders, with the reason. */
export const SHOWCASE_EXCLUSIONS: readonly { name: string; reason: string }[] = []

/** The exports of index.ts that are not components, each with why no section shows it. Types are erased at
 *  runtime and not listed. */
export const SHOWCASE_NOT_COMPONENTS: Readonly<Record<string, string>> = {
  ARM_MS: 'a number: how long ArmedButton ignores presses after it appears',
  anchoredMenuPosition: 'where AnchoredMenu opens, a function',
  closeTopPopup: 'the popup layer API, which PopupHost draws',
  dismissToast: 'the toast queue API, which ToastHost draws',
  hasOpenPopup: 'the popup layer API, which PopupHost draws',
  openPopup: 'the popup layer API, which PopupHost draws',
  pageWindow: 'the page numbers Pager shows, a function',
  rarityOf: 'a rarity art lookup the cards use',
  rarityPanel: 'a rarity art lookup the cards use',
  rarityTile: 'a rarity art lookup the cards use',
  resetPopups: 'the popup layer API, which PopupHost draws',
  showConfirm: 'opens a dialog on the popup layer: the Dialogs section calls it',
  showDialog: 'opens a dialog on the popup layer: the Dialogs section calls it',
  showToast: 'shows a toast: the Toast section calls it',
  subscribePopups: 'the popup layer API, which PopupHost draws',
  topPopupOrder: 'the popup layer API, which PopupHost draws'
}
