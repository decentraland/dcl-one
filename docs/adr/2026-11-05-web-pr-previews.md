# ADR: Independent web pull request previews

- Status: Proposed
- Date: 2026-11-05

## Decision

Preview selection follows the two web products: `apps/sites/` selects Sites and `library/ui/` selects the standalone UI reviewer. API-only changes do not trigger this preview pipeline. Changes to the preview pipeline select both products. Other application paths remain outside preview selection by owner decision; build dependency checks remain in the existing CI. Each PR update re-evaluates its cumulative diff, so a later documentation-only commit refreshes a preview selected by earlier web changes.

The UI reviewer is a static build published under an immutable versioned CDN path. Its assets, navigation, reference images and JSON requests must resolve inside that path. A successful product can publish independently of failures in the other product.

Pull request builds execute without publication credentials. Publication uses the trusted default-branch workflow and accepts only static reviewer output, with generated package metadata and no lifecycle scripts. Each publication requires human approval through a dedicated protected `web-preview` environment with administrator bypass disabled; publication code verifies that these protections and the exact default-branch deployment allowlist are configured. A pending comment precedes the approval wait, and newer publications cancel stale waits for the same PR. A configured maintainer may authorize publication of their own source build. It verifies the current open pull request, source repository, successful product job and artifact before publication. A preview link is reported only after the CDN serves its matching source receipt, HTML entry, version-scoped hashed asset and fidelity JSON. Reviewer publication stays disabled until the source CDN-base regression and audit pass, then the repository variable `WEB_PREVIEW_REVIEWER_ENABLED` enables it.

Sites retains server rendering, loaders and actions. Its preview uses a complete container image built from the pull request and runs in Kubernetes at a shared hostname supplied by deployment configuration. The initial preview is manually triggered and expires after six hours; it can be triggered again when needed. The infrastructure owner supplies the cluster configuration, and the application deployment integration remains to be agreed. Per-pull-request hosts are a possible later option with prorated cost. Services should default to development, with an explicit production selection; server and browser requests, caches and sessions must agree on that selection. The environment selector remains unimplemented.

## Consequences

Reviewer and Sites builds have independent artifacts and status rows. The build workflow provides an availability summary even before the publisher is installed on the default branch. Reviewer artifacts must pass a browser regression under a nested version prefix before upload; that regression checks component navigation, fidelity data and decoded reference/render images as well as asset delivery. API contract checks remain separate from preview selection.

Immutable npm versions and CDN copies remain after pull requests close; artifact retention does not delete published copies. Deployment status must distinguish a successful build from an available preview.

PR-built HTML and JavaScript are public and permanent on the shared CDN origin. A compromised dependency in a same-repository PR can alter that output; artifact validation and secret scanning do not establish content trust. Withdrawing published content requires the npm/CDN operators; expiring GitHub artifacts does not withdraw it.
