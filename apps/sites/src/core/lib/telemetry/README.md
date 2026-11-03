# Travel telemetry

Place detail and world-scene Visit links share the `jump-in` story:

- `jump_in_started`: the visitor activates a travel control.
- `jump_in_completed`: a valid launch URL is handed to browser navigation.
- `jump_in_failed`: the destination could not be resolved and no navigation is offered.

Completion records the handoff, not arrival or successful Explorer loading.
For a ready link, started and completed occur on the same click. Their ratio
measures destination-resolution availability, not an Explorer loading funnel.
Use Explorer loading telemetry to measure arrival and loading failures.
