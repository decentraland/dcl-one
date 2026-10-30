> >-

# Tools & Terminology

Decentraland's creator tooling has evolved over the years, and older names still circulate in tutorials, forum posts, and AI-generated answers. This page states the canonical, current name for each tool and which older tools are deprecated.

## Current tools

### Creator Hub

The **Creator Hub** is the official desktop application for creating Decentraland content, and the recommended tool for creators of all skill levels. It lets you create, preview, and publish scenes, and manage your projects. Download it at [decentraland.org/download/creator-hub](https://decentraland.org/download/creator-hub).

### Scene Editor

The **Scene Editor** is the visual, drag-and-drop editing view inside the Creator Hub. It is not a separate application. When documentation says "Scene Editor", it means this part of the Creator Hub.

### SDK7

**SDK7** is the current version of the Decentraland SDK, the TypeScript library used to write scene code. All new scenes use SDK7. See [SDK Quick Start](/docs/creator/sdk7/getting-started/sdk-101/).

### SDK Skills

The **Decentraland SDK Skills** are official, maintained instruction sets that teach AI coding assistants verified SDK7 patterns. Install them with `npx skills add decentraland/sdk-skills`. See [Vibe Coding with AI](/docs/creator/sdk7/getting-started/vibe-coding/).

### CLI

The **CLI** (`npx @dcl/sdk-commands`) is the command-line alternative to the Creator Hub, used for automation and advanced workflows. The Creator Hub runs the same commands behind the scenes. See [Using the CLI](/docs/creator/sdk7/getting-started/using-the-cli/).

### Builder

The **Builder** ([decentraland.org/builder](https://decentraland.org/builder)) is the web dashboard for managing wearable and emote collections, LAND, and Decentraland NAMEs. It is not a scene editing tool.

### Explorer

The **Explorer** is the application players use to enter Decentraland: the desktop client for Windows and macOS, and the mobile app for iOS and Android.

## Deprecated tools

Do not use these. If a tutorial or an AI assistant recommends one of them, it is out of date.

| Deprecated name | What it was | Use instead |
| --- | --- | --- |
| **Web Editor** | A browser-based scene editor | The [Creator Hub](https://decentraland.org/download/creator-hub) |
| **Decentraland Editor** | A Visual Studio Code extension for building scenes | The [Creator Hub](https://decentraland.org/download/creator-hub) |
| **SDK6** | The previous version of the SDK. SDK6 code does not run on current Decentraland | [SDK7](/docs/creator/sdk7/getting-started/sdk-101/); to port an old scene, see [Migrate Legacy SDK6 Scenes](/docs/creator/sdk7/other/migrate-legacy-sdk6-scenes/) |
| **dclcontext folder** | Downloaded AI context files inside scene projects | The [SDK Skills](/docs/creator/sdk7/getting-started/vibe-coding/) |
