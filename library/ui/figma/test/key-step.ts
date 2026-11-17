// The exact steps a job that holds FIGMA_FILE_KEY may run, written once for the workflow policy (workflows.test.ts)
// and the run test (key-scan-workflow.test.ts). A key step's run must equal one of these templates once trimmed,
// and its env must hold exactly the names the template lists; nothing in it is parsed as shell.

export const KEY_SECRET = '${{ secrets.FIGMA_FILE_KEY }}'
// The commit the event names: a default branch commit under the trigger rule, read by object id.
export const TRUSTED_SHAS = ['${{ github.sha }}', '${{ github.workflow_sha }}']
// Git reads only the repository's own config, and no replace object.
export const GIT_HARDENING: Record<string, string> = {
  GIT_CONFIG_NOSYSTEM: '1',
  GIT_CONFIG_GLOBAL: '/dev/null',
  GIT_NO_REPLACE_OBJECTS: '1'
}
// The actions a key job may use, each at the one commit the repository pins it to. Any other sha, a fork's
// commit included, is refused.
export const KEY_JOB_ACTIONS: Record<string, string> = {
  'actions/checkout': '3d3c42e5aac5ba805825da76410c181273ba90b1',
  'actions/setup-node': '949feb2413d6458794dcd2491c4babbbce0c15c1',
  'actions/download-artifact': '9000827ccba6bdab643e8b6fd33ac0654aef8333'
}
export const GUARD_COPY = '$RUNNER_TEMP/key-guard.mjs'
export const SCAN_OUTPUT = '$RUNNER_TEMP/key-scan.txt'
// The scan inputs the unprivileged job uploads: the scanned commits as a bundle and the LFS objects they point to.
export const SCAN_INPUT = '$RUNNER_TEMP/key-scan-input'

type KeyStepTemplate = {
  name: string
  run: string
  // Where the job's single checkout lands ('' for the workspace root), as the template's --root reads it.
  checkout: string
  // The only artifacts the job may download: each path with its one artifact name and the run it comes from
  // (runId absent: this workflow run's own artifact, which the unprivileged job uploaded).
  downloads: { path: string; name: string; runId?: string }[]
  // The data variables the run reads, each with the only expressions its value may be.
  data: Record<string, string[]>
}

const copy = (git: string) =>
  `${git} --no-replace-objects cat-file blob "$TRUSTED_SHA:library/ui/figma/key-guard.mjs" > "${GUARD_COPY}"`
const check = `grep -qx 'check key: ran' "${SCAN_OUTPUT}"`

export const KEY_STEP_TEMPLATES: KeyStepTemplate[] = [
  {
    name: 'key scan over the trusted/ checkout',
    run: [
      'set -o pipefail',
      '[[ "$HEAD_SHA" =~ ^[0-9a-f]{40}$ && "$BASE" =~ ^[0-9a-f]{40}$ ]] || exit 1',
      copy('git -C trusted'),
      `git -C trusted fetch --quiet --no-tags --no-recurse-submodules --no-write-fetch-head "${SCAN_INPUT}/commits.bundle" "+refs/key-scan/*:refs/key-scan/*"`,
      'range="$BASE..$HEAD_SHA"',
      'if [ "$BASE" = "0000000000000000000000000000000000000000" ]; then range="$HEAD_SHA"; fi',
      'extra=()',
      'if [ "$ARTIFACT_REQUIRED" = true ]; then',
      '  extra=(--tree "$HEAD_SHA:library/ui" --artifact "$RUNNER_TEMP/pr-artifact")',
      'fi',
      `node "${GUARD_COPY}" --root trusted --no-worktree --revision "$TRUSTED_SHA" --range "$range" \\`,
      `  --allowlist-from "$TRUSTED_SHA" --lfs-store "${SCAN_INPUT}/lfs" "\${extra[@]}" | tee "${SCAN_OUTPUT}"`,
      check
    ].join('\n'),
    checkout: 'trusted',
    downloads: [
      { path: '${{ runner.temp }}/key-scan-input', name: 'key-scan-input' },
      {
        path: '${{ runner.temp }}/pr-artifact',
        name: 'ui-review-${{ needs.prepare.outputs.head }}',
        runId: '${{ github.event.workflow_run.id }}'
      }
    ],
    data: {
      HEAD_SHA: ['${{ needs.prepare.outputs.head }}'],
      BASE: ['${{ needs.prepare.outputs.base }}'],
      ARTIFACT_REQUIRED: ['${{ needs.prepare.outputs.artifact_required }}']
    }
  },
  {
    name: 'artifact scan over a workspace root checkout',
    run: [
      'set -o pipefail',
      copy('git'),
      `node "${GUARD_COPY}" --root . --no-worktree --no-range --revision "$TRUSTED_SHA" \\`,
      `  --allowlist-from "$TRUSTED_SHA" --artifact .preview/input | tee "${SCAN_OUTPUT}"`,
      check
    ].join('\n'),
    checkout: '',
    downloads: [
      {
        path: '.preview/input',
        name: 'web-preview-reviewer',
        runId: '${{ github.event.workflow_run.id }}'
      }
    ],
    data: {}
  }
]

// The optional last step of a key job: the check run built from the scanner's stdout alone, without the key.
export const REPORT_TEMPLATE = {
  run: [
    'conclusion=failure',
    '[ "$OUTCOME" = success ] && conclusion=success',
    `summary=$(head -c 60000 "${SCAN_OUTPUT}" 2> /dev/null || echo "The scan did not run.")`,
    `[ -d "${SCAN_INPUT}" ] || summary="The key-scan-input artifact is missing or expired: re-run all jobs."`,
    'gh api --method POST "repos/$REPOSITORY/check-runs" \\',
    `  -f name='UI library key scan' -f head_sha="$HEAD_SHA" -f status=completed \\`,
    '  -f conclusion="$conclusion" -f "output[title]=Figma file key scan: $conclusion" \\',
    '  -f "output[summary]=$summary" > /dev/null'
  ].join('\n'),
  env: {
    GH_TOKEN: ['${{ github.token }}'],
    REPOSITORY: ['${{ github.repository }}'],
    HEAD_SHA: ['${{ needs.prepare.outputs.head }}'],
    OUTCOME: ['${{ steps.scan.outcome }}']
  } as Record<string, string[]>
}
