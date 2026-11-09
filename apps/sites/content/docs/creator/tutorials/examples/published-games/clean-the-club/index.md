> Cooperative nightclub cleanup with shared component polling and avatar-attached props

# Clean The Club

![Clean The Club preview](/docs/creator/tutorials/examples/published-games/images/cleanTheClub-preview.webp)

A cooperative cleanup game where players race to restore a nightclub before the next party.

- [Play on Decentraland](https://decentraland.org/jump?realm=cleantheclub.dcl.eth)

<!-- example-context:start -->
## Project information

**Maintained by:** Community · CleanTheClub

[View upstream source](https://github.com/CleanTheClub/CleanTheClub)

**License:** [Apache-2.0](https://github.com/CleanTheClub/CleanTheClub/blob/main/LICENSE). Check upstream terms before reuse; code and assets may have different terms.


> [!WARNING]
> **Community-maintained code — review before use**
>
> This community project is maintained outside the Decentraland Foundation and DAO GitHub organizations. Its source code and live deployment may change independently.
>
> Inclusion in this catalog does not imply a security audit or guarantee of safety. Review code, dependencies, and installation scripts before running or reusing it.
>
> [Report a catalog concern](https://github.com/decentraland/docs/issues/new). For suspected vulnerabilities, follow the upstream project's security policy. Do not include exploit details or credentials in public issues.

<!-- example-context:end -->

<!-- example-highlights:start -->
## Explore the code

These links point to a specific source snapshot so the explanations remain useful as upstream development continues. They are learning references, not a security assessment.

### Share one component scan across several systems

onClutterPoll gathers ClutterSync entries once per polling interval, builds an item-ID lookup, and passes the same snapshot to subscribers. Each consumer can do its own filtering and change detection.

**Project context:** This is useful for studying how to avoid repeated ECS scans; it depends on the shared schema and phase-gate polling interval, and subscriptions belong in client initialization.

**Source references:**

- [onClutterPoll](https://github.com/CleanTheClub/CleanTheClub/blob/8ffc6c1fa24ff0da22a1d82ecdf2f9ce72f47f73/src/client/clutterWatcher.ts#L15-L54)

### Show another player's carried equipment

updateRemoteCarry attaches a prop hierarchy to the other avatar's left hand, selects a model from the current gear or hauling state, and scales the bag by its fill fraction. It also removes the hierarchy when there is nothing to display.

**Project context:** Read this with the carry message handlers, theme model lookup, and model-fit offsets: those provide the state and asset-specific positioning.

**Source references:**

- [updateRemoteCarry](https://github.com/CleanTheClub/CleanTheClub/blob/8ffc6c1fa24ff0da22a1d82ecdf2f9ce72f47f73/src/client/carrySystem.ts#L805-L846)

<!-- example-highlights:end -->
