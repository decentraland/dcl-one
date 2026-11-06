// Magic's publishable key for the social sign-in (Google, Apple, Discord, X), as decentraland.org uses it. It is public
// by design but deployment configuration: MAGIC_API_KEY on the server, handed to the browser with the site's other
// public settings. Without it no social option is offered, rather than one that would sign in a different wallet.

export function magicApiKey(): string {
  if (typeof process !== "undefined" && process.env?.MAGIC_API_KEY) return process.env.MAGIC_API_KEY;
  if (typeof window !== "undefined") return window.__DCL_PUBLIC__?.magicApiKey ?? "";
  return "";
}
