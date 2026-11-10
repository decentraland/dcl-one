// The key step's two fixed commands, written once for the workflow policy (workflows.test.ts) and the run test
// (key-scan-workflow.test.ts): the fresh copy of the pinned guard blob, and node running that copy.
const escape = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

export const GUARD_COPY = '$RUNNER_TEMP/key-guard.mjs'
const PINNED_BLOB =
  '--no-replace-objects cat-file blob "$TRUSTED_SHA:library/ui/figma/key-guard.mjs"'
export const FRESH_COPY = `git -C trusted ${PINNED_BLOB} > "${GUARD_COPY}"`
export const GUARD_NODE = `node "${GUARD_COPY}"`

// The copy command without its redirect, as the policy sees it once redirects are stripped, and with it.
const PINNED_READ = String.raw`git (?:-C trusted )?${escape(PINNED_BLOB)}`
export const PINNED_READ_PATTERN = new RegExp(`^${PINNED_READ}$`)
export const FRESH_COPY_PATTERN = new RegExp(`^${PINNED_READ} > ${escape(`"${GUARD_COPY}"`)}$`)
// node on the copy, with arguments but no further command.
export const GUARD_NODE_PATTERN = new RegExp(`^${escape(GUARD_NODE)}(?: [^;&|]*)?$`)
// Every line that starts by running the copy, to tell it apart from node running any other $RUNNER_TEMP file.
export const GUARD_NODE_LINES = new RegExp(`^${escape(GUARD_NODE)}`, 'gm')
// The copy followed by node on it, as consecutive lines of a step.
export const COPY_THEN_NODE = new RegExp(`^${escape(FRESH_COPY)}\\n${escape(GUARD_NODE)} `, 'm')
