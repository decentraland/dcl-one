> >-

# Development Workflow

This document outlines the steps recommended for developing a scene for Decentraland, from ideation to publishing and beyond.

## How do I create a Decentraland scene?

1. **Install the [Creator Hub](https://decentraland.org/download/creator-hub)**, the official desktop app for creating, previewing, and publishing scenes. It's the recommended tool for creators of all skill levels.
2. **If you use an AI coding assistant** (like Claude Code, Cursor, or Copilot), install the official [Decentraland SDK Skills](/docs/creator/sdk7/getting-started/vibe-coding/) so it knows verified SDK7 patterns: `npx skills add decentraland/sdk-skills`
3. **The [CLI](/docs/creator/sdk7/getting-started/using-the-cli/)** is an alternative for advanced users and automated workflows.

The sections below expand on each stage of that workflow.

## Install the Creator Hub

Make sure you have the Decentraland Creator Hub installed.

* [Installation Guide](/docs/creator/scene-editor/get-started/editor-installation/)

If you intend to work with code, also make sure you install [Visual Studio Code](https://code.visualstudio.com/) or [Cursor AI](https://www.cursor.com/).


> [!NOTE]
> **💡 Tip**: You can also use AI assistants to generate scene code from plain language descriptions. See [Vibe Coding with AI](/docs/creator/sdk7/getting-started/vibe-coding/) for how to get started with AI-assisted development.


## Design your experience

Think about how much space you need to take up, what kind of distribution, what kinds of mechanics you want players to be able to carry out, etc. The following documents can serve as a guide:

* [UX & UI Guide](/docs/creator/sdk7/design-experience/ux-ui-guide/)
* [Design constraints for games](/docs/creator/sdk7/design-experience/design-games/)
* [Scene MVP guidelines](/docs/creator/sdk7/design-experience/mvp-guidelines/)

## Where to publish

In Decentraland, content is published to adjacent plots of land in a finite amount of space. Players can freely walk from one to the other. Each scene is its own contained little world, items from one scene can't extend out into another scene, and the code for each scene is sandboxed from all others.

Permission to publish to each of these is controlled via tokens. You don't need land to develop a scene, but you will need access to land once you're ready to publish.

Alternatively, you have the option to publish to Decentraland [Worlds](/docs/creator/sdk7/publishing/publishing-options/#decentraland-worlds), which are self-contained and isolated scenes.

The following options are available:

* Rent LAND
* Purchase LAND
* Obtain permissions from a land owner
* Publish to a Decentraland World, see [worlds](/docs/creator/sdk7/publishing/publishing-options/#decentraland-worlds) to learn more.

See [Publishing options](/docs/creator/sdk7/publishing/publishing-options/) for more details.

## Templates and examples

When creating a new scene, choose amongst several base template scenes that include some basic code and 3d models. Use these to get started faster.

* [Example scenes](https://studios.decentraland.org/resources?sdk_version=SDK7): here you can find a large collection of example scenes, each showcasing different mechanics that you can borrow. You can also clone any of these scenes and use it as a starting point.
* [Helper libraries](https://studios.decentraland.org/resources?sdk_version=SDK7\&resource_type=Library): these can simplify many common tasks.

## Art assets

If you're an experienced artist or you have access to someone who is, you can create custom `.gltf` or `.glb` models for your scene. See [3D model essentials](/docs/creator/3d-modeling/3d-models/) for tips on how to create 3D models for Decentraland.

You don't need to create your own assets, though. See [Useful Resources](/docs/creator/sdk7/getting-started/useful-resources/) for asset libraries and generative AI tools you can use to source 3D models, as well as other tools that can speed up your workflow.

## Run a local preview

To run a preview of your scene, open it in the Creator Hub and click the **Preview** button. Alternatively, if you're working from the command line, run `npm run start` on your project's root folder.

* [Preview your scene](/docs/creator/sdk7/getting-started/preview-scene/) for more details.
* Check the [Debug a scene](/docs/creator/sdk7/getting-started/preview-scene/#debug-a-scene) for tips on how to debug any issues.
* Enable **Optimize Assets** in the preview settings (or run `npm run start -- --local-ab` from the CLI) to load your 3D models as optimized asset bundles. The preview runs smoother and shows the models just as they'll look once published. See [Preview with optimized assets](/docs/creator/sdk7/getting-started/preview-scene/#preview-with-optimized-assets).


> [!NOTE]
> **💡 Tip**: When using the Creator Hub, every time you make a change on your scene, the preview is automatically updated. Even while running.


## Publish to Decentraland

Once you're happy with your scene, it's time to publish to Decentraland. For this, you need to own LAND, a Decentraland NAME, or an ETH ENS name, or have permissions given by someone that does.

See [publishing](/docs/creator/sdk7/publishing/publishing/) for instructions on how to do that.

Alternatively, you can publish to [Worlds](/docs/creator/sdk7/publishing/publishing-options/#decentraland-worlds), a personal 3D space that doesn't require LAND.

## Promote

Now that your scene is out there, spread the voice! Here are a few ways to do that:

* Share it on social media (#DCL)
* Announce it on [Discord](https://dcl.gg/discord)
* Submit it to be featured on [events.decentraland.org](https://events.decentraland.org/)
* Organize an event in your scene
* Add a spawn point on a high-traffic area that links to your scene

## Iterate

Once your scene has been live for a while and you've gotten feedback from players, you're in a great position to iterate on it!

Update your content with improvements and new features, deploying new versions of your scene to the same coordinates.

## Giving back

If you create a scene, game, or application that you're proud of, consider making it open source! That way others can learn from your code and build on your work. You can also share the whole project in [Awesome Repository](https://github.com/decentraland-scenes/Awesome-Repository).

If you build a reusable piece of functionality, you may want to make it into a library that others can import into their projects.
