> >-

# Vibe Coding with AI

Build Decentraland scenes by describing what you want. An AI assistant handles the SDK7 code, ECS architecture, and project structure for you.

Whether you're a first-time creator or a seasoned developer, AI-assisted "vibe coding" lets you go from an idea to a running scene in minutes instead of hours.


> [!NOTE]
> **💡 Tip**: You don't need to know TypeScript to get started. AI assistants can generate working scene code from plain language descriptions.


## What is Vibe Coding?

Vibe coding means building scenes by having a conversation with an AI assistant rather than writing every line of code by hand. You describe what you want — "a medieval tavern with clickable doors and background music" — and the AI writes correct, deployable SDK7 code.

This approach works at any skill level:

- **Beginners & non-developers** — Go from zero to a working scene without writing code manually.
- **Experienced developers** — Skip the boilerplate. Let the AI handle multiplayer sync, UI scaffolding, and deployment config while you focus on creative decisions.
- **Teams & studios** — Prototype scene concepts quickly before committing full development resources.

## Use the AI Assistant in the Creator Hub

The quickest way to start is the Creator Hub's built-in [AI Assistant](/docs/creator/scene-editor/code/ai-assistant/). It sees your open scene, edits entities, components, smart items and code for you, and can run the preview to check its own work. It installs the skills below into your project automatically, so there is nothing to set up beyond signing into a coding CLI.

It is an experimental feature, turned off by default.

## Combine a code editor with AI

Use a general-purpose AI code editor like [Cursor](https://www.cursor.com/) or VS Code with GitHub Copilot or Claude AI. Decentraland provides a context folder so these tools understand the SDK.

1. Open the Creator Hub and create or open a scene.
2. Click the **< > CODE** button to open your code editor.
3. Use the editor's built-in AI assistant (Cursor's chat, Copilot, etc.) to generate or modify code.

## Install Skills for Any AI Agent

Skills are ready-made instruction sets that teach your AI agent how to work with the Decentraland SDK. Each skill covers a specific topic, like creating scenes, adding 3D models, or setting up multiplayer, so the AI already knows the right patterns, APIs, and constraints without you having to explain them. Installing skills means fewer mistakes and better results from the very first prompt.

```bash
# Choose which Decentraland skills to install from an interactive picker
npx skills add decentraland/sdk-skills

# Or install all Decentraland skills
npx skills add decentraland/sdk-skills --all

# Or pick specific skills
npx skills add decentraland/sdk-skills --skill create-scene

# Install globally (available in all projects)
npx skills add decentraland/sdk-skills -g
```

This copies skill files into your agent's configuration so it knows Decentraland patterns and constraints.

## Updating Skills

New skills are added over time, and existing ones are improved. To get the latest versions, re-run the install command with `--all`:

```bash
# Update every installed skill and download any new ones
npx skills add decentraland/sdk-skills --all
```

Running `add` again re-fetches the repository, so it refreshes the skills you already have and installs any that were added since your first install. If you installed skills globally, add `-g` to this command too.


> [!WARNING]
> **📔 Note**: Don't use `npx skills update` for this. It only refreshes the skills already on your machine, so any skills added to the repository after your last install are silently skipped. Always use `npx skills add decentraland/sdk-skills --all` instead.


## Available AI Skills

When you install skills into your agent, the following capabilities are available:

| Skill                  | What it does                                                        |
| ---------------------- | ------------------------------------------------------------------- |
| `sdk-scenes`           | Entry point with agent guidelines and index of all topic skills     |
| `create-scene`         | Scaffold a new SDK7 scene project from scratch                      |
| `migrate-sdk6-to-sdk7` | Port a legacy SDK6 scene to SDK7                                    |
| `add-3d-models`        | Add 3D models (`.glb`/`.gltf`) with positioning, scaling, colliders |
| `add-interactivity`    | Pointer events, triggers, raycasts                                  |
| `build-ui`             | 2D screen-space UI with React-ECS — HUDs, menus, dialogs            |
| `animations-tweens`    | GLTF model animations with Animator, SDK tweens                     |
| `multiplayer-sync`     | Peer-to-peer multiplayer using CRDT networking                      |
| `authoritative-server` | Headless Multiplayer Server for server-validated scenes (BETA)      |
| `audio-video`          | Sound effects, music, audio streaming, and video players            |
| `audio-analysis`       | Real-time amplitude and frequency data for audio-reactive scenes    |
| `deploy-scene`         | Deploy scenes to Genesis City (LAND-based)                          |
| `deploy-worlds`        | Deploy scenes to Worlds (personal 3D spaces)                        |
| `optimize-scene`       | Performance optimization, scene limits, best practices              |
| `camera-control`       | Camera mode detection, cinematic camera, virtual cameras            |
| `composites`           | Composite file format reference for static scene content            |
| `lighting-environment` | Dynamic lighting, shadows, skybox, fog, environment settings        |
| `particle-system`      | Particle effects — fire, smoke, sparks, snow, fireworks             |
| `npcs`                 | Non-player characters — NPC Toolkit library and manual approaches   |
| `player-avatar`        | Player position, profile, avatar customization, attachments         |
| `player-physics`       | Physics forces — impulses, knockback, continuous forces             |
| `nft-blockchain`       | NFT display and blockchain/crypto interactions                      |
| `advanced-rendering`   | Billboard, TextShape, PBR materials, video materials                |
| `advanced-input`       | System-level input polling and player movement control              |
| `scene-runtime`        | Cross-cutting runtime APIs — async work, HTTP, messaging            |
| `script-components`    | Script component classes for the Creator Hub                        |
| `game-design`          | Game design patterns, scene limits, performance budgets             |
| `unity-explorer-mcp`   | Drive a running Explorer to test and verify a scene in-world        |

