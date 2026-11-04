// from: auth@e081cc6:src/modules/translations/en.json (common, connection, email_input, email_login_modal,
//       connection_layout)
// decentraland/auth is Apache-2.0.
// The sign-in page's English copy, word for word, under the upstream keys.

const COPY = {
  "common.try_again": "Try Again",
  "connection.title": "Log in or Sign up to Jump In",
  "connection.continue_with": "Continue with {provider}",
  "connection.more_options": "More Options",
  "connection.or_continue_with": "or continue with",
  "connection.metamask_not_installed":
    "You need to install the MetaMask Browser Extension to proceed. Please install it and try again.",
  "email_input.recommended": "Recommended",
  "email_input.placeholder": "Enter Your Email",
  "email_input.next": "NEXT",
  "email_input.validation_error": "Enter A Valid Email Address",
  "email_login_modal.title": "Enter verification code",
  "email_login_modal.subtitle_prefix": "One time password sent to ",
  "email_login_modal.subtitle_suffix": ". Please enter the code below to complete verification.",
  "email_login_modal.verifying": "Verifying...",
  "email_login_modal.resend_code": "Resend Code",
  "email_login_modal.didnt_get_email": "Didn't get an email? ",
  "email_login_modal.back": "BACK",
  "email_login_modal.enter_complete_code": "Please enter the complete verification code",
  "email_login_modal.invalid_or_expired": "The code is invalid or expired. Please resend code",
  "email_login_modal.failed_verify": "Failed to verify the code. Please check and try again.",
  "email_login_modal.failed_resend": "Failed to resend verification code",
  "email_login_modal.network_error": "Unable to connect. Please check your internet connection and try again.",
  "login.errors.network_error": "Unable to connect. Please check your internet connection and try again.",
  "login.errors.failed_send_code": "Failed to send verification code. Please try again.",
  "login.errors.invalid_email": "The email address is invalid. Please check and try again.",
  "connection_layout.error":
    "You did not confirm this action in your digital wallet extension. To continue, please try again.",
  "connection_layout.error_locked_wallet":
    "Your wallet is currently locked. To continue, please unlock your wallet and try again.",
  "connection_layout.error_wallet_signature_unsupported":
    "Your wallet can't sign the Decentraland login request. To continue, please connect with a different wallet.",
  "connection_layout.error_connection_failed":
    "We couldn't complete the connection to your wallet. To continue, please try again.",
  "connection_layout.waiting_for_signature": "Confirm in your digital wallet extension to continue.",
  "connection_layout.loading_magic": "Redirecting...",
  "connection_layout.error_generic": "Something went wrong. Please try again.",
  "connection_layout.error_contact_support": "Or contact support with the following error message:",
} as const;

export type CopyKey = keyof typeof COPY;

export function t(key: CopyKey, values?: Record<string, string>): string {
  const text: string = COPY[key];
  return values ? text.replace(/\{(\w+)\}/g, (m, name: string) => values[name] ?? m) : text;
}
