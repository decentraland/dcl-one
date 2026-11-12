---
name: verify-dcl-networking
description: Use when asked to formally verify or maintain a Rocq proof of dcl-one's LiveKit, room retry, island handoff, microphone lifecycle, voice session ownership, or comms admission/Pulse decisions. Also use when assessing whether networking proof evidence still matches the source. Ordinary networking edits and live connectivity troubleshooting do not require this skill.
---

# Verify dcl-one networking

A small source-bound proof establishes a named decision under explicit assumptions. It does not establish SDK behavior, audible silence, ECS ordering, authorization across an entire service, or eventual connectivity.

## Select the boundary

Find the repository root with Git and read its applicable instructions. Read [boundaries.md](references/boundaries.md) for the requested pipeline, then inspect the current declarations and callers. Paths are search starting points, not proof of current behavior.

Write the contract before the model: source revision and dirty state; owning state; events and their order; invariant; observable failure; assumptions and exclusions. Choose one decision that can be exercised independently. Track requested intent, in-flight work and acknowledged completion separately.

For an existing proof, start with `check` and `drift`. A red gate requires source/model review; recording a new lock only accepts the reviewed baseline. Preserve the claim's meaning; ask before weakening it or introducing new trusted axioms.

## Establish evidence

Read [proofgate.md](references/proofgate.md) for the pinned tool, manifest and execution commands. Keep artifacts in ignored task storage and follow local build/cache placement rules.

1. State claims in plain language and anchor every source input, including generators' inputs. Record immutable source revisions or hash the dirty files actually examined.
2. Model the smallest transition system. Give conditional claims a reachable example satisfying their premises. State abstractions such as natural-number time, saturated retry buckets or serialized mutex transitions.
3. Bind the model with a restricted generator that rejects unrecognized source shapes. A hash detects change; it does not establish correspondence. Review every modeled branch and caller assumption.
4. Exercise the real decision in an implementation witness. Prefer an unchanged std-only module; an extracted method harness must name its stubs and omitted integration. A second handwritten model is not an implementation witness.
5. Run fresh compilation, assumption audit, kernel checking, witness and mutations. Mutation evidence identifies its rejecting layer: source grammar, implementation assertion or Rocq claim. Count setup failures separately. A survivor means a missing property, an equivalent mutation, or an explicit scope limit—not automatic success.

## Report the result

Return the scoped verdict, source revision, compiled claim/axiom counts, witness coverage, mutation results by rejecting layer, open findings and excluded behavior. Label evidence `model`, `test`, `source` or `hypothesis`. Distinguish an actual production counterexample from an injected regression. Include commands and retained artifact locations so another agent can reproduce the result.

For microphone drainage, a useful bounded contract is: repeated revocation contributes at most one charge; an unacknowledged stop keeps admission closed; acknowledged completion releases only that publication's charge. Proving this accounting still leaves native/browser stop semantics and concurrent scheduling for integration evidence.
