// Modal family — the shared ModalCard and its parts under the names the HUD's dialogs use: Modal (the card),
// ModalTitle (its header), ModalActions (its footer) and ModalShell (the three laid out). The card is
// presentational: the overlay is owned by whoever hosts it, PopupHost for popup-layer dialogs or a self-contained
// surface like CrashModal. Both draw the scrim, its DPI scale and the entrance with the shared ModalBackdrop,
// and keep the focus trap and Escape.

export { ModalCard as Modal, ModalActions, ModalShell, ModalTitle } from '@dcl/one-ui'
