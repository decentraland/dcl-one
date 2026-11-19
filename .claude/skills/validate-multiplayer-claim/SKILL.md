---
name: validate-multiplayer-claim
description: Use when asked to prove, reject or check a claim about multiplayer or comms behaviour against a running stack - a pull request that says it fixes a connection problem, a decision record about what a server or the explorer does, a bug report, or evidence to post on a pull request. Covers Pulse, the archipelago, the gatekeeper, LiveKit rooms and the explorer's connections. Formal proofs of a decision belong to verify-dcl-networking.
---

# Validate a multiplayer claim

A claim is settled by an experiment. A scenario fails on the **control** (the build without the change), passes on the **treatment**, and its outcome is traced to the claimed cause. Anything else that could produce the same outcome is a **confound**, and a pass with an open confound proves nothing.

The instrument is `comms-e2e`: scripted actors, a fault link, the real headless explorer and the servers' own oracles. Its commands, stack file and scenario rules are in `services/catalyst/comms-e2e/README.md`.

## 1. Split the claim

Write every sentence of the claim as statements with one observable each: a console state, a disconnect code, a metric, what another actor sees. Name each statement's **owner**: the process that decides the fact (the explorer, Pulse, the archipelago, the gatekeeper, LiveKit).

Done when every statement has one owner and one observable.

## 2. Read the owner

A statement about what another process does ("the server replaces the older socket") is a claim, not a premise. Read the owner's code at the revision the stack runs, down to the line that decides the fact (a caller's comment is not the decision), and cite `path:line` for or against each statement. A statement the owner's code contradicts gets its verdict now, and the scenario asserts what the owner really does.

Done when every statement has source evidence.

## 3. Fill the claim sheet

One sheet per statement. Every field is required; an empty field is an open confound.

| Field | Content |
|---|---|
| Statement | One observable outcome under one condition |
| Owner, source | The process and `path:line` at the stack's revision |
| Control, treatment | Two builds by commit; keep the binaries, labelled by commit |
| Actors | Each actor and what makes it distinct **to the owner**: wallet, session key, source address, transport, room identity. Two actors the owner cannot tell apart are one actor |
| Trigger | The fault or action, and its **proof of effect**: a counter showing it happened (packets dropped, flows cut, sends above the limit) |
| Control fails with | The first failing expectation, for the documented reason |
| Attribution | The signal showing the outcome came from the claimed cause: which message, code or log line, and that it came first |
| Confounds closed | Another signal that ends the same state; warm-up after a restart; a budget shared per source address; machine load; a different failure mode at another fault length |

## 4. Run the experiment

- `comms-e2e doctor` first, then the scenario with `--strict` on the control and the treatment. Races repeat with seeds.
- One failure mode per scenario. Move the fault's length until the mode changes and state the range the scenario covers: a stall that outlasts the transport timeout tests the timeout, not the burst.
- On a failure read the first failing expectation and the actor's log. On a pass find the attribution signal and the proof of effect in the timeline.
- A deadline missed on a busy machine is run once more before it counts.

Done when both builds ran and every sheet's attribution and proof of effect were seen in a timeline or a log.

## 5. Give one verdict per statement

| Verdict | Requires |
|---|---|
| Proven | The control fails for the documented reason, the treatment passes every repeat, the attribution was seen, every confound is closed |
| Holds, not discriminated | It passes on both builds, or no control exists |
| Rejected | The owner's code or a run contradicts it; quote the evidence |
| Wrong record | A run contradicts it and the owner behaves as designed (a decision record, an ADR, the protocol). Correct the statement and every expectation copied from it, and ask the owner's maintainer for a ruling before calling anything a defect |
| Not reproduced | The instrument cannot drive the trigger; say what was tried |

## Report

Per statement: the verdict, both builds by commit, repeats, the scenario's result line, the control's first failing expectation verbatim, the attribution line, and what was not covered. Redact tokens and connection strings.

## Example

Claim: "A second device on the same wallet takes over; the older device is told with `Kicked`."

| Statement | Owner | Finding |
|---|---|---|
| The newer device proceeds on `already_connected` | Explorer | Proven: the control ends in an identity conflict, the treatment connects |
| The server kicks the older socket | Archipelago | Wrong record: the registry is keyed by wallet and session key and replaces only the same key |
| The older device stops for good | Explorer | Two explorers started from one wallet seed sign with the same key, so the owner saw one device: a confound. A second device is the same wallet with another session key, and the older explorer's log must show which signal ended it |
