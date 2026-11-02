// The one place that knows where this package sits in the repository.
import { fileURLToPath } from 'node:url'
import path from 'node:path'

/** `apps/bevy-explorer/e2e`. */
export const PACKAGE_ROOT = path.resolve(fileURLToPath(new URL('..', import.meta.url)))

/** The dcl-one repository root. */
export const REPO_ROOT = path.resolve(PACKAGE_ROOT, '..', '..', '..')

/** The web driver of #194 (`rendering/bevy-explorer/source/local-harness/web-driver`). */
export const WEB_DRIVER = path.join(REPO_ROOT, 'rendering', 'bevy-explorer', 'source', 'local-harness', 'web-driver', 'driver.mjs')

/** The web app as `apps/bevy-explorer` builds it (`make web`). */
export const DEFAULT_WEB_APP = path.join(REPO_ROOT, 'apps', 'bevy-explorer', 'dist', 'web')

export const SCENES_DIR = path.join(PACKAGE_ROOT, 'scenes')
export const REFERENCES_DIR = path.join(PACKAGE_ROOT, 'references')
export const REPORT_DIR = path.join(PACKAGE_ROOT, 'report')
