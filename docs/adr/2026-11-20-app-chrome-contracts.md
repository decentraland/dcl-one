# ADR: Shared app chrome contracts

- Status: Accepted
- Date: 2026-11-20

Shared chrome contexts, router link adapters, and layout slots live in `library/ui/components/AppChrome/`. UI hosts import these modules by path so their providers and consumers share the same contexts. The namespace follows the library's handwritten component naming rule. Sites-specific chrome remains under `library/ui/apps/sites/chrome/`.

The public component verifier checks AppChrome as handwritten source. Capture and mapping requirements apply to components with Figma identities.