Note: Some of these skills involve fetching 3D models or other assets from free asset catalogs. The AI agent should always get confirmation from the user before downloading any new assets to a scene project.

## Let the AI see your scene in-world

Normally the AI writes code and _you_ run the preview, look at it, and report back what's wrong. The Decentraland desktop client can close that loop: it ships with an optional **MCP server** that lets an AI agent look at and control the running Explorer directly. The agent takes its own screenshots, reads the scene's console output, walks the player around, clicks on objects, and checks whether the scene actually does what it was asked to build — then fixes what it finds and looks again.

This turns vibe coding from "describe, wait, review" into a loop the agent can run mostly on its own.


> [!NOTE]
> **💡 Tip**: Install the `unity-explorer-mcp` skill before trying this. It teaches the agent the whole workflow — how to launch the client, how to frame useful screenshots, how to cross-check what it sees against the scene's actual state, and how to recover when the scene stops loading.
> 
> ```bash
> npx skills add decentraland/sdk-skills --skill unity-explorer-mcp
> ```
> 


### What you need

- The **Decentraland desktop client** installed (the same one the Creator Hub launches for previews).
- An AI agent that can connect to MCP servers over HTTP — Claude Code, Cursor, Cline, VS Code with an MCP-capable extension, and others.
- An up-to-date SDK in your scene: run `npm i @dcl/sdk@latest` if the `--mcp` flag below is rejected as an unknown option.

### 1. Launch the scene with the MCP server enabled

From your scene folder:

```bash
npm run start -- --mcp
```

This does what `npm run start` always does — serves your scene at `http://127.0.0.1:8000` and hot-reloads it whenever you save a file — and additionally launches the desktop client with the MCP server listening on `http://127.0.0.1:8123/unity-explorer-mcp`.

Log in when the client opens. The agent can only start working once you're through the login screen and the world has loaded.

### 2. Connect your AI agent to the server

In **Claude Code**, register it once:

```bash
claude mcp add --transport http --scope user explorer http://127.0.0.1:8123/unity-explorer-mcp
```

In **any other MCP client**, add the server the way that client documents, using these details:

