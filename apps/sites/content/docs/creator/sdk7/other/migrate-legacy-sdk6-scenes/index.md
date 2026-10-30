> Migrate a legacy SDK6 scene to SDK7 using an AI assistant.

# Migrate Legacy SDK6 Scenes

If you have an old SDK6 scene that you'd like to bring up to date, you can migrate it to SDK7 automatically with the help of an AI assistant. The [Decentraland SDK Skills](/docs/creator/sdk7/getting-started/vibe-coding/#install-skills-for-any-ai-agent) include a dedicated migration workflow that the AI follows step by step, so you don't have to translate code by hand.

This guide walks you through the full process.

## 1. Create a new empty SDK7 scene

Start with a fresh, empty SDK7 project. The AI will move your legacy code and assets into this clean structure.

**Option A — Creator Hub**

1. Open the Creator Hub.
2. Select the **Scenes** tab and click **New Scene**.
3. Pick the **Empty Scene** template.

**Option B — Command line**

Run the following on an empty folder:

```bash
npx @dcl/sdk-commands init
```

This populates the folder with the default files of an SDK7 project.

## 2. Install the SDK Skills

The SDK Skills teach your AI agent how to work with the Decentraland SDK7, including how to perform a proper SDK6 → SDK7 migration.

You can install them in two ways.

**Option A — Ask the AI to install them**

Open your AI assistant in the scene folder and simply say:

> Install the Decentraland SDK skills.

The AI knows the command and will install them for you.

**Option B — Install from the command line**

```bash
npx skills add decentraland/sdk-skills --all
```

The `--all` flag installs all of the available skills. If you leave it out, an interactive picker opens for you to choose which skills to install.

For more details on what skills are available and how they work, see [Vibe Coding with AI](/docs/creator/sdk7/getting-started/vibe-coding/#install-skills-for-any-ai-agent).

## 3. Ask the AI to migrate the scene

Place your legacy SDK6 project files somewhere the AI can read them — for example in a separate folder on your machine, or in a public GitHub repo.

Then prompt the AI:

> Migrate this SDK6 scene to SDK7.

The skills include the full migration playbook, so the AI will:

- Identify the entry point and components used in the SDK6 scene.
- Translate entities, components, and systems to their SDK7 equivalents.
- Move 3D models, audio, and other assets into the new project.
- Update `scene.json` and project configuration.
- Replace deprecated APIs with their current SDK7 counterparts.

## 4. Test and iterate

Once the migration pass finishes, preview the scene and play through it.

- In the Creator Hub, click **Preview**.
- From the command line, run `npm run start`.

As you test, look for:

- Visual issues — missing models, wrong positions or scales, broken materials.
- Interaction issues — clicks that don't fire, triggers that don't activate, dialogs that don't appear.
- Multiplayer or networking issues, if your scene used them.
- Console errors in the preview window.

When you find a bug, describe it to the AI in plain language:

> The red button on the table doesn't open the door anymore. It used to play a sound and rotate the door 90 degrees.

The AI can use the surrounding skills (interactivity, animations, audio, etc.) to fix issues one by one. Iterate until the scene behaves like the original.


> [!NOTE]
> **💡 Tip**: Fix one bug at a time and preview after each change. It's much easier to catch regressions when changes are small.


## Next Steps

- [Vibe Coding with AI](/docs/creator/sdk7/getting-started/vibe-coding/) — Full reference for working with AI assistants and SDK Skills.
- [SDK Quick Start](/docs/creator/sdk7/getting-started/sdk-101/) — Learn SDK7 fundamentals so you can review and refine the migrated code.
