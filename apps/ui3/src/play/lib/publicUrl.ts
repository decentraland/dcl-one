// The page's directory URL, treating the pathname as a directory even without a trailing slash —
// a hosted entry URL is <site>/bevy-web (no slash), which would otherwise make relative
// resolution escape to the origin root. A page FILE (app.html, the app inside the web
// shell) resolves to the directory it is in. Same-origin things (the engine, the bridge scene, the
// service worker) resolve against THIS, never against the asset CDN base.
// On Sites the app document is the `app.html` route under the shell's directory (/play/app.html);
// /play/app only redirects there. Both, including an appended slash, resolve to /play/.
const PAGE_FILE = /(\.html|\/app)\/?$/
export const pageDir = (href: string): string => {
  const { pathname } = new URL(href)
  return PAGE_FILE.test(pathname)
    ? new URL(pathname.replace(/\/?$/, '').replace(/[^/]*$/, ''), href).href
    : new URL(pathname.replace(/\/?$/, '/'), href).href
}
export const PAGE_DIR = pageDir(location.href)

// Public-dir assets referenced at RUNTIME (JSX src strings). Sites serves the HUD's public files
// (apps/ui3/public/play) and the engine artifact under the same /play/ directory as the page.
const BASE = PAGE_DIR

export const publicUrl = (path: string): string => new URL(path, BASE).href
