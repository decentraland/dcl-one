# Unity Explorer on dcl.dev

November 23–24, 2026. On Windows, a new guest entered the world at 100,100. Two signed-in players became friends, exchanged private messages in both directions and accepted a voice call. Browser wallet and social-provider sign-in failed on the live deployment. The macOS check covered welcome and login selection.

To select the deployment, run the following command from the extracted Windows CI player directory.

```powershell
.\Decentraland.exe --gateway https://dcl.dev
```

[Implementation](https://github.com/decentraland/unity-explorer/pull/10686) · [Windows CI player](https://explorer-artifacts.decentraland.org/@dcl/unity-explorer/branch/feat/realm-scene-pointers-pulse-server/pr-28116-7308da1/Decentraland_windows64.zip) · [macOS CI player](https://explorer-artifacts.decentraland.org/@dcl/unity-explorer/branch/feat/realm-scene-pointers-pulse-server/pr-28116-7308da1/Decentraland_macos.zip) · [Tracking issue](https://github.com/decentraland/dcl-one/issues/1281)

## Findings

| Issue | Finding | Latest evidence | Related issues |
| --- | --- | --- | --- |
| [#2052](https://github.com/decentraland/dcl-one/issues/2052) | Browser wallet sign-in fails before returning to the player | Browser handoff blocked. The identity API hostname still failed DNS on November 24. | [Native-auth tracker #1035](https://github.com/decentraland/dcl-one/issues/1035) · [Relay parity #1595](https://github.com/decentraland/dcl-one/issues/1595) |
| [#2054](https://github.com/decentraland/dcl-one/issues/2054) | Friends and incoming requests show a generic silhouette instead of the friend's avatar | Build 28116 showed the friend's name and dressed 3D avatar. Its 2D portrait still returned HTTP 404. | [Existing rendering gap #1584](https://github.com/decentraland/dcl-one/issues/1584) |
| [#2059](https://github.com/decentraland/dcl-one/issues/2059) | Google and Apple sign-in open a wallet-only page | Google, Apple, Discord and X selections all opened a wallet-only page. | [Native-auth tracker #1035](https://github.com/decentraland/dcl-one/issues/1035) |
| [#2060](https://github.com/decentraland/dcl-one/issues/2060) | Mirrored communities show the default icon in Browse and community details | Several imported communities had missing cover images. The same community is tracked in website QA. | [Existing import gap #1564](https://github.com/decentraland/dcl-one/issues/1564) · [Same community in Sites QA #1122](https://github.com/decentraland/dcl-one/issues/1122) |
| [#2062](https://github.com/decentraland/dcl-one/issues/2062) | WebP event posters are blank in Events and community listings | WebP posters appeared white. A PNG rendered on the same screen. | [Related event GIF decode failure #4621](https://github.com/decentraland/unity-explorer/issues/4621) · [Related website QA #1123](https://github.com/decentraland/dcl-one/issues/1123) |
| [#2063](https://github.com/decentraland/dcl-one/issues/2063) | Genesis Plaza reports ready while its buildings and plaza floor are missing | Fixed in the PR. Build 28116 rendered paved streets, buildings and props with the gateway defaults, exceeding 1.1 million triangles from 210 sources. | [Prior Genesis symptom #9074](https://github.com/decentraland/unity-explorer/issues/9074) · [Registry migration #1623](https://github.com/decentraland/dcl-one/issues/1623) |
| [#2064](https://github.com/decentraland/dcl-one/issues/2064) | Map zoom reaches its maximum while landmarks stay the same size on a slow player | Reproduced on the software-rendered player at 0.8 FPS. Hardware verification remains open. | [Expected zoom range #2665](https://github.com/decentraland/unity-explorer/issues/2665) |
| [#2066](https://github.com/decentraland/dcl-one/issues/2066) | Community Places owner lookup leaves an unrelated error on the NAME requirement dialog | An account without a NAME saw the ownership error while opening Create Community. | [Same warning and dialog #7013](https://github.com/decentraland/unity-explorer/issues/7013) |
| [#2068](https://github.com/decentraland/dcl-one/issues/2068) | Backpack Emotes shows only 3 of 16 thumbnails at 60 seconds and fills by 120 seconds | Slow loading on hosted Windows. Build 28116 showed 0 of 16 images at 30 seconds, 3 at 60 seconds and all 16 at 120 seconds. Selecting Clap worked. | [Earlier macOS symptom #6814](https://github.com/decentraland/unity-explorer/issues/6814) |
| [#2069](https://github.com/decentraland/dcl-one/issues/2069) | FAQ opens a documentation hostname that does not resolve | Fixed in the PR. The sidebar FAQ opened Decentraland 101 in Edge on build 28113. | [FAQ requirement #8616](https://github.com/decentraland/unity-explorer/issues/8616) |
| [#2070](https://github.com/decentraland/dcl-one/issues/2070) | Gallery Copy Link sends photos to an unresolved reels hostname | Unity still generates a dead hostname. The deployed gateway now serves the same photo. | [Viewer tracker #1120](https://github.com/decentraland/dcl-one/issues/1120) |
| [#2071](https://github.com/decentraland/dcl-one/issues/2071) | Passport Creations calls the unavailable marketplace subdomain and shows an error | Fixed in the PR. Build 28116 rendered both the empty state and a creator with 12 wearables and 2 emotes. | [Creations requirement #9334](https://github.com/decentraland/unity-explorer/issues/9334) |
| [#2075](https://github.com/decentraland/dcl-one/issues/2075) | Hardware warning truncates the GPU requirement at 1080p | The 1080p hardware warning clipped the DirectX 12 requirement. | [Hardware requirements #3983](https://github.com/decentraland/unity-explorer/issues/3983) |
| [#2083](https://github.com/decentraland/dcl-one/issues/2083) | Backpack Equip spins without applying a displayed base shirt | The same shirt equipped within 60 seconds on build 28116 and stayed equipped at 120 seconds. The earlier stall's exact cause remains open. | [Earlier macOS symptom #2505](https://github.com/decentraland/unity-explorer/issues/2505) |
| [#2084](https://github.com/decentraland/dcl-one/issues/2084) | Read More on the pre-login hardware warning opens an unresolvable hostname | Fixed in the PR. Read More opened the hardware question in the browser viewport on build 28116. | [Hardware help requirement #3983](https://github.com/decentraland/unity-explorer/issues/3983) · [Related sidebar FAQ #2069](https://github.com/decentraland/dcl-one/issues/2069) |
| [#2088](https://github.com/decentraland/dcl-one/issues/2088) | View on a Passport Creations item sends players to an unresolved marketplace host | Fixed in the PR. On build 28116, View opened the matching Chiri Pantuflas item in the Shop with HTTP 200. | [Related catalog fix #2071](https://github.com/decentraland/dcl-one/issues/2071) |
| [#2143](https://github.com/decentraland/dcl-one/issues/2143) | A timed-out Genesis teleport can leave the avatar suspended without an active scene | One of two matched trips left the player at 100,100 without an active scene for two minutes. A subsequent trip recovered the session. | [Earlier lost-ground issue #9517](https://github.com/decentraland/unity-explorer/issues/9517) · [Open timeout tracker #8721](https://github.com/decentraland/unity-explorer/issues/8721) |
| [#2144](https://github.com/decentraland/dcl-one/issues/2144) | Two equipped-item previews stay gray in a friend's Passport after 95 seconds | On build 28116, two of ten peer item previews stayed gray after 95 seconds. The two source PNGs were available. | [Separate Friends portrait issue #2054](https://github.com/decentraland/dcl-one/issues/2054) |

The issues above are children of [dcl-one #1281](https://github.com/decentraland/dcl-one/issues/1281) and are in the [dcl-one soft launch Backlog](https://github.com/orgs/decentraland/projects/134/views/1). Each links the relevant prior work and records how much of the proposed diagnosis was verified.

## Related work

High relevance means a PR fixes the finding or changes a directly implicated feature. Medium covers the affected component or an earlier failure of the same kind. Low covers adjacent behavior or another client.

| PR link | Importance for the subject | What it fixes |
| --- | --- | --- |
| [unity-explorer #10686](https://github.com/decentraland/unity-explorer/pull/10686) · open | **High** · Five verified fixes in the native gateway configuration. [#2063](https://github.com/decentraland/dcl-one/issues/2063) · [#2069](https://github.com/decentraland/dcl-one/issues/2069) · [#2071](https://github.com/decentraland/dcl-one/issues/2071) · [#2084](https://github.com/decentraland/dcl-one/issues/2084) · [#2088](https://github.com/decentraland/dcl-one/issues/2088) | Selects public ABGen assets for Genesis, routes the FAQ and hardware warning to dcl.dev Docs, and routes Passport’s catalog and item links through the gateway. |
| [dcl-one #1851](https://github.com/decentraland/dcl-one/pull/1851) · merged | **High** · Contains the data migration for the exact missing community thumbnails. [#2060](https://github.com/decentraland/dcl-one/issues/2060) | Copies community images into the local blob store and links them to the community. Verify that the dcl.dev migration included the affected IDs. |
| [dcl-one #2034](https://github.com/decentraland/dcl-one/pull/2034) · merged | **High** · Provides the working destination for the same photo copied by Unity. [#2070](https://github.com/decentraland/dcl-one/issues/2070) | Restores direct photo links under `/reels/<id>`. The deployed route served the QA photo. Unity still copied the unresolved `reels.dcl.dev` hostname. |
| [unity-explorer #9784](https://github.com/decentraland/unity-explorer/pull/9784) · merged | **High** · Merged recovery fix for an earlier teleport timeout that left the player without ground. [#2143](https://github.com/decentraland/dcl-one/issues/2143) | Prevents a cancelled teleport’s teardown from racing recovery and removing ground and LODs. The same-realm dcl.dev failure still reproduced in one matched trip. |
| [unity-explorer #9828](https://github.com/decentraland/unity-explorer/pull/9828) · merged | **High** · Merged change in the shared thumbnail loader used by the affected cards. [#2144](https://github.com/decentraland/dcl-one/issues/2144) | Allows a later thumbnail request to retry after an earlier failure. Whether either gray Passport card retried while it remained visible is unverified. |
| [dcl-one #1223](https://github.com/decentraland/dcl-one/pull/1223) · merged | **High** · Directly affects missing-snapshot profiles in Friends and incoming requests. [#2054](https://github.com/decentraland/dcl-one/issues/2054) | Keeps the friend in lists and assigns a face URL. QA confirmed the name and relationship appeared, but the generated face URL still returned 404. |
| [dcl-one #219](https://github.com/decentraland/dcl-one/pull/219) · merged | **High** · Implements the four providers Unity offers on the website’s regular login route. [#2059](https://github.com/decentraland/dcl-one/issues/2059) | Adds Magic OAuth sign-in and return handling for Google, Apple, Discord and X. The native request page still showed only the wallet action. |
| [dcl-one #1753](https://github.com/decentraland/dcl-one/pull/1753) · merged | **Medium** · Changes the same identity handoff after the browser reaches the API. [#2052](https://github.com/decentraland/dcl-one/issues/2052) | Binds verifier-based deep-link requests to identity pickup. Explorer keeps its existing unbound pickup flow. The browser’s failing API hostname is still unresolved. |
| [dcl-one #2121](https://github.com/decentraland/dcl-one/pull/2121) · open | **Medium** · Defines the HTTP behavior the browser needs after DNS succeeds. [#2052](https://github.com/decentraland/dcl-one/issues/2052) | Proposes restoring the relay’s origin allowlist, error responses and request parsing under the gateway. |
| [dcl-one #1834](https://github.com/decentraland/dcl-one/pull/1834) · merged | **Medium** · Applies when a synced profile already advertises a snapshot hash. [#2054](https://github.com/decentraland/dcl-one/issues/2054) | Fetches and records existing avatar renders during sync. Fresh QA profiles had no snapshot hash to import. |
| [dcl-one #211](https://github.com/decentraland/dcl-one/pull/211) · merged | **Medium** · Introduces the web login screen that exposes the provider choices. [#2059](https://github.com/decentraland/dcl-one/issues/2059) | Adds provider choices and email and wallet dialogs at `/auth/login`. Unity opens a separate `/auth/requests/:id` page. |
| [dcl-one #316](https://github.com/decentraland/dcl-one/pull/316) · merged | **Medium** · Explains why Unity receives the original WebP data for these posters. [#2062](https://github.com/decentraland/dcl-one/issues/2062) | Validates media types from their bytes. A request without a width can return WebP unchanged. The audit rendered the JPEG derivative successfully. |
| [unity-explorer #9301](https://github.com/decentraland/unity-explorer/pull/9301) · merged | **Medium** · Fixes an earlier missing-plaza symptom after a world change. [#2063](https://github.com/decentraland/dcl-one/issues/2063) | Restores visibility of shared AssetBundle renderers after loading. The dcl.dev comparison traced the missing geometry to a mismatched conversion manifest. |
| [unity-explorer #1176](https://github.com/decentraland/unity-explorer/pull/1176) · merged | **Medium** · Addresses an earlier delay in the same Backpack Emotes catalog. [#2068](https://github.com/decentraland/dcl-one/issues/2068) | Removes an obsolete embedded emote from a secondary list. All configured pointers resolved in this audit, and the cause of the 120-second load remains unconfirmed. |
| [unity-explorer #8751](https://github.com/decentraland/unity-explorer/pull/8751) · merged | **Medium** · Introduces the Help & Support action used to reproduce the problem. [#2069](https://github.com/decentraland/dcl-one/issues/2069) | Consolidates the sidebar’s help actions and exposes the FAQ entry that opens Decentraland 101. |
| [unity-explorer #9114](https://github.com/decentraland/unity-explorer/pull/9114) · merged | **Medium** · Introduces the affected Unity Creations grid. [#2071](https://github.com/decentraland/dcl-one/issues/2071) · [#2088](https://github.com/decentraland/dcl-one/issues/2088) | Displays a player’s published wearables and emotes in Passport. The gateway PR corrects its catalog and item destinations. |
| [unity-explorer #4608](https://github.com/decentraland/unity-explorer/pull/4608) · merged | **Medium** · Introduces the warning used in both hardware-screen findings. [#2075](https://github.com/decentraland/dcl-one/issues/2075) · [#2084](https://github.com/decentraland/dcl-one/issues/2084) | Adds the minimum-specifications screen, hardware checks and a forced-display option. The clipping and Read More findings exercise that screen. |
| [unity-explorer #6522](https://github.com/decentraland/unity-explorer/pull/6522) · merged | **Medium** · Defines when the affected Equip button should show and clear its spinner. [#2083](https://github.com/decentraland/dcl-one/issues/2083) | Shows progress while Unity downloads and equips a wearable. The blue shirt equipped on build 28116, but the cause of the earlier stall remains unknown. |
| [unity-explorer #9777](https://github.com/decentraland/unity-explorer/pull/9777) · closed without merging | **Medium** · Contains the original diagnosis and regression test incorporated into [unity-explorer #9784](https://github.com/decentraland/unity-explorer/pull/9784). [#2143](https://github.com/decentraland/dcl-one/issues/2143) | Proposed guarding the cancelled fallback before teardown. The change shipped through [unity-explorer #9784](https://github.com/decentraland/unity-explorer/pull/9784). |
| [unity-explorer #9790](https://github.com/decentraland/unity-explorer/pull/9790) · closed without merging | **Medium** · Documents the original timeout, cached failure and retry proposal incorporated into [unity-explorer #9828](https://github.com/decentraland/unity-explorer/pull/9828). [#2144](https://github.com/decentraland/dcl-one/issues/2144) | Proposed clearing the failed thumbnail state so subsequent requests can retry. The merged implementation is in [unity-explorer #9828](https://github.com/decentraland/unity-explorer/pull/9828). |
| [dcl-one #2125](https://github.com/decentraland/dcl-one/pull/2125) · open | **Low** · Covers a different relay transport and the Creator Hub client. [#2052](https://github.com/decentraland/dcl-one/issues/2052) | Proposes restoring Socket.IO request and outcome delivery and moving Creator Hub identity pickup into the main process. |
| [dcl-one #1941](https://github.com/decentraland/dcl-one/pull/1941) · merged | **Low** · Confirms which migration owns thumbnails while changing a different part of the import. [#2060](https://github.com/decentraland/dcl-one/issues/2060) | Imports community ranking counters and reports uncatalogued places. It identifies [dcl-one #1851](https://github.com/decentraland/dcl-one/pull/1851) as the change that populates the thumbnail rows. |
| [dcl-one #2056](https://github.com/decentraland/dcl-one/pull/2056) · merged | **Low** · Addresses a similar cover-image symptom on the website. [#2062](https://github.com/decentraland/dcl-one/issues/2062) | Restores uploaded event covers on HTTP origins in Sites. The Unity reproduction used HTTPS and failed while decoding the poster. |
| [unity-explorer #10457](https://github.com/decentraland/unity-explorer/pull/10457) · merged | **Low** · Describes expected map detail as the camera moves closer. [#2064](https://github.com/decentraland/dcl-one/issues/2064) | Streams higher-resolution satellite tiles for zoom levels 3–8 and keeps bundled imagery as a fallback. The observed camera interpolation failure on slow frames remains open. |
| [dcl-one #743](https://github.com/decentraland/dcl-one/pull/743) · closed without merging | **Low** · Documents the intended website paths and redirects. [#2069](https://github.com/decentraland/dcl-one/issues/2069) | Proposed localized documentation pages and redirects to their mirror paths. The native URL change is in [unity-explorer #10686](https://github.com/decentraland/unity-explorer/pull/10686). |
| [dcl-one #2073](https://github.com/decentraland/dcl-one/pull/2073) · open | **Low** · Changes sharing controls in the HUD’s photo viewer. [#2070](https://github.com/decentraland/dcl-one/issues/2070) | Proposes a photo information panel and share, visibility and download controls in the HUD. Unity’s native clipboard URL is configured separately. |
| [dcl-one #294](https://github.com/decentraland/dcl-one/pull/294) · merged | **Low** · Provides a comparison for expected empty and error states in the HUD. [#2071](https://github.com/decentraland/dcl-one/issues/2071) | Adds the Creations tab to the dcl-one HUD with item actions and separate empty-catalog and lookup-error states. |
| [unity-explorer #10588](https://github.com/decentraland/unity-explorer/pull/10588) · merged | **Low** · Improves access to the same action on the last row of the grid. [#2088](https://github.com/decentraland/dcl-one/issues/2088) | Adds space below Creations cards so hovering the last row exposes the Buy/View controls. The target URL is configured separately. |
| [unity-explorer #10150](https://github.com/decentraland/unity-explorer/pull/10150) · open | **Low** · Moves another Passport action to the Shop. [#2088](https://github.com/decentraland/dcl-one/issues/2088) | Proposes sending Claim a NAME to the Shop from Passport and other entry points. The item View button has its own route. |
| [dcl-one #79](https://github.com/decentraland/dcl-one/pull/79) · merged | **Low** · Shows how another client restores control after a failed scene load. [#2143](https://github.com/decentraland/dcl-one/issues/2143) | Makes the Bevy client retry server errors and release the player when a scene fails to load. Unity has a separate recovery path. |
| [unity-explorer #4065](https://github.com/decentraland/unity-explorer/pull/4065) · merged | **Low** · Fixes an earlier Passport problem where equipped items were missing from the list. [#2144](https://github.com/decentraland/dcl-one/issues/2144) | Includes force-rendered items in Passport’s Equipped Items list. All ten item names were present in this audit. |

The [Community NAME warning](https://github.com/decentraland/dcl-one/issues/2066) has a [matching Unity issue](https://github.com/decentraland/unity-explorer/issues/7013) with no linked fix PR.

## Coverage

The Windows screen audit used the [official CI player at `7469fb4`](https://github.com/decentraland/unity-explorer/actions/runs/45204922809) with a software Direct3D 11 renderer. The FAQ and Passport catalog retests used the [CI player at `ef5a4ce`](https://github.com/decentraland/unity-explorer/actions/runs/45216048757). The final checks used [build 28116 at `7308da1`](https://github.com/decentraland/unity-explorer/actions/runs/45226322119) on Windows and macOS. Signed-in checks used disposable, pre-provisioned Catalyst profiles. The two-client checks used separate identities with a real friendship on dcl.dev. [Two-client audit](https://github.com/decentraland/unity-explorer/actions/runs/45215896630) · [Final player audit](https://github.com/decentraland/unity-explorer/actions/runs/45342967461)

The six checked gateway regression cases passed. GitHub reported successful Unity test jobs. The EditMode XML recorded 29,904 passing cases, 13 skipped cases and a separate assembly teardown error in SingletonRegistry.Reset. [Unity CI results](https://github.com/decentraland/unity-explorer/actions/runs/45226322356)

Each issue embeds original PNGs as GitHub attachments. Most Windows player frames are 1920×1080. Some login windows resized during handoff. Edge screenshots are 2880×1800. Detail crops preserve the original pixels.

### Login and onboarding

| Screen or action | Verified result |
| --- | --- |
| Welcome and login selection | The CI player displayed the guest and sign-in paths. The login screen showed email, Google and MetaMask. More options opened the additional provider choices. |
| Wallet sign-in | A disposable wallet signed a Decentraland session request on the live dcl.dev page. The browser's identity submission failed on the advertised API hostname and Unity kept waiting. In a controlled comparison, correcting the API base in that browser session returned HTTP 201, produced the matching launcher callback and opened Unity onboarding. [Wallet issue](https://github.com/decentraland/dcl-one/issues/2052) |
| Social and wallet providers | Google, Apple, Discord, X, WalletConnect, Coinbase and Fortmatic each reached a dcl.dev page with the selected method in its query. Each page offered the wallet button in a fresh browser. Google, Apple, Discord and X offered no social sign-in. Provider-specific wallet extensions and WalletConnect pairing were not independently completed. [Provider-selection issue](https://github.com/decentraland/dcl-one/issues/2059) |
| Email and one-time code | The native form displayed its invalid-email state for a reserved, undeliverable test address. Actual inbox delivery, code verification and account recovery require a controlled mailbox. |
| Guest appearance and avatar preview | The preset selection, body-type menu, and Continue controls rendered. The chosen green-jacket preset appeared in the profile editor and on the guest avatar after world entry. The second body type changed all four starter cards and the full-size 3D preview on a fresh authenticated login. |
| Guest profile and Jump In | Entering a username and accepting the terms enabled Jump In. One disposable guest completed the transition and appeared at 100,100 in the interactive-text scene. [Guest CI run](https://github.com/decentraland/unity-explorer/actions/runs/45210711754/job/137818430506) |

The wallet flow used an ephemeral test identity. On the hosted machine the test driver passed the browser's genuine, matching callback to Unity through the launcher bridge file. The OS-level protocol registration and first-party launcher installation were outside the hosted test. No blockchain transaction or real provider account was used.

### Hardware warning before login

The CI player displayed **Performance Adjusted to Your Device** with a detected CPU and GPU, a **Don't show this message again** checkbox, **Read More** and **Continue**. The GPU requirement was truncated in the 1080p display configuration. The audit used the client's forced-display option to reach this conditional screen. [Original screenshot and investigation](https://github.com/decentraland/dcl-one/issues/2075)

**Read More** opened an unresolvable hostname on build 28113. On build 28116 the same button opened the hardware question on Decentraland 101, with the heading in view and HTTP 200. **Continue** advanced past the warning to the loading screen. [Before and after](https://github.com/decentraland/dcl-one/issues/2084)

### World, Map, Places and Events

For the eight most recent active dcl.dev deployments in each of three categories (scene, wearable and emote), the realm, public Catalyst and public ABGen registry returned matching entity IDs for all 24. Every matching ABGen result included a Windows conversion version. This sampled deployment metadata complements the byte and player checks below.

The Windows and macOS ABGen manifests each included all 470 GLB paths in the active Genesis Plaza scene (468 distinct content hashes). Windows scene counters and screenshots independently confirm the player instantiated the clock tower, street benches and several buildings.

| Surface | Evidence and current result |
| --- | --- |
| Signed-in world HUD at 100,100 | In final Windows CI build 28116 (`7308da1`), `--gateway https://dcl.dev` was the only deployment selector. The player landed on the ready interactive-text scene at 100,100 with the HUD, minimap, scene label and avatar visible. At 30 seconds the avatar was grounded and standing on the scene. Its model `models/SCENE.glb` contributed all 142 visible triangles. [Final signed-in player run](https://github.com/decentraland/unity-explorer/actions/runs/45342967461) |
| Genesis Plaza at -1,-1 | In final Windows CI build 28116 the gateway selected ABGen automatically. The grounded player stood at 0,0 on the active plaza. By 120 seconds the theatre, DCL Store, paving, vegetation and furniture were visible, with 1,132,242 scene triangles across 210 geometry sources. The fixed camera reported 1,221,215 triangles across 214. Earlier signed-in A/B runs on the same scene returned 10,280 primitive triangles using legacy versus 944,453 triangles from 184 sources using ABGen. [Final native player](https://github.com/decentraland/unity-explorer/actions/runs/45342967461) · [Controlled comparison](https://github.com/decentraland/unity-explorer/actions/runs/45217514764) |
| Same-realm parcel teleport and recovery | One of two matched trips from 100,100 to Genesis using ABGen showed a native chat timeout and an avatar above a void. The player logged its two-minute teleport exception, retained the old coordinates and had no active scene when the loading screen closed. The audit's 130-second waiting period expired afterward. Those readings persisted at both follow-ups. A subsequent trip to MetaZoo recovered that session, and a separate run completed the original route. Final build 28116 also logged a teleport timeout during direct Genesis startup but showed a grounded avatar with an active scene at 0,0. Exact-target travel and scene rendering have separate evidence. [Teleport recovery issue and screenshots](https://github.com/decentraland/dcl-one/issues/2143) · [Failed trip](https://github.com/decentraland/unity-explorer/actions/runs/45214694517) · [Native recovery history](https://github.com/decentraland/unity-explorer/issues/9517) |
| MetaZoo International at 95,98 | Both fixed-build runs show buildings, glazing and paving after travel. Each renders 17 geometry sources and 199,431 triangles. A complete 18.4 MB legacy model returned the same SHA-256 through the gateway and the public CDN. |
| Map, layers and satellite tiles | The old player rendered its bundled satellite overview and logged DNS failure for `abgen-cdn.dcl.dev`. PR #10686 now uses `https://abgen-cdn.decentraland.org/SatelliteMaps`. That origin returned a 2,004-byte manifest and a referenced 185,522-byte KTX2 tile. The rebuilt player opens Layers, switches to Parcels, and returns to visible satellite imagery. The initial parcel frame contained a partially loaded checkerboard. A 50-second follow-up showed roads, estates and the highlighted plaza. The 1020×1020 parcel chunk around 100,100 also returned a valid PNG. On both builds, three Zoom In clicks disabled the plus button while landmarks stayed in place. The rebuilt player measured 0.8 FPS on the hosted runner. |
| Map search, place information and directions | Submitting “Genesis” displayed multiple relevant locations with thumbnails. Selecting Genesis Plaza moved the map, showed its artwork and coordinates, and populated the Overview, Photos and Events tabs. Start Navigation changed to Exit Navigation. |
| Places | Browse rendered cards with thumbnail artwork, and the selected SDK7 EMG Campus detail showed its artwork, creator, interactions, coordinates and category. The fresh account had zero Favorites and My Places, with explanatory empty states. Recent showed the interactive-text parcel the account had actually visited. Search populated 15 Genesis results and the sort/filter dropdown opened. The public service returned matching results and 57 points of interest. |
| Events | The calendar opens with real events and changes its displayed week. Selecting November 28 opens a ten-event daily grid. SkeleFrenz Gamenight opens its detail panel. Nine daily covers were white. Matching WebP and working PNG controls are documented in the event posters issue. The public service advertised 68 weekly events and two live events, with zero highlights. |

The signed-in screen evidence comes from the Windows CI player, including original 1920×1080 lossless frames in run 45210711754. Screens labelled as menus in pre-fix run 45206767062 still showed the HUD and are excluded from menu coverage. A macOS CI player also reached the native welcome/login screen. Its signed-in world was not inspected in this audit. Network checks on both platforms retrieved actual UnityFS model samples at the starting parcel, Genesis Plaza and MetaZoo, plus six non-base collectible wearables and emotes. A realm-exclusive scene or collectible absent from the public conversion registries was unavailable for this check. [Final macOS startup check](https://github.com/decentraland/unity-explorer/actions/runs/45342967461/job/138270715155)

### Social screens on dcl.dev

The signed-in fixtures use separate dcl.dev profiles and an ordinary friendship request. The CI player launched with `--gateway https://dcl.dev`. Coverage below records what a reviewer could see in the player.

| Screen | Verified observation |
| --- | --- |
| Friends, no relationships | The empty list displayed “Time to Make Some Friends!” |
| Incoming requests | The received row showed the owned test account's display name and date. Clicking the row opened the request dialog. Accept triggered “You And AuditFriend Are Now Friends!”, one Offline friend in Unity and one relationship in the independent signed API read. The portrait slot retained a generic colored silhouette. |
| Accepted friends | The list placed the named account under Offline. It had the same generic silhouette. Both signed friends endpoints returned the relationship. |
| Blocked accounts | The Blocked tab selected and displayed “No Blocked Accounts” for a fresh account. |
| Notifications | The bell displayed a Friend Request Received event with the owned test account's name, age and unread marker on the final player. |
| Nearby chat | The conversation expanded, displayed the DCL System command hint, and exposed the input box. The emoji picker opened showing a search field and emoji grid. The Nearby Members control opened a populated list. |
| Presence, private messages and calls | Two real CI players signed in with distinct QA accounts and each showed the other friend online. The recipient saw “QA blue umbrella” in its direct conversation, and the first player saw the private reply “QA gold ribbon”. Starting a call produced the peer's incoming call with answer and reject controls. The peer accepted, then both showed the ongoing-call UI. The hosted VM has no microphone; acoustic capture, playback and two-way audio quality remain unverified. [Two-player CI run](https://github.com/decentraland/unity-explorer/actions/runs/45215896630/job/137836818554) |
| Own Passport | The profile menu and all four tabs opened. Overview rendered the configured 3D avatar, profile description and ten distinct equipped-item thumbnails. Badges rendered the two categories, icons, detailed locked Emotionista Bronze badge and progress. Photos displayed the fresh account's empty state. Creations showed an explicit lookup error on `7469fb4`. On the `ef5a4ce` Windows CI player it displayed “There are no creations to show yet”. [Empty-account CI retest](https://github.com/decentraland/unity-explorer/actions/runs/45220913997) |
| Friend's Passport | The two-player run rendered the online friend's display name, Friend status, distinct configured hair and outfit, and profile description. The 28116 player independently rendered a newly deployed offline friend's red pigtails, yellow sweater, jeans, and profile text despite its missing 2D portrait. The full registry batch, compact registry batch and realm lambdas all returned this same identity, so this player run did not exercise a missing registry entry. [Final peer Passport](https://github.com/decentraland/unity-explorer/actions/runs/45342967461/job/138270715450) [Preview issue and original screenshots](https://github.com/decentraland/dcl-one/issues/2144) |
| Public creator's Passport | The signed-in player reached Chiri through **Communities → Chiri Storytelling → Members**. Creations displayed 12 wearable cards and two emote thumbnails, matching the independently fetched gateway catalog. Each visible card rendered its own artwork. [Published-creator CI retest](https://github.com/decentraland/unity-explorer/actions/runs/45221983714) |
| Final peer's equipped items and empty catalog | On 28116, eight of ten equipped-item thumbnails loaded. **Bun shoes** and **f_Mouth_00** remained gray after over 95 seconds, although both source PNGs returned HTTP 200. The peer's Creations tab displayed “There are no creations to show yet”. The same build returned Chiri's populated Creations grid in the independent public profile check. [Final fresh-peer Passport](https://github.com/decentraland/unity-explorer/actions/runs/45343858594/job/138273606476) · [Final published creator](https://github.com/decentraland/unity-explorer/actions/runs/45342967461/job/138270715708) |
| Creations item → View | On 28113, **Chiri Pantuflas → View** emitted a `market.dcl.dev` URL and Edge displayed a DNS failure. On the final 28116 player, the same selection opened the dcl.dev Shop item with HTTP 200 and the correct name, art, rarity and creator. [Original click and control](https://github.com/decentraland/unity-explorer/actions/runs/45224796956) · [Final click and Shop page](https://github.com/decentraland/unity-explorer/actions/runs/45342967461/job/138270715708) |
| Communities browser | 195 cards were reported in the signed-in catalog. The active stream, names and counts appeared. Decentraland Foundation, Chiri Storytelling and ZEROxWORK.com used fallback artwork while two neighboring cards showed their images. |
| Community detail | Chiri Storytelling's Announcements loaded dated messages and reactions after the skeletons cleared. Members loaded a populated two-column list, most with visible distinct portraits, with Add Friend controls. Places showed The Impossible Dimension with artwork. Photos displayed “No photos here yet!” Six Upcoming Events retained white poster tiles after the other content loaded. |
| Community invitations and creation | Invites & Requests displayed the correct empty state for the new account. Create a Community opened the NAME ownership requirement and its Cancel / Get a NAME controls. A delayed owner-name error appeared over it after the Places visit. |

The accepted-friend and incoming-request screenshots use independently generated accounts. The color and the unclaimed-name suffix can differ between runs. The nearby chat check did not send a message to the public channel. No NAME was purchased.

### Backpack, settings, camera, Shop and support

The final official macOS player at `7308da1` (28116) also ran with `--gateway https://dcl.dev`. It rendered the native Welcome screen, accepted **Log In or Sign Up**, and rendered the sign-in methods page. The macOS smoke stopped there. [Final CI player run](https://github.com/decentraland/unity-explorer/actions/runs/45342967461)

These checks used the published Windows Unity CI player at `7469fb4`, the `--gateway https://dcl.dev` selector, a disposable signed-in profile and the hosted runner's WARP graphics adapter. The player screenshots are 1920×1080 and the two browser errors are 2880×1800. The captures come from the [full audit](https://github.com/decentraland/unity-explorer/actions/runs/45210711754), [timed inventory check](https://github.com/decentraland/unity-explorer/actions/runs/45213409795) and [FAQ / Photo Link browser check](https://github.com/decentraland/unity-explorer/actions/runs/45213892989).

| Screen | Actions and visual results |
| --- | --- |
| Backpack → Wearables | Opened the Backpack tab, inspected body, hair and upper-body categories, and opened Filter & Sort. The player rendered the fixture’s dressed 3D avatar and equipped-slot thumbnails. Searching for a unique nonexistent item rendered the empty-results message. All sixteen upper-body thumbnails became visible within 30 seconds. |
| Backpack → Wearable Detail | Selected the Baggy Pullover from populated upper-body thumbnails. The right-hand panel displayed its image, name, rarity and description. |
| Backpack → Equip | On build 28113 the blue shirt’s Equip button spun for over 90 seconds while the avatar kept its yellow sweater. On final build 28116 (`7308da1`) the same direct click applied the blue shirt to the avatar and equipped slot by the 60-second checkpoint. The card offered Unequip and the result persisted through the 120-second checkpoint. [Original failure](https://github.com/decentraland/unity-explorer/actions/runs/45222591284) · [Final-player retest](https://github.com/decentraland/unity-explorer/actions/runs/45342967461) |
| Backpack → Saved Outfits | Opened the tab, inspected the empty slots and option to unlock more slots with a name, clicked Save Outfit in the first slot, then reopened it. The saved slot contained the dressed avatar thumbnail. [CI capture](https://github.com/decentraland/unity-explorer/actions/runs/45215896630) |
| Backpack → Emotes | On build 28112 the catalog stayed blank through 60 seconds while the equipped slots showed Clap, Dance and Wave. On final build 28116 the first page showed zero of sixteen images at 30 seconds, three at 60 seconds and all sixteen at 120 seconds. Selecting the first card opened the Clap details and played its animation. All 21 configured base pointers and thumbnail URLs resolved on the gateway. [Final-player retest](https://github.com/decentraland/unity-explorer/actions/runs/45344894989) |
| Emote wheel | Opened the wheel and selected the equipped Clap. The player closed the wheel and played the animation. |
| Settings → Graphics | Opened and scrolled the graphics panel, including the preset, display settings, post-processing and landscape groups. |
| Settings → Sounds | The panel rendered master and individual audio sliders, the microphone selector and background mute. Hardware audio output and recording were outside the CI renderer checks. |
| Settings → Controls | Opened the mouse sensitivity, head sync and pointer settings. Toggled Head Sync, observed the state change, then restored it. |
| Settings → Chat | Opened notification and chat preferences. Toggled Chat Reactions, observed the state change, then restored it. |
| Settings → Lobby | The lobby preference and two selectable background previews rendered. |
| Settings tab strip | The deployment exposed Graphics, Sounds, Controls, Chat and Lobby. General appeared as a group inside Graphics. The client hides a standalone General tab when it has no configured settings. |
| Mouse and Key Controls | Opened the help overlay and reviewed the keyboard and mouse diagrams. |
| Quick Audio | Opened the sidebar popover and inspected the four volume controls. |
| Nearby Voice | Opened the sidebar widget. Its enable toggle, playback volume slider, Speak button and microphone-binding hint rendered. The runner has no physical microphone. [CI capture](https://github.com/decentraland/unity-explorer/actions/runs/45217146241) |
| Minimap → Skybox | Opened the clock below the minimap and inspected its time control and automatic progression setting. [CI capture](https://github.com/decentraland/unity-explorer/actions/runs/45217146241) |
| Minimap → Place menu and collapse | Opened the place menu and inspected Set as Home, Copy Link, and Report Scene. Closing the menu and collapsing the minimap visibly reduced its footprint to the place header. [CI capture](https://github.com/decentraland/unity-explorer/actions/runs/45218450191) |
| HUD → Profile menu and Sign Out | Opened the sidebar profile menu and selected Sign Out on the disposable account. The player returned to the welcome screen with Play as Guest and Log In or Sign Up. [CI capture](https://github.com/decentraland/unity-explorer/actions/runs/45218450191) |
| Camera and controls | Opened Take Shot, opened and closed Camera Controls and used the shutter. The photo appeared in Gallery. |
| Gallery and Photo Detail | Opened the empty gallery, took a photo, reopened the gallery and selected its actual thumbnail. The count advanced from 0/500 to 1/500. Photo Detail rendered the captured scene, place, people and photo actions. |
| Photo Detail → Copy Link | Clicked Copy Link, observed its confirmation and opened the actual clipboard URL in Edge. The browser reported DNS_PROBE_FINISHED_NXDOMAIN for reels.dcl.dev. On November 24, the same photo ID rendered at the deployed dcl.dev/reels route with the correct title and JPEG. The copied hostname still failed DNS. [Unity link issue](https://github.com/decentraland/dcl-one/issues/2070) · [Viewer deployment](https://github.com/decentraland/dcl-one/pull/2034) |
| Shop → Wearables and Emotes | Clicked both Shop banners. The player emitted the matching dcl.dev category links and each returned HTTP 200 with the “Browse Collectibles” Decentraland Shop page. This deployment uses the Shop sidebar and web listings. |
| Buy Credits | Opened the Credits chooser and visually inspected four packs and their prices. The check stopped before payment. |
| System → FAQ | On 7469fb4, the emitted documentation hostname failed DNS. On the official corrected build 28113 (ef5a4ce), the same native click opened the canonical dcl.dev page in Edge with HTTP 200 and a visible Decentraland 101 heading. [Corrected player run](https://github.com/decentraland/unity-explorer/actions/runs/45220913997) |
| System → Contact Support and Discord | Clicked both entries. Contact Support opened the dcl.dev Help Center. Discord followed the gateway redirect to the official Decentraland Discord page. Both completed with HTTP 200. |
| System → Report Bug | Opened the bug report form and inspected its issue-type selector, description, attachment and submission controls. The check stopped before submitting a report. |

**Coverage limits.** The CI player used an ephemeral inventory. Real-money checkout, transfers, paid packs, moderation actions and sharing to third parties were outside the exercised flows. The WARP session establishes screen state and layout, with hardware-specific quality and audio behavior requiring a device session.

### Optional Lobby

In official build 28113, a signed-in player enabled the optional Lobby with the supported `--lobby true` setting, opened it from the Explore header, and saw its populated welcome view. The seasonal stage, dressed 3D avatar, local username, Friends empty state, and starting-place card all rendered. The dark artwork on **interactive-text** included the same red parcel marker and outlines as the PNG returned by the deployment. [Lobby CI run](https://github.com/decentraland/unity-explorer/actions/runs/45222678584) · [Starting-place artwork](https://dcl.dev/api/v2/map.png?height=1024&width=1024&selected=100%2C100&center=100%2C100&size=20)

The pencil opened **Customize Lobby** with the widget picker and occupied grid. **Cancel** returned to the Lobby. Selecting **Places** opened the browse panel, with thumbnails and place names. Widget persistence after saving a rearranged layout was outside this check.

## Selected screenshots

Expand a screen to view the capture. Select the image to open the original PNG.

<details>
<summary>Guest after Jump In</summary>

[![Guest after Jump In](./screenshots/world-entry-adcb29c53a.png)](./screenshots/world-entry-adcb29c53a.png)

</details>

<details>
<summary>Selected avatar and profile form</summary>

[![Selected avatar and profile form](./screenshots/onboarding-profile-a8d23e28e2.png)](./screenshots/onboarding-profile-a8d23e28e2.png)

</details>

<details>
<summary>Backpack and avatar preview</summary>

[![Backpack and avatar preview](./screenshots/personal-backpack-ec3de3d8cc.png)](./screenshots/personal-backpack-ec3de3d8cc.png)

</details>

<details>
<summary>Emote wheel</summary>

[![Emote wheel](./screenshots/personal-emote-wheel-c1fcca6b6c.png)](./screenshots/personal-emote-wheel-c1fcca6b6c.png)

</details>

<details>
<summary>Graphics settings</summary>

[![Graphics settings](./screenshots/personal-settings-graphics-3f26452caf.png)](./screenshots/personal-settings-graphics-3f26452caf.png)

</details>

<details>
<summary>Photo detail</summary>

[![Photo detail](./screenshots/personal-photo-detail-e50828df7d.png)](./screenshots/personal-photo-detail-e50828df7d.png)

</details>

<details>
<summary>Shop and Credits</summary>

[![Shop and Credits](./screenshots/personal-credits-top-up-a441e3018f.png)](./screenshots/personal-credits-top-up-a441e3018f.png)

</details>

<details>
<summary>Minimap place menu</summary>

[![Minimap place menu](./screenshots/place-context-menu-4c3a18ce4b.png)](./screenshots/place-context-menu-4c3a18ce4b.png)

</details>

<details>
<summary>Nearby Voice control</summary>

[![Nearby Voice control](./screenshots/nearby-voice-control-f2c21122cc.png)](./screenshots/nearby-voice-control-f2c21122cc.png)

</details>

<details>
<summary>Nearby chat and emoji picker</summary>

[![Nearby chat and emoji picker](./screenshots/social-chat-emoji-742b156cc0.png)](./screenshots/social-chat-emoji-742b156cc0.png)

</details>

<details>
<summary>Private message received by the second player</summary>

[![Private message received by the second player](./screenshots/pair-message-received-fe6e05f393.png)](./screenshots/pair-message-received-fe6e05f393.png)

</details>

<details>
<summary>Private reply received by the first player</summary>

[![Private reply received by the first player](./screenshots/pair-reply-received-bd21aef3fe.png)](./screenshots/pair-reply-received-bd21aef3fe.png)

</details>

<details>
<summary>Incoming voice call on the second player</summary>

[![Incoming voice call on the second player](./screenshots/pair-call-incoming-d73abfbbec.png)](./screenshots/pair-call-incoming-d73abfbbec.png)

</details>

<details>
<summary>Places detail</summary>

[![Places detail](./screenshots/world-08-place-details-17f5003034.png)](./screenshots/world-08-place-details-17f5003034.png)

</details>

<details>
<summary>macOS login method screen, build 28116</summary>

[![macOS login method screen, build 28116](./screenshots/login-methods-69ca503de9.png)](./screenshots/login-methods-69ca503de9.png)

</details>

<details>
<summary>Signed-in player at the default parcel, build 28116</summary>

[![Signed-in player at the default parcel, build 28116](./screenshots/world-direct-start-30s-74cf7bbbe2.png)](./screenshots/world-direct-start-30s-74cf7bbbe2.png)

</details>

<details>
<summary>Genesis Plaza with the native gateway defaults, build 28116</summary>

[![Genesis Plaza with the native gateway defaults, build 28116](./screenshots/world-direct-genesis-120s-89437549a5.png)](./screenshots/world-direct-genesis-120s-89437549a5.png)

</details>

<details>
<summary>Hardware question opened by Read More, build 28116</summary>

[![Hardware question opened by Read More, build 28116](./screenshots/hardware-heading-crop-14eb31bafe.png)](./screenshots/hardware-heading-crop-14eb31bafe.png)

</details>

<details>
<summary>Shop item opened from the creator’s Passport, build 28116</summary>

[![Shop item opened from the creator’s Passport, build 28116](./screenshots/social-creator-item-emitted-07f9d004fa.png)](./screenshots/social-creator-item-emitted-07f9d004fa.png)

</details>

<details>
<summary>Backpack after equipping the blue shirt, build 28116</summary>

[![Backpack after equipping the blue shirt, build 28116](./screenshots/personal-outfit-equipped-60s-aa19ea3f62.png)](./screenshots/personal-outfit-equipped-60s-aa19ea3f62.png)

</details>

<details>
<summary>Friend’s equipped item thumbnails after 95 seconds, build 28116</summary>

[![Friend’s equipped item thumbnails after 95 seconds, build 28116](./screenshots/peer-items-settled-crop-cbd50621c3.png)](./screenshots/peer-items-settled-crop-cbd50621c3.png)

</details>

<details>
<summary>Backpack showing Clap selected after the emote catalog loaded, build 28116</summary>

[![Backpack showing Clap selected after the emote catalog loaded, build 28116](./screenshots/personal-inventory-emote-selected-74f1fed7ab.png)](./screenshots/personal-inventory-emote-selected-74f1fed7ab.png)

</details>

<details>
<summary>Body Type B and avatar preview</summary>

[![Body Type B and avatar preview](./screenshots/onboarding-second-body-04523ad96a.png)](./screenshots/onboarding-second-body-04523ad96a.png)

</details>

<details>
<summary>Optional Lobby with the avatar, place card and friends panel</summary>

[![Optional Lobby with the avatar, place card and friends panel](./screenshots/lobby-settled-caaae0c70b.png)](./screenshots/lobby-settled-caaae0c70b.png)

</details>

<details>
<summary>Lobby layout editor</summary>

[![Lobby layout editor](./screenshots/lobby-edit-layout-511024d9d4.png)](./screenshots/lobby-edit-layout-511024d9d4.png)

</details>

<details>
<summary>Places opened from the Lobby</summary>

[![Places opened from the Lobby](./screenshots/lobby-places-a5c400c41d.png)](./screenshots/lobby-places-a5c400c41d.png)

</details>

<details>
<summary>Corrected sidebar FAQ destination</summary>

[![Corrected sidebar FAQ destination](./screenshots/personal-system-faq-success-browser-3ced2fd52c.png)](./screenshots/personal-system-faq-success-browser-3ced2fd52c.png)

</details>

<details>
<summary>Creator’s Passport with published wearables and emotes</summary>

[![Creator’s Passport with published wearables and emotes](./screenshots/social-creator-passport-creations-settled-451dc7c100.png)](./screenshots/social-creator-passport-creations-settled-451dc7c100.png)

</details>
