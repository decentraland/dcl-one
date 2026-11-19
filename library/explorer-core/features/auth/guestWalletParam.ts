/** `/play?guestWallet=thirdweb` asks for the thirdweb guest on a host outside Decentraland's domains
 *  whose origin the thirdweb project allows too, so a test deployment's guests behave like
 *  Decentraland's (features/auth/guest.ts `webGuestBackendFor`). */
export const THIRDWEB_GUEST_PARAM = 'guestWallet'
