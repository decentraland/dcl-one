// Stands in for the main-process modules that need a running Electron app (windows, the
// Explorer gateway, logging) while the manifest script imports the tool definitions. The unit
// tests stub the same modules. Listing the tools never calls any of this.
const noop = () => {};

export const MAIN_WINDOW_ID = 'main';
export const getWindow = () => undefined;
export const isRealmDown = () => false;
export const gatewayProject = () => null;
export const explorerTools = () => [];
export const onExplorerToolsChanged = () => noop;
export const callExplorerTool = noop;
export const launchPreview = noop;
export const previewStatus = noop;
export const stopExplorerGateway = noop;
export const stopPreview = noop;

export const app = { getPath: () => '', getVersion: () => '0.0.0', isPackaged: false };

const log = { info: noop, warn: noop, error: noop, debug: noop };
export default log;
