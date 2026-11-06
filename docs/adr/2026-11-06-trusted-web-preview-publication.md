# ADR: Install trusted web preview publication independently

- Status: Proposed
- Date: 2026-11-06

## Decision

The trusted reviewer publisher is installed on the default branch before the UI feature needs a preview. It consumes static artifacts from the existing `Web previews / Build` workflow in an open same-repository pull request. The reviewer application and its generated images remain in their feature branch. This allows that pull request to receive a preview without first merging its UI feature.

Publication code and the dependency-free Figma key guard run from the trusted default-branch workflow commit. The guard and its allowlist keep their canonical `library/ui/figma/` paths, as defined by the [shared UI layout](2026-10-30-shared-ui-library.md); they require no UI package installation. Their source is unchanged from commit `e1c73580b5d0bcd775eaecba585c21efbe82dad6`. The pull request contributes validated static files, with no lifecycle scripts executed by the publisher.

The protected `web-preview` environment requires explicit maintainer approval, disables administrator bypass and allows only the default branch. A configured maintainer may approve their own source build. Publication revalidates the current PR, head, source run attempt, successful reviewer job and static artifact before sending the package to npm and requesting the CDN upload. A newer source build cancels stale approval waits. A pending comment replaces any older preview link before approval and remains pending while the upload runs.

CDN readiness is the pipeline's successful GitHub deployment status for environment `cdn`, task `upload-to-cdn` and this package/version, following the existing Bevy rollout contract. The publisher does not poll the CDN while files are uploading. A separate completion job reads the trusted source receipt, revalidates the current PR and source attempt, and checks the HTML, bundle and fidelity data before posting a URL. Every CDN request sends an explicit preview User-Agent because the CDN rejects Python's default urllib signature. Completion retries verify the same deployed version and never publish another npm version. Comment writers share one serialized queue; successful npm publication does not post a competing final comment.

## Activation

Keep `WEB_PREVIEW_REVIEWER_ENABLED` disabled until this checklist is verified:

1. Install the trusted workflow and scanner on the default branch after CI, review and the required merge authorization. Installing them does not publish a preview or replay earlier completed builds.
2. Register `@dcl-regenesislabs/dcl-one-ui-review` in `definitions/src/static-site-rollouts.ts`, merge that definitions change and republish `@decentraland/definitions@next` for the upload pipeline to consume. An unregistered package can produce a successful pipeline that skips upload. Entries currently require domains; support for an entry serving only the CDN prefix is unverified and must be confirmed before activation.
3. Have an `@dcl-regenesislabs` owner publish an initial placeholder npm version, then configure `npm trust` for repository `decentraland/dcl-one`, workflow `web-previews-publish.yml` and environment `web-preview`, with direct `npm publish` allowed. The package currently does not exist, so the trust configuration cannot yet be established. Verify this binding before enabling publication.
4. Add `FIGMA_FILE_KEY` to the protected `web-preview` environment and verify that the trusted artifact scan runs the key check and passes. It is the file key used for leak detection; PR builds receive no Figma credentials and do not call the Figma API. The existing `GITLAB_CDN_DEPLOYER_TOKEN` and `GITLAB_CDN_DEPLOYER_URL` are shared organization secrets also used by Bevy previews; retain their existing consumers and verify that this repository can use them.
5. Verify that the GitLab upload pipeline pins `https://registry.npmjs.org` and rejects or ignores caller-provided alternate `REGISTRY_URL` values. Oddish passes that variable, and npm's environment-bound trust protects publishing only through the canonical registry. The GitLab-side registry policy is unverified and remains an activation blocker. GitHub runtime environment checks cannot attest to this policy or npm setup.
6. Enable `WEB_PREVIEW_REVIEWER_ENABLED`, re-run the current `Web previews / Build` for an open UI PR and approve its protected deployment. Confirm the pipeline creates the matching successful `cdn` / `upload-to-cdn` deployment, then the completion job posts the verified preview URL. If post-upload verification fails, retry the completion job for that deployment/version instead of publishing another npm version.

## Consequences

Publisher validation runs without publication credentials. UI build and hosting-base changes continue in their owning feature branches, including nested-prefix browser validation before artifact upload. Static artifact validation rejects unsafe paths, links and unsupported files, and the trusted guard enforces file-count, byte-size and depth limits.

Published npm versions and CDN copies are public and immutable; closing a PR or expiring its GitHub artifacts does not remove them. Withdrawing published content requires the npm/CDN operators and verification that the affected URLs no longer serve it. This installation does not supply an SSR runtime for Sites.
