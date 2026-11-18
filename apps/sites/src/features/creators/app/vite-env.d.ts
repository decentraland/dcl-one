/// <reference types="vite/client" />

/** Baked in by vite.config (and by vitest.config for tests): the Sentry release this build reports. */
declare const __SENTRY_RELEASE__: string

interface ImportMetaEnv {
  readonly VITE_DCL_DEFAULT_ENV?: string
  readonly VITE_BASE_URL?: string
  readonly VITE_REACT_APP_WEBSITE_VERSION?: string
  readonly VITE_FEATURE_FLAG_OVERRIDES?: string
}

/** Baked in by vite.config (and vitest.config): the installed wearable-validator version, part of every cached result's key. */
declare const __VALIDATOR_VERSION__: string
