> Tools, add-ons, asset libraries, and example projects that speed up your Decentraland creation workflow.

# Useful Resources

This page collects tools, add-ons, asset libraries, and example projects that can speed up your workflow when creating experiences for Decentraland.

## Decentraland SDK Skills

Give your AI coding assistant Decentraland-specific guidance to quickly create, iterate, and deploy your experiences. Decentraland SDK Skills work with tools like Claude Code, Cursor, Codex, Cline, and Windsurf, and are kept up to date with the latest SDK changes.

🔗 [github.com/decentraland/sdk-skills](https://github.com/decentraland/sdk-skills)

See [Vibe Coding with AI](/docs/creator/sdk7/getting-started/vibe-coding/) for tips on using SDK Skills in your AI workflow.

## Decentraland SDK7 Test Scenes

Use the Decentraland SDK7 Test Scenes to see working examples of specific SDK features. The repo includes many isolated test scenes, and the live test world lets you try them out before using similar systems in your own experiences.

🔗 [github.com/decentraland/sdk7-test-scenes](https://github.com/decentraland/sdk7-test-scenes/)

## Multiplayer Server Leaderboard

See how Decentraland's Multiplayer server can be used to build a resilient leaderboard. The server owns the game state, validates each score, persists per-player totals, and broadcasts a ranked leaderboard that every Decentraland client can render.

🔗 [Multiplayer server leaderboard test scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/90,-9-authoritative-server-leaderboard)

See [Authoritative Servers](/docs/creator/sdk7/networking/authoritative-servers/) to learn how to build on the authoritative server.

## Decentraland Tools for Blender

Speed up your workflow with this Blender add-on, which handles parcel grids, wearable limits, texture optimization, LOD generation, and GLB export with Decentraland settings built in.

🔧 [extensions.blender.org/add-ons/decentraland-tools](https://extensions.blender.org/add-ons/decentraland-tools/)

## 3D Art Assets

A good experience will have great 3D art to go with it. If you're keen on creating those 3D models yourself, see the [3D Modeling section](/docs/creator/3d-modeling/3d-models/). But if you prefer to focus on the coding or game design side of things, you don't need to create your own assets!

Here are a few sources for 3D models you can use in a Decentraland scene:

- [IWB Catalog](https://dcl-iwb.co/)
- [Asset Ovi](https://assetovi.com/)
- [SketchFab](https://sketchfab.com/)
- [Clara.io](https://clara.io/)
- [Archive3D](https://archive3d.net/)
- [SketchUp 3D Warehouse](https://3dwarehouse.sketchup.com/)
- [Thingiverse](https://www.thingiverse.com/)
- [ShareCG](https://www.sharecg.com/)
- [CGTrader](https://cgtrader.com)

You can also use Generative AI tools to generate your own 3D models. Check out:

- [Blender MCP](https://github.com/ahujasid/blender-mcp) + [Claude Desktop](https://claude.ai/download): connect an AI assistant directly to [Blender](https://www.blender.org/), so you can create and edit 3D models by describing what you want. All free tools.
- [Meshy](https://www.meshy.ai/)
- [Luma AI](https://lumalabs.ai/genie)
- [Tripo3D](https://www.tripo3d.ai/app)
- [Rodin](https://hyper3d.ai/rodin)


> [!WARNING]
> **📔 Note**: Models must be in the supported `.gltf` or `.glb` formats, and must have a number of triangles, textures and materials that adhere to the [scene limitations](/docs/creator/sdk7/optimizing/scene-limitations/). If getting models from a third party site, pay attention to the license restrictions of the content you download.


## Blender MCP Server

Use Blender MCP to connect an AI agent, such as Claude Code, to Blender. It can help create and edit GLTF/GLB models, materials, and textures from simple text prompts.

🔗 [blender.org/lab/mcp-server](https://www.blender.org/lab/mcp-server/)


> [!WARNING]
> **📔 Note**: Blender recommends using MCP in a safe environment without sensitive files.


## Open Source 3D Assets

A curated collection of high-quality 3D assets including props, environments, and structures, completely free and open source. Built on GLB and glTF standards.

🔗 [opensource3dassets.com](https://www.opensource3dassets.com/en)

## Genesis Plaza Assets

Reuse any of Genesis Plaza's assets or textures in your own experiences. You can download the .blend file, or use Stom's Blender Asset Library to drag and drop from a repository. You can also download matching procedural landscaping assets such as trees and bushes.

- 📂 [Download the .blend file](https://drive.google.com/file/d/1beZHX1D8zR9iMCEmcPQU6etxpZXohZ5P/view?usp=sharing)
- 🐈‍⬛ Find Stom's repository at [github.com/stom66/dcl-genesis-plaza-assets](https://github.com/stom66/dcl-genesis-plaza-assets)
- ℹ️ [Watch a video on how to use the repo](https://files.catbox.moe/eo5hhb.mp4)
- 🌺 Match Decentraland's procedural assets [with these trees, bushes, and rocks](https://drive.google.com/drive/folders/1Hxoog-ov_igbNTU3H7EwOX634EVCbhYQ?usp=drive_link)

## Decentraland Scene Optimizer

Try this useful tool to extract, deduplicate, and compress textures from GLB files to further optimize your scenes.

🔗 [github.com/decentraland/SceneOptimizer](https://github.com/decentraland/SceneOptimizer)
