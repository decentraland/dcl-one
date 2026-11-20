/**
 * Who may frame the web Explorer's two documents, /play (the shell) and /play/app.html (the HUD and the engine):
 * only this origin, where the shell frames the app. The shell keeps the sign-in key and draws BUY ITEM, SEND GIFT
 * and SEND TIP, which an in-app wallet signs with no window of its own, so a page of another site that framed it
 * could lay its own buttons over them (clickjacking). The shell checks the same in the page (apps/ui3
 * play/shell/framing), for a response that lost these headers on the way.
 */
export const SAME_ORIGIN_FRAMES = {
  "Content-Security-Policy": "frame-ancestors 'self'",
  "X-Frame-Options": "SAMEORIGIN",
} as const;
