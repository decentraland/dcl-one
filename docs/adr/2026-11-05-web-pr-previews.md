# ADR: Independent web pull request previews

- Status: Accepted
- Date: 2026-11-05

## Decision

Preview selection follows the two web products: `apps/sites/` selects Sites,
and `library/ui/` selects the standalone UI reviewer. Other applications,
including `apps/ui3/`, and API-only changes do not trigger this preview
pipeline. Changes to the preview pipeline select both products.

The UI reviewer is a static build published under an immutable versioned CDN
path. Its assets, navigation, reference images and JSON requests must resolve
inside that path. A successful product can publish independently of failures
in the other product.

Pull request builds execute without publication credentials. Publication uses
the trusted default-branch workflow and accepts only static reviewer output,
with generated package metadata and no lifecycle scripts. It verifies the
current open pull request, source repository, successful product job and
artifact before publication. A preview link is reported only after the CDN
serves its matching source receipt and HTML entry.

Sites retains server rendering, loaders and actions. Its preview requires a
Node runtime behind the CDN and integration with an existing hosting platform.
Static client output alone is not a functional Sites preview. Services should
default to development, with an explicit production selection; server and
browser requests, caches and sessions must agree on that selection. The
hosting integration and environment selector remain unimplemented until their
deployment contract is established.

## Consequences

Reviewer and Sites builds have independent artifacts and status rows. The
current Sites build still consumes `apps/ui3/`; this dependency is not removed
by changing preview triggers. API contract checks remain separate from preview
selection.

Immutable npm versions and CDN copies remain after pull requests close;
artifact retention does not delete published copies. Deployment status must
distinguish a successful build from an available preview.
