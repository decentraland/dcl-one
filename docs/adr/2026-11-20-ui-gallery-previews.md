# ADR: UI gallery previews for pull requests

- Status: Accepted
- Date: 2026-11-20

## Context

The runtime gallery ([ADR](2026-11-20-ui-runtime-gallery.md)) has a hosted build (`vite build --mode hosted`): the gallery alone, without the Figma reference, with relative URLs. A reviewer of a pull request that changes `library/ui` had to download the `ui-review-<sha>` artifact and serve it locally. A pull request run executes the pull request's own workflow file and code, so it cannot hold a publication credential. The Figma file key check runs in `ui-library-key-scan.yml`, a `workflow_run` workflow that runs the default branch's code and reads the pull request's commits and Review artifact as data.

## Decision

Every pull request from this repository that changes `library/ui` gets one comment that opens its hosted gallery, built for that pull request and kept five days. Publication is the default branch's job: nothing a pull request changes runs with the credential.

**Build.** `ui-library.yml` builds the hosted gallery into `hosted/` inside the `ui-review-<sha>` artifact, checks it under a prefix as deep as the published one, and only then runs the url scan and uploads the artifact. The key scan clears `hosted/` with the rest of the artifact. The artifact keeps its name, which `library/ui/figma/test/workflows.test.ts` pins; it changes when the Figma import's retirement removes those tests.

**Publisher.** `ui-gallery-publish.yml` runs when `Shared UI library key scan` completes. `workflow_run` always runs the default branch's copy of the file, every job checks out that commit, and no job checks out or runs pull request code. Before any credential exists, `tooling/ui-gallery-publish.py` proves that:

1. the triggering run is the default branch's key scan, by file path and workflow id, it succeeded, and its key-scan job ran every step, including the Review download;
2. the key-scan job's log, read only up to its key step, names the Review artifact, its run, and the artifact id and digest it downloaded and checked;
3. that run is the `Shared UI library` pull request run of that head from this repository, not from a fork, and its library job ran (the `ui_changed` gate);
4. the pull request is open and its current head is the scanned SHA;
5. the artifact API reports that id for that run and head with that digest, the run holds no other live artifact of that name, and the downloaded bytes have that digest;
6. `hosted/index.html` exists and every file under `hosted/` is a regular file of a served type, within the key guard's artifact limits;
7. the four repository variables are set and the `ui-gallery` environment deploys from `main` only.

A run that is not eligible (publication not configured, a fork, a closed or moved-on pull request, a scan that read no Review artifact) ends with a notice and no failure, and a failed key scan skips the run's jobs. Evidence that does not match (another artifact id or digest, an unsafe archive, no `hosted/index.html`) fails the run. Then the job assumes `UI_GALLERY_ROLE_ARN` through OIDC, copies `hosted/` to `s3://$UI_GALLERY_BUCKET/ui-gallery/pr/<n>/<sha>/` with explicit content types (`Cache-Control: public, max-age=300` for pages, one day for hashed assets) and no ACL, and waits until `$UI_GALLERY_BASE_URL/ui-gallery/pr/<n>/<sha>/index.html` and one asset answer 200 (about two minutes at most). Only then does it create or update the comment from `github-actions[bot]` that carries the `<!-- ui-gallery-preview -->` marker, after checking again that the head is current. Its first line is the **Open the UI gallery** link; its second names the short SHA and the expiry date, five days after the upload. Only the uploading job can request an OIDC token, only the comment job can write to pull requests, and the runs of one pull request publish and comment one at a time.

**Hosting**, defined in `decentraland/dcl-one-infra`:

- A private bucket (Block Public Access, no listing, server-side encryption that needs no key permission from the writer, such as S3-managed keys), read only through a CloudFront distribution with Origin Access Control.
- A hostname under `dcl.dev` dedicated to previews, `ui-gallery.dcl.dev`, which `UI_GALLERY_BASE_URL` names. The distribution forwards no cookies, adds `X-Robots-Tag: noindex, nofollow` to every response, and caches for minutes by default and for well under five days at most.
- A lifecycle rule that expires objects under `ui-gallery/pr/` five days after their upload. Each head has its own prefix, so the publisher never deletes or lists objects.
- A writer role assumable only through GitHub OIDC, with `StringEquals` on `aud` (`sts.amazonaws.com`), `sub`, `repository_id` (`1439007511`), `job_workflow_ref` (`decentraland/dcl-one/.github/workflows/ui-gallery-publish.yml@refs/heads/main`) and `ref` (`refs/heads/main`). IAM supports each of these keys for GitHub. This repository uses GitHub's immutable subject claims, so `sub` is `repo:decentraland@12685795/dcl-one@1439007511:environment:ui-gallery`, not the name-only form. The role's one permission is `s3:PutObject` on `arn:aws:s3:::<bucket>/ui-gallery/pr/*`.

**Why main only.** A pull request can change its own workflows, but not the default branch's `ui-gallery-publish.yml`, the role's conditions or the `ui-gallery` environment's branch rule. `workflow_run` runs only the default branch's publisher, and the role's conditions and the environment's branch rule each admit only that publisher, so the credential is never in a job whose code a pull request chose. The pull request still chooses the bytes; the key scan clears them of the Figma file key, and the publisher publishes exactly the bytes it cleared.

## Alternatives

- The drafts [#297](https://github.com/decentraland/dcl-one/pull/297) and [#336](https://github.com/decentraland/dcl-one/pull/336) publish the reviewer as npm packages through the GitLab CDN pipeline that `decentraland/definitions` registers. Copies on the shared CDN origin are public and permanent, and removing one needs the CDN operators. The upload uses a shared deploy token and a caller-supplied registry whose policy could not be verified, and every publication waits for a manual approval. This decision supersedes both drafts for UI previews.
- The Creator Hub SDK bucket and its role belong to the Creator Hub's builds and previews. UI previews there would share that product's role, writers and retention instead of a role that can write only `ui-gallery/pr/`.

## Consequences

- A preview runs scripts that a writer of this repository built; pull requests from forks never publish. The preview hostname is same-site with the services on `dcl.dev`: its scripts can read and set cookies scoped to the whole domain and send same-site requests, so no `dcl.dev` service may rely on a domain-wide cookie or on `SameSite` alone.
- S3 rounds a lifecycle expiration up to the next midnight UTC and removes objects asynchronously, so a link works at least until the date in its comment. A later build of the same head replaces the pages and adds its own assets.
- The comment follows the latest head a few minutes after its Shared UI library run and key scan pass. A head that never passes them keeps the previous link, labelled with its SHA.
- Publication starts once this workflow is on `main`, the infrastructure is applied, a repository admin has created the `ui-gallery` environment with `main` as its only deployment branch, and has set `UI_GALLERY_ROLE_ARN`, `UI_GALLERY_BUCKET`, `UI_GALLERY_AWS_REGION` and `UI_GALLERY_BASE_URL` (an origin, no trailing slash). Until then every run ends with a notice. The publisher checks the environment before a job names it, because naming a missing environment creates it without that rule.
- The publisher reads the key scan's job log and job list. A change to the key scan's steps or to that log's format stops publication with a notice or an error until the publisher follows it; `tooling/tests/test_ui_gallery_publish.py` pins the log format it reads.
