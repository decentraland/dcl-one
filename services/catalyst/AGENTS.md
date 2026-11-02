# Comms and multiplayer design (catalyst-comms)

Read `docs/comms/comms-crate.md` before placing a comms, worlds, storage,
scene-host or Pulse responsibility, and `docs/comms/implementation-pipeline.md`
before planning, dispatching or merging work.

## Rules

- Update root `AGENTS.md` "Ownership" and `docs/comms/comms-crate.md` in the change that moves a comms responsibility.
- Write secrets, internal hostnames and personal data as placeholders.
- Security reports and the optional `comms-security-reviewer` definition stay in the private review workspace. Never copy security findings into tracked files.
- Never use Hammurabi as a reference for behaviour, limits or design; the scene runtime source is the Bevy engine.
- Re-verify a pinned `file:line` at the target revision before citing it.
- Return architecture, divergence and CI evidence in the task report per `docs/comms/README.md`; describe deployment-boundary changes using the architecture model schema. Keep ledger exports, run logs, rendered pages and their builders outside this public tree.
- When a change establishes an architectural choice, record its decision in the task report using `docs/comms/agents/architecture/schema.md`; preserve `status_changed_at` on text edits and update it on creation or a real transition.
- A decision without `current_stack` is not ready for a verdict. Only the user's verdict changes acceptance; record `decided_by`.
- The main session reconciles specialist reports, checks their evidence at the target revision and records the user's decisions before claiming completion.
- When what runs where changes, describe the changed deployment boundaries in the report and update the public ownership guidance. Keep accepted, public architectural decisions in root `docs/adr/`.

Agents (`.claude/agents/`, start the session here): `comms-architect`,
`comms-ci-engineer` (`build`, `gate`, `audit`, `release`), `divergence-analyst`.
Their record schemas are in `docs/comms/agents/<area>/schema.md`;
their working reports belong in root `.agent-work/`.
