// The in-world Shop's own UI: presentational components and their styles, with no data (queries,
// fetchers) and nothing of the HUD's runtime (session, engine, menu). The page (../InWorldShop) reads the
// data and wires them to the HUD (docs/ui-standards.md, App-specific UI). What it may import is checked by
// apps/ui3/src/play/test/shopUiBoundary.test.ts, with the tests of /play, its host.

export { AvatarNametag, NAMETAG_PLACEHOLDER } from './AvatarNametag'
export { EmoteControls } from './EmoteControls'
export { buyableInWorld, PriceTag } from './PriceTag'
export { ShopFilters, type ShopFilterPatch, type ShopFilterStatus, type ShopFilterValues } from './ShopFilters'
export { ShopHome, type ShopHomeOutfit, type ShopHomeOutfitOffer, type ShopHomeRow } from './ShopHome'
export { ShopCardActions } from './ShopCardActions'
export { ShopItemPanel, type ShopItemHide, type ShopItemPanelProps } from './ShopItemPanel'
export { CreditsPill } from './CreditsPill'
export { ShopItemTile, type ShopItemTileProps } from './ShopItemTile'
export { ShopModal, ShopModalHeader, type ShopModalProps } from './ShopModal'
export { ShopItemCard, type ShopItemCardProps } from './ShopItemCard'
export { ShopNameCard, type ShopNameCardProps } from './ShopNameCard'
export { ShopNameTile, type ShopNameTileProps } from './ShopNameTile'
export { ShopNames, type ShopNameStatus, type ShopNamesProps, type ShopNamesTab } from './ShopNames'
export { pressedWith, type PressedWith } from './pressedWith'
export {
  ShopPurchaseDialog,
  type ShopCreditsChoice,
  type ShopManaChoice,
  type ShopPaymentMethod,
  type ShopPaymentProgress,
  type ShopPurchaseDialogProps,
  type ShopPurchaseItem,
  type ShopPurchaseStep
} from './ShopPurchaseDialog'
export { CreditsIcon } from './CreditsIcon'
export { ShopCart, ShopCartBadge, ShopCartToast, type ShopCartLine, type ShopCartProps } from './ShopCart'
