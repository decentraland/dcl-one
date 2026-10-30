# ADR: Scene CLI journey screenshots in CI

- Status: Accepted
- Date: 2026-10-30

Scene CLI changes run the same browser journeys against the pull request's base
and proposed commit. A dependency-free Node CDP runner drives Chromium, disposable
SDK7 scenes and local HTTP fixtures. Screenshots, manifests, logs and a standalone
before/after HTML report are workflow artifacts. Existing visual defects are
recorded, not asserted as required behavior; functional errors fail the check.

A separate `workflow_run` publisher checks out only its trusted default-branch
script. It validates the workflow, current PR association, revisions, archive
limits and PNG paths before publishing images. It never executes downloaded
artifact code. Fork PRs receive the same reports without write credentials in
the build job.

Only changed journeys appear in one updated PR comment, grouped in collapsed
`<details>` sections. Images use GitHub's direct user-attachment storage, the same
endpoint used by GitHub CLI's attachment support. There are no releases, tags,
branches, external storage or contents-write permissions. With no changes and no
failures, no initial comment is posted; an existing comment is updated to clear
stale results. Full reports remain workflow artifacts for 30 days. PR identity
comes from GitHub APIs, never artifact metadata; stale heads do not replace
newer comments. Raw screenshots are retained, while separate comparison hashes
mask carets, clock labels, request counts and minted entity IDs to limit noise.

Live wallet signatures, deployments, World mutations and external runtimes remain
explicit verification boundaries. No live compatibility claim follows from the
fixture journeys. The suite and publisher live in existing `tooling/` paths;
workflow definitions live in `.github/workflows/`.
