// Modal family — the shared ModalCard and its parts under the names the HUD's dialogs use: Modal (the card),
// ModalTitle (its header), ModalActions (its footer) and ModalShell (the three laid out). The card is
// presentational: the overlay (scrim, DPI scale, entrance animation, focus trap, Escape) is owned by whoever
// hosts it, PopupHost for popup-layer dialogs or a self-contained surface like CrashModal.

export { ModalCard as Modal, ModalActions, ModalShell, ModalTitle } from '@dcl/one-ui'
