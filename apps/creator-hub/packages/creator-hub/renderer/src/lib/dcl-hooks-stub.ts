// decentraland-ui2 imports this name in its DownloadButton, which Creator Hub does not use.
// The workspace removes `@dcl/hooks` from the install; renderer/vite.config.js points the import here.
export function useAdvancedUserAgentData(): never {
  throw new Error('@dcl/hooks is not installed in Creator Hub');
}
