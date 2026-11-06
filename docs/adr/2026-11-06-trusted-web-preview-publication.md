# ADR: Install trusted web preview publication independently

- Status: Proposed
- Date: 2026-11-06

## Decision

The trusted reviewer publisher is installed on the default branch before the UI feature needs a preview. It consumes static artifacts from the existing `Web previews / Build` workflow in an open same-repository pull request. The reviewer application and its generated images remain in their feature branch. This allows that pull request to receive a preview without first merging its UI feature.

Publication code and the dependency-free Figma key guard run from the trusted default-branch workflow commit. The guard and its allowlist keep their canonical `library/ui/figma/` paths, as defined by the [shared UI layout](2026-10-30-shared-ui-library.md); they require no UI package installation. Their source is unchanged from commit `e1c73580b5d0bcd775eaecba585c21efbe82dad6`. The pull request contributes validated static files, with no lifecycle scripts executed by the publisher.

The protected `web-preview` environment requires explicit maintainer approval, disables administrator bypass and allows only the default branch. A configured maintainer may approve their own source build. Publication revalidates the current PR, head, source run attempt, successful reviewer job and static artifact before upload, then verifies the CDN source receipt, HTML, bundle and fidelity data before posting a URL. A newer source build cancels stale approval waits. A pending comment replaces any older preview link before the approval wait begins.

## Activation

Publication remains disabled until `WEB_PREVIEW_REVIEWER_ENABLED` is set to `true`. Before activation, a repository maintainer must verify that `GITLAB_CDN_DEPLOYER_TOKEN`, `GITLAB_CDN_DEPLOYER_URL` and `FIGMA_FILE_KEY` are environment-only secrets for `web-preview`, with no equivalent broadly scoped credentials that bypass this publication boundary. The npm trusted publisher for `@dcl-regenesislabs/dcl-one-ui-review` must bind this repository, `web-previews-publish.yml` and the `web-preview` environment. GitHub runtime policy checks cannot verify secret storage or npm configuration. Those configuration checks remain unverified.

After the workflow is installed and configuration is verified, re-run the current `Web previews / Build` run for an open UI PR. The publisher reports the source build awaiting approval; the maintainer approves the deployment in GitHub Actions. A CDN link is posted only after propagation and source verification succeed. Installing the workflow does not itself publish a preview or replay previously completed builds.

## Consequences

Publisher validation runs without publication credentials. UI build and hosting-base changes continue in their owning feature branches, including nested-prefix browser validation before artifact upload. Static artifact validation rejects unsafe paths, links and unsupported files, and the trusted guard enforces file-count, byte-size and depth limits.

Published npm versions and CDN copies are public and immutable; closing a PR or expiring its GitHub artifacts does not remove them. Withdrawing published content requires the npm/CDN operators and verification that the affected URLs no longer serve it. This installation does not supply an SSR runtime for Sites.
