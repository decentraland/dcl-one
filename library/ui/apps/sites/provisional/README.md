# Provisional Sites UI

Components ported from the current `decentraland/sites` pages to CSS Modules and `library/ui` tokens, so the account and profile pages can move to `apps/sites` before their Figma components exist. Each file carries a `TODO(figma-ds)` mark. When the Figma-generated component lands in `library/ui/components`, its consumers switch to it and the provisional one is deleted, together with its row here.

A test in `apps/sites` fails when a component in this folder is missing from this table.

| Component                     | Source in `decentraland/sites`                                                                  | Expected Figma component      | Consumers                                      |
| ----------------------------- | ----------------------------------------------------------------------------------------------- | ----------------------------- | ---------------------------------------------- |
| `ProfileAvatar/ProfileAvatar` | `src/components/profile/ProfileAvatar`                                                          | Avatar                        | `profile/ProfileHeader`, `SceneCard/SceneCard` |
| `profile/ProfileLayout`       | `src/components/profile/ProfileLayout`                                                          | Profile layout                | `apps/sites` profile page                      |
| `profile/ProfileHeader`       | `src/components/profile/ProfileHeader`                                                          | Profile header                | `apps/sites` profile page                      |
| `profile/ProfileTabs`         | `src/components/profile/ProfileTabs`                                                            | Tabs                          | `apps/sites` profile page                      |
| `profile/ProfileOverview`     | `src/pages/profile/tabs/OverviewTab`                                                            | Profile overview              | `apps/sites` profile page                      |
| `profile/ProfileBadges`       | `src/pages/profile/tabs/OverviewTab` (badges row and tooltip)                                   | Badges row, Tooltip           | `apps/sites` profile page                      |
| `profile/ProfileAvatarBody`   | `src/components/profile/AvatarRender` (its fallback image)                                      | Avatar render                 | `apps/sites` profile page                      |
| `profile/ProfilePlaces`       | `src/pages/profile/tabs/PlacesTab` (states, count and grid)                                     | Profile places panel          | `apps/sites` profile page                      |
| `profile/ProfileCollectibles` | `src/pages/profile/tabs/{CreationsTab,AssetsTab}` (filters, states, grid, NAME rows, Load more) | Profile collectibles panel    | `apps/sites` profile page                      |
| `CatalogCard/CatalogCard`     | `decentraland-ui2` `CatalogCard`, with the tabs' wearable badges (`OverviewTab.icons`)          | Catalog card                  | `apps/sites` profile page; meant for Shop      |
| `SceneCard/SceneCard`         | `decentraland-ui2` `SceneCard`                                                                  | Scene card                    | `apps/sites` profile page                      |
| `FilterChip/FilterChip`       | `src/components/profile/FilterChips`                                                            | Filter chip                   | `apps/sites` profile page                      |
| `EmptyState/EmptyState`       | `src/components/profile/ProfileEmptyState`                                                      | Empty message                 | `apps/sites` profile page                      |
| `useBrokenImage`              | Image failure detection for provisional avatars                                                 | Removed with avatar consumers | `ProfileAvatar`, `profile/ProfileAvatarBody`   |
