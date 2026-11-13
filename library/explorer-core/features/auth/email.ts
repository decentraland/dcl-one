// An email sign-in's address rules, shared by the sign-in screen (the HUD) and the shell page that
// checks them again before calling thirdweb.

/** An address the email sign-in takes. */
export const EMAIL_PATTERN = /^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$/
/** The longest address it takes. */
export const EMAIL_MAX = 100