| Setting   | Value                                            |
| --------- | ------------------------------------------------ |
| Transport | Streamable HTTP (not stdio — there's no command to run, the server lives inside the running client) |
| URL       | `http://127.0.0.1:8123/unity-explorer-mcp`        |
| Auth      | None                                              |
| Name      | `explorer`                                        |

Many clients use a JSON config file for this (`.cursor/mcp.json`, `mcp.json`, and similar — check your client's docs for the exact key names):

```json
{
  "mcpServers": {
    "explorer": {
      "type": "http",
      "url": "http://127.0.0.1:8123/unity-explorer-mcp"
    }
  }
}
```

Restart or reload your AI client after registering the server, so it picks up the connection. If the agent says the Explorer tools aren't available, the usual cause is that the client wasn't running when the agent started — reconnect the server (in Claude Code, run `/mcp`) with the Explorer open.


> [!NOTE]
> **Is this safe?** The server only runs while you launch the client with `--mcp`, only accepts connections from your own machine (`127.0.0.1`), and rejects requests coming from web pages. Nothing is exposed to the internet, and it's completely off in a normal client launch.


### 3. Ask for what you want, and let it verify

With the server connected, ask for work the way you normally would — the difference is that the agent can now check its own results:

> "Add a treasure chest at the center of the parcel that opens when clicked, then walk over and click it to confirm the lid animates."

> "The neon sign looks too dim. Take a screenshot, adjust the emissive intensity, and show me a before/after."

> "Something's wrong with the elevator. Walk onto the platform, watch the logs, and tell me why it doesn't move."

Behind the scenes the agent can:

- **See** — take screenshots, read the scene's `console.log` output and errors, check whether the scene loaded or crashed, list the scene's entities and inspect their components, and read the player and camera position.
- **Control** — move and teleport the player, walk in a direction through real collisions, aim the camera, place a free camera for a specific shot, switch camera modes, click on scene objects, send chat messages and `/commands`, trigger emotes, and reload the scene.
- **Measure** — read live counts of the scene's content (triangles, entities, textures, materials, and more) against Decentraland's limits, rank which 3D models contribute the most to that load from any viewpoint, and sample the real frame rate the player experiences at any spot.

### Use it to optimize your scene

Because the agent can measure as well as see, you can hand it performance work directly. It can check your scene against the [scene limitations](/docs/creator/sdk7/optimizing/scene-limitations/), identify which 3D models are the heaviest, measure the frame rate at specific spots, and, most importantly, re-measure after making changes to prove that they helped:

> "Check if my scene is within Decentraland's content limits, and tell me which budget is closest to the cap."

> "The scene feels choppy near the fountain. Stand there, measure the frame rate, and tell me which models cost the most from that spot."

> "Optimize the scene: find what's dragging the frame rate down, fix it, and show me before and after measurements."

For best results, install the `optimize-scene` skill together with `unity-explorer-mcp`, so the agent also knows Decentraland's optimization best practices. See [Performance Optimization](/docs/creator/sdk7/optimizing/performance-optimization/#optimize-with-ai) for more on this workflow, including how to pair it with the Blender MCP server to automate fixes to the 3D models themselves.

### Tips

- **Ask for proof, not claims.** "Verify with a screenshot" or "confirm from the logs" is what makes this workflow pay off. A good agent cross-checks both: pixels can look right while the underlying state is broken, and the reverse.
- **Screenshots cost tokens.** Each screenshot the agent looks at consumes part of its context. If you want a long visual sweep — an animation over time, a walk-through of many spots — ask it to capture frames to files and only read the ones that matter. The `unity-explorer-mcp` skill ships a script that does exactly this.
- **Save once, not five times in a row.** Rapid successive saves can make the client load a half-written bundle and drop the scene entirely, which needs a client restart to recover. Ask the agent to batch its edits into a single save.
- **Keep the client open.** If you close it, the connection dies and the agent loses its eyes. Relaunching with the same command brings it back.
- **Teleports behave differently in local scene development.** Moving between parcels with `/goto` is disallowed there, so the agent should reposition the player within the scene instead of teleporting.

## Tips for Effective Prompting

Getting the best results from AI is about giving clear, specific prompts. Here are some tips:

### Be specific about what you want

Instead of:

> "Make my scene better"

Try:

> "Add a door at position (8, 0, 8) that opens with a rotation animation when clicked, and plays a creak sound effect"

### Reference existing items

> "Make the red button on the table trigger the elevator to go up"

### Ask for one thing at a time

Break complex requests into steps:

1. "Add a scoreboard UI in the top-right corner"
2. "Add a counter that increases when the player clicks the target"
3. "Display the counter value on the scoreboard"

### Iterate and refine

After each change:

1. Preview the scene (click **Preview** in Creator Hub, or `npm run start` in the command line)
2. Check what works and what doesn't
3. Tell the AI what to adjust: "Move the NPC 2 meters to the left and make it face the player"

### Example prompts

To be ran on a fresh clone of [sdk7-scene-template](https://github.com/decentraland/sdk7-scene-template/).

#### Prompt example 1

```
I want you to scrap the current scene code and make a small simple labyrinth game, in 1 parcel, the walls can be cubes, and they should have collision.

The game should start when the player actually enters the labyrinth on point A and it finishes when he exits on point B (only  exit of the labyrinth).

I want you to add 3 different doors in the maze, that have to be interacted with the pointed input to open them and pass.

You should verify that the player can actually play and win
```


#### Prompt example 2
```
I want you to scrap the current scene code and build a simple platformer game.

You have to verify that the game can be won before considering it finished.
```

## What AI Can Help With

- Scaffolding new scenes from a description
- Adding and positioning 3D models
- Writing click handlers and interactivity
- Building UI (HUDs, menus, dialogs)
- Setting up multiplayer sync
- Configuring the Multiplayer Server for anti-cheat
- Adding audio, video, and streaming
- Creating animations and tweens
- Optimizing scene performance
- Preparing scenes for deployment
- Debugging issues in existing code
- Testing and visually verifying a scene in a running Explorer (see [Let the AI see your scene in-world](/docs/creator/sdk7/getting-started/vibe-coding/#let-the-ai-see-your-scene-in-world))

## Limitations

While AI tools are powerful, keep these in mind:

- **Always preview** — AI-generated code may not look exactly how you expect. Run a preview to verify.
- **Scene limits still apply** — AI cannot bypass Decentraland's [scene limitations](/docs/creator/sdk7/optimizing/scene-limitations/) (triangle counts, file sizes, parcel boundaries).
- **Complex game logic** — For intricate game mechanics, you may need to guide the AI step by step or refine its output manually.
- **Custom 3D models** — AI can reference existing free assets or load models you provide, but it cannot create 3D models from scratch (unless you use other tools like Blender official MCP server at the same time).

## Next Steps

- [SDK Quick Start](/docs/creator/sdk7/getting-started/sdk-101/) — Learn SDK7 fundamentals
- [Combine with Code](/docs/creator/scene-editor/code/overview/) — Mix visual editing with code
- [Multiplayer Server](/docs/creator/sdk7/networking/authoritative-servers/) — Server-authoritative multiplayer
- [Scene Examples](https://studios.decentraland.org/resources?sdk_version=SDK7) — Browse example scenes for inspiration
- [Useful Resources](/docs/creator/sdk7/getting-started/useful-resources/) — More AI tools, asset libraries, and add-ons to speed up your workflow
