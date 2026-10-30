> >-

# Using the CLI


> [!NOTE]
> **💡 Tip**: If you're working with an AI coding assistant (like Claude Code, Cursor, or Copilot), install the official Decentraland SDK Skills before it writes any code, so it knows verified SDK7 patterns:
> 
> ```bash
> npx skills add decentraland/sdk-skills
> ```
> 
> See [Vibe Coding with AI](/docs/creator/sdk7/getting-started/vibe-coding/) for the full AI-assisted workflow.


To build scenes for Decentraland you can either use:

* The [Creator Hub](/docs/creator/scene-editor/get-started/editor-installation/)
* The Command Line Interface (CLI)

Both tools allow you to compile and preview your scene in an "off-chain" development environment. After testing your scene locally, you can upload your content to the content server, linking it with your LAND or WORLD.

Although the Scene Editor in the Creator Hub is easier to use, the CLI allows you more flexibility, and can be easily used in automated processes.


> [!WARNING]
> **📔 Note**: The Scene Editor runs the same command-line operations behind the curtains.



> [!NOTE]
> **💡 Tip**: See [Installation guide](/docs/creator/scene-editor/get-started/editor-installation/) for instructions on how to install the Creator Hub.


## Before you Begin

To deal with the scene via the command line, please install the following dependencies before you run CLI commands with the scene:

* [Node.js](https://nodejs.org) (version 20 or later)

## Initiate a new project

Run `npx @dcl/sdk-commands init` on an empty folder to populate it with the default files of a Decentraland [scene](/docs/creator/sdk7/projects/scene-metadata/) project. You can also create a new scene with the **New Scene** button in the Creator Hub, which produces the same project structure.

To start from a different kind of project, use the `--project` flag. For example, to create a [smart wearable](/docs/creator/sdk7/projects/smart-wearables/) project:

```bash
npx @dcl/sdk-commands init --project smart-wearable
```

The available options for `--project` are `scene-template` (the default), `px-template`, `smart-wearable`, and `library`.

## Update the SDK version of a scene

Run the following command on the scene folder:

```bash
npm i @dcl/sdk@latest
```

You can confirm that it worked by checking the `package.json` file for the scene, and looking for the `@dcl/sdk` version there.

## Run a preview

Run `npm run start` on the root level of a scene, workspace, or smart wearable project to open a preview in the Decentraland Desktop client.

```bash
npm run start
```

To preview your scene on the [Decentraland mobile app](/docs/creator/build-for-mobile/mobile-client/overview/) instead, run `npm run start -- --mobile` (alias `-- -m`). The CLI prints a QR code that opens the scene on a phone connected to the same Wi-Fi network as your computer. See [Preview on mobile](/docs/creator/build-for-mobile/develop/preview-on-mobile/) for the full guide.

```bash
npm run start -- --mobile
```

To make the preview run smoother and render 3D models just as they'll look once published, add the `--local-ab` flag. See [Preview with optimized assets](/docs/creator/sdk7/getting-started/preview-scene/#preview-with-optimized-assets).

```bash
npm run start -- --local-ab
```

See [preview scenes](/docs/creator/sdk7/getting-started/preview-scene/) for details and special options when running a preview.

## Build

Run `npm run build` to build your project. Decentraland scenes are written in TypeScript, but they are built to minified JavaScript when published. See [coding scenes](https://github.com/decentraland/docs-creator/blob/main/sdk7/getting-started/coding-scenes.md) for more details.

The build command is optional, as it also runs in the background before deploying (although you can add a flag to skip it).

The build command runs more rigurous type checks than those that run with `npm run start`, running it can sometimes be helpful to debug a scene.

## Deploy a scene

Run `npm run deploy` to publish your scene to Decentraland. This command opens a browser window where you can sign with your wallet to authorize the deployment.

See [publishing](/docs/creator/sdk7/publishing/publishing/) for details and special options when publishing a scene.

## Troubleshooting

If you run into issues, see the [troubleshooting](/docs/creator/sdk7/debugging/troubleshooting/) section.
