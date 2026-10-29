declare module "virtual:react-router/hmr-runtime" {
  const runtime: {
    injectIntoGlobalHook(target: Window): void;
    validateRefreshBoundaryAndEnqueueUpdate(...args: unknown[]): string | undefined;
  };
  export default runtime;
}
