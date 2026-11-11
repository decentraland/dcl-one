# Figma component migration inventory

Last checked: 2026-11-11. This inventory separates captured previews, public runtime contracts and application adoption. A generated preview is not an app-ready component.

## Sources and states

This ledger is owned by dcl-one. Availability is checked against this repository’s [public exports](../library/ui/index.ts), [promotion registry](../library/ui/components/promotion.json) and [review manifest](../library/ui/figma/review/src/generated/components/manifest.ts). The requested component names below are a migration backlog, not another library to import. Baseline checked: dcl-one commit `1058d9a6317a1394b91ef6ff3d483e384d9d16f4`. Local preview copies do not change the public package contract.

- **Public:** exported from [index.ts](../library/ui/index.ts), installed under `components/`, and recorded by [components/promotion.json](../library/ui/components/promotion.json). Read each component’s accepted limits before adoption.
- **Review only:** captured code exists under `figma/review/src/generated/`; applications must not import it.
- **Pending port:** requested for this library but absent from its tracked review catalogue; integration must follow the repository’s [port workflow](../library/ui/figma/MAINTENANCE.md#refresh-workflow), including the source pin and verification record.
- **Blocked:** current `needsManual` entries or an explicit promotion gate remain unresolved. Reported backlog blockers are recorded separately and must be verified during integration; they do not change current readiness.
- **Adopted:** an application imports the public contract and its consuming flow has been verified. Record consumer path, route, verification date/result and PR URL together.

## Using a component from an app

Load shared foundations once from the application global entry. Consumers that honor package exports can use `@dcl/one-ui/foundations.css`. Sites currently uses a directory alias and imports `@dcl/one-ui/foundations/foundations.css`; preserve that working setup or explicitly map exported CSS subpaths in both the bundler and TypeScript configuration. Import public components from `@dcl/one-ui` only after those aliases resolve the intended local files. The only current public generated component is NavBar, a limited bridge with the constraints listed in the [promotion registry](../library/ui/components/promotion.json) and [library README](../library/ui/README.md). Sites and the Explorer HUD already consume shared foundations. Sites also consumes app-scoped modules under `library/ui/apps/sites/`; those modules are separate from this shared-component inventory. Adoption of a public component contract remains separate work.

Do not import from `figma/review/src/generated/` or a local preview copy. Complete the verified port workflow and confirm the local public export and its contract first, then replace one bounded app component and validate its interactions, accessibility, responsive behavior and consuming routes. Update this inventory in the same PR.

## Component status

Counts: 46 current previews, 71 requested components, 20 shared node identities and 97 distinct identities. Four shared identities have different names. The backlog records eight components with reported `needsManual` entries, plus the separate Progress Bar visual-review gate.

The requested-name column preserves renamed identities by Figma node ID instead of counting a renamed component twice. The page column shows both pages when an identity moves between them. A requested name does not rename an existing public contract. “Not recorded” means this inventory has no verified app adoption for that contract. Verify current blocker cases in this repository’s review manifest; blockers reported for pending components remain unverified until their integration.

| Current component      | Requested component   | Figma page                   | Node        | Library status         | App adoption | Blocking work                                                               |
| ---------------------- | --------------------- | ---------------------------- | ----------- | ---------------------- | ------------ | --------------------------------------------------------------------------- |
| Not in current capture | ButtonSplit           | Button Split                 | 14113:6829  | Pending port           | Not recorded | Reported backlog: state-without-base-variant (16 cases)                     |
| Button                 | Button                | Buttons                      | 15721:13657 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| CreditsButton          | ButtonCredits         | Buttons                      | 15724:581   | Review only            | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | ButtonDownload        | Buttons                      | 16305:929   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | ButtonFriend          | Buttons                      | 16534:723   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | ButtonIcon            | Buttons                      | 12886:779   | Pending port           | Not recorded | Reported backlog: state-without-base-variant (60 cases)                     |
| FriendCTA              | Not requested         | Buttons                      | 8071:1665   | Review only            | Not recorded | Public contract and consumer validation pending                             |
| JumpInCTA              | JumpInButton          | Buttons                      | 15724:536   | Review only            | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | Checkbox              | Checkbox                     | 16719:607   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | CheckboxLabel         | Checkbox                     | 16713:28842 | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | PriceRange            | Controllers                  | 16872:385   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | Slider                | Controllers                  | 5242:1968   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | SliderBar             | Controllers                  | 16905:846   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | SliderRangeBar        | Controllers                  | 15796:182   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | DateCalendarDay       | Drop-downs                   | 15707:229   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | DatePresentItem       | Drop-downs                   | 15707:242   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | DateRangeDropdown     | Drop-downs                   | 15834:984   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | DateRangePicker       | Drop-downs                   | 15708:602   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | DateRangePickerMobile | Drop-downs                   | 15834:7     | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | DateRangeTrigger      | Drop-downs                   | 15709:370   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | DropdownEntry         | Drop-downs                   | 16855:1284  | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | DropdownField         | Drop-downs                   | 16856:1434  | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | DropdownPanel         | Drop-downs                   | 16855:1530  | Pending port           | Not recorded | Reported backlog: slot-property (1 cases)                                   |
| Not in current capture | DropdownSecondary     | Drop-downs                   | 16864:1552  | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | DropdownTrigger       | Drop-downs                   | 16855:1389  | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | FriendEntry           | Friends                      | 16321:172   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| WebTheme               | Not requested         | Layout & Spacing             | 15521:20151 | Review only; blocked   | Not recorded | Current: slot-property (1 cases)                                            |
| WebThemeDarkMode       | Not requested         | Layout & Spacing             | 15521:22173 | Review only; blocked   | Not recorded | Current: slot-property (1 cases)                                            |
| Not in current capture | BoldContextMenu       | Menus                        | 16673:785   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | BoldMenu              | Menus                        | 16673:641   | Pending port           | Not recorded | Reported backlog: slot-property (1 cases)                                   |
| Not in current capture | BoldMenuItem          | Menus                        | 16673:640   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | MenuHeader            | Menus                        | 16689:628   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | RegularMenu           | Menus                        | 16902:59984 | Pending port           | Not recorded | Reported backlog: slot-property (1 cases)                                   |
| Not in current capture | RegularMenuItem       | Menus                        | 16902:59580 | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| ShopModals             | ShopModals            | Modals                       | 15906:10331 | Review only; blocked   | Not recorded | Current: slot-property (1 cases); Reported backlog: slot-property (1 cases) |
| Backpack               | Not requested         | Nav bar                      | 15841:55351 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| CameraReel             | Not requested         | Nav bar                      | 15841:55336 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| CloseCTA               | Not requested         | Nav bar                      | 15841:55389 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| Communities            | Not requested         | Nav bar                      | 15841:55318 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| Events                 | Not requested         | Nav bar                      | 15841:55369 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| Map                    | Not requested         | Nav bar                      | 15841:55344 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| Market                 | Not requested         | Nav bar                      | 15841:55329 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| NavBar                 | Not requested         | Nav bar                      | 15599:30721 | Public, limited bridge | Not recorded | See accepted bridge limits                                                  |
| NavBarExplorer         | Not requested         | Nav bar                      | 15845:1842  | Review only            | Not recorded | Public contract and consumer validation pending                             |
| NavCreditsBalance      | Not requested         | Nav bar                      | 15599:31621 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| NavMenuDropdown        | Not requested         | Nav bar                      | 15599:30668 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| NavMenuItem            | Not requested         | Nav bar                      | 15599:30646 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| NavMenuTab             | Not requested         | Nav bar                      | 15599:30557 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| NavMenuTabMobile       | Not requested         | Nav bar                      | 15599:30658 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| NavNetworkOption       | Not requested         | Nav bar                      | 15599:32334 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| NavNetworkSelector     | Not requested         | Nav bar                      | 15599:32285 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| NavNotificationItem    | Not requested         | Nav bar                      | 15599:32384 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| NavNotificationsButton | Not requested         | Nav bar                      | 15599:32355 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| NavProfileButton       | Not requested         | Nav bar                      | 15599:31680 | Review only; blocked   | Not recorded | Current: hidden-variants-excluded (1 cases)                                 |
| NavProfileMenu         | Not requested         | Nav bar                      | 15599:31943 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| Places                 | Not requested         | Nav bar                      | 15841:55380 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| Quest                  | Not requested         | Nav bar                      | 15841:55362 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| Settings               | Not requested         | Nav bar                      | 15841:55311 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | NavArrow              | Navigation                   | 16726:88    | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | PageNumber            | Navigation                   | 16727:57    | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | Pagination            | Navigation                   | 16727:89    | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | PaginationCarrousel   | Navigation                   | 16726:149   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | PaginationDots        | Navigation                   | 16726:127   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | Step                  | Navigation                   | 16727:114   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | StepConnector         | Navigation                   | 16727:100   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | StepIndicator         | Navigation                   | 16727:97    | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | Stepper               | Navigation                   | 16727:151   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | TextArrow             | Navigation                   | 16726:123   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | Photo                 | Photos                       | 16447:59129 | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | PillIconText          | Pills                        | 8712:2783   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | PillText              | Pills                        | 9799:4677   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| PlaceCard              | PlaceCard             | Places                       | 16189:2716  | Review only            | Not recorded | Public contract and consumer validation pending                             |
| ConnectionStatus       | ConnectionStatus      | Profile Picture              | 12877:3067  | Review only            | Not recorded | Public contract and consumer validation pending                             |
| ProfilePic             | ProfilePic            | Profile Picture              | 8064:1628   | Review only            | Not recorded | Public contract and consumer validation pending                             |
| ProgressBar            | ProgressBar           | Progress Bar                 | 8711:19210  | Review only; blocked   | Not recorded | Current: Pending visual review; Reported backlog: Pending visual review     |
| Not in current capture | Radio                 | Radio                        | 16751:2562  | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | RadioLabel            | Radio                        | 16740:33387 | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | Scrollbar             | Scrollbar                    | 16449:13    | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| BlogSearchResultItem   | BlogSearchResultItem  | Search Bar                   | 15715:13    | Review only; on hold   | Not recorded | Hold: await updated component list                                          |
| SearchBar              | SearchBar             | Search Bar                   | 3959:1973   | Review only; on hold   | Not recorded | Hold: await updated component list                                          |
| SearchResultItem       | SearchResultItem      | Search Bar                   | 15714:19    | Review only; on hold   | Not recorded | Hold: await updated component list                                          |
| Skeleton               | SkeletonAnimation     | Spinner & Skeleton Animation | 16228:10    | Review only            | Not recorded | Public contract and consumer validation pending                             |
| Spinner                | Spinner               | Spinner & Skeleton Animation | 4275:1730   | Review only            | Not recorded | Public contract and consumer validation pending                             |
| ShopFilters            | ShopFilters           | Tabs bar                     | 15843:57003 | Review only; blocked   | Not recorded | Current: slot-property (1 cases); Reported backlog: slot-property (1 cases) |
| Tabs                   | Tabs                  | Tabs bar                     | 15674:34262 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| TabsBar                | TabsBar               | Tabs bar                     | 15808:46232 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| TitleBar               | TitleBar              | Tabs bar                     | 15843:56922 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| InputExplorerNames     | InputExplorerNames    | Text Input                   | 7522:2940   | Review only            | Not recorded | Public contract and consumer validation pending                             |
| InputWebCreatorTools   | InputWebCreatorTools  | Text Input                   | 15819:11187 | Review only            | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | Toast                 | Toasts & Tooltips            | 7502:2312   | Pending port           | Not recorded | Reported backlog: slot-property (1 cases)                                   |
| Not in current capture | TooltipDark           | Toasts & Tooltips            | 16846:53476 | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | MarkAsFavorite        | Toggle                       | 8706:7405   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | SegmentedSwitchIcon   | Toggle                       | 15772:654   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | SegmentedSwitchText   | Toggle                       | 16810:199   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | Switch                | Toggle                       | 8706:7603   | Pending port           | Not recorded | Public contract and consumer validation pending                             |
| CardActionButton       | ToggleIconButton      | Buttons → Toggle             | 16192:306   | Review only            | Not recorded | Public contract and consumer validation pending                             |
| Not in current capture | ToggleIconTextButton  | Toggle                       | 5324:1731   | Pending port           | Not recorded | Public contract and consumer validation pending                             |

## Capture gaps

Shop and Events are not yet represented in this backlog. Search Bar, Notifications and Backgrounds are on hold until the updated component list is available. Do not promote the existing SearchBar, SearchResultItem or BlogSearchResultItem entries while that hold is active. Foundation readiness is recorded separately in this repository’s library README and source lock.

## Updating this inventory

The author of each verified port or application migration updates this tracked ledger in that same PR; reviewers check it against the package export, promotion registry and consuming code. For a port, record this repository’s component path, current and requested identities, source pin, verification record, status and unresolved blockers. For app adoption, replace “Not recorded” with the consumer path, consuming route, verification date/result and full PR URL. Keep accepted limitations visible. A local copy, passing build or screenshot alone is insufficient evidence of app readiness.
