> Scene content driven by a Google Sheet, updated without redeploying.

# Data from a spreadsheet

Drive scene content from a Google Sheet. Edit the sheet to change the artist name, video stream, and featured wearable without redeploying the scene.

<!-- example-context:start -->
## Project information

**Maintained by:** Decentraland

[View source](https://github.com/decentraland/Read_Spreadsheet_Scene)

**License:** [Apache-2.0](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/main/LICENSE). Check upstream terms before reuse; code and assets may have different terms.
<!-- example-context:end -->

<!-- example-highlights:start -->
## Explore the code

These links point to a specific source snapshot so the explanations remain useful as upstream development continues. They are learning references, not a security assessment.

### Read a published Google Sheet as JSON

`getSpreadsheetData` requests the sheet through the Google Visualization endpoint (`gviz/tq?tqx=out:json`), strips the `setResponse(...)` wrapper, and turns each row into an object keyed by the column headers.

**Project context:** The sheet must be shared so anyone with the link can view it. `gid` selects the tab and `headers=1` treats the first row as column names.

**Source references:**

- [`getSpreadsheetData`](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/57bb4086aed3054d69a74f33f20f2d86c4eb9bb6/src/scheduler.ts#L1-L47)

### Apply a row to the scene

`updateFromSpreadsheet` reads one row and updates a banner `TextShape`, replaces the screen's `VideoPlayer` source, swaps the wearable shown on the dispenser, and points the dispenser's click action at the row's purchase link.

**Project context:** Column names such as `NowPlaying`, `VideoURL`, `WearableURN`, and `BuyURL` must match the sheet headers. Entities are looked up by the names given in the Creator Hub scene.

**Source references:**

- [`updateFromSpreadsheet`](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/57bb4086aed3054d69a74f33f20f2d86c4eb9bb6/src/index.ts#L10-L64)
- [`createWearable`](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/57bb4086aed3054d69a74f33f20f2d86c4eb9bb6/src/index.ts#L100-L130)

### Cycle through rows from a Creator Hub button

`main` listens for the `Next` and `Previous` actions on the `Switch Artist` entity and wraps the row index around the downloaded data.

**Project context:** Uses `getActionEvents` from `@dcl/asset-packs`, so the actions are configured on the entity in the Creator Hub.

**Source references:**

- [`main`](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/57bb4086aed3054d69a74f33f20f2d86c4eb9bb6/src/index.ts#L69-L97)

<!-- example-highlights:end -->
