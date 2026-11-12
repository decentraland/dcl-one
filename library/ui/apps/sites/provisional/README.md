# Provisional Sites UI

Components ported from the current `decentraland/sites` pages to CSS Modules and `library/ui` tokens, so the account and profile pages can move to `apps/sites` before their Figma components exist. Each file carries a `TODO(figma-ds)` mark. When the Figma-generated component lands in `library/ui/components`, its consumers switch to it and the provisional one is deleted, together with its row here.

A test in `apps/sites` fails when a component in this folder is missing from this table.

| Component                     | Source in `decentraland/sites`                             | Expected Figma component | Consumers                 |
| ----------------------------- | ---------------------------------------------------------- | ------------------------ | ------------------------- |
| `ProfileAvatar/ProfileAvatar` | `src/components/profile/ProfileAvatar`                     | Avatar                   | `profile/ProfileHeader`   |
| `profile/ProfileLayout`       | `src/components/profile/ProfileLayout`                     | Profile layout           | `apps/sites` profile page |
| `profile/ProfileHeader`       | `src/components/profile/ProfileHeader`                     | Profile header           | `apps/sites` profile page |
| `profile/ProfileTabs`         | `src/components/profile/ProfileTabs`                       | Tabs                     | `apps/sites` profile page |
| `profile/ProfileOverview`     | `src/pages/profile/tabs/OverviewTab`                       | Profile overview         | `apps/sites` profile page |
| `profile/ProfileAvatarBody`   | `src/components/profile/AvatarRender` (its fallback image) | Avatar render            | `apps/sites` profile page |
| `profile/ProfileActions`      | `src/components/profile/ProfileHeader` (its buttons)       | Button                   | `apps/sites` profile page |
| `profile/ProfileMobileNav`    | `src/components/profile/ProfileMobileMenu`                 | Profile mobile menu      | `apps/sites` profile page |
| `profile/icons`               | the Material icons those pages use                         | Icons                    | `profile/*`               |
