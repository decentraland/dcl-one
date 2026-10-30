> Combine content created on the Scene Editor with the power of writing code.

# Combine with code



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/55H37rygD7M" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="https://www.youtube.com/watch?v=55H37rygD7M">Watch on YouTube</a></p>



The Creator Hub plus custom code is a very powerful combination for creating content. You can use the canvas to visually position items intuitively, and then write code that interacts with these items with complete freedom. You can even place a smart item, that has its own default behavior, and write code that reacts to when the item is activated.

For example, you can take advantage of an existing lever smart item, that already comes with its sounds and animations and states, and write code that detects when the lever is pulled to run your own custom logic.

See [Reference items in code](/docs/creator/scene-editor/code/reference-items/) for how to fetch items by name or by tags from your code.

## Editing code

You must install a code editor on your machine to edit the code of your scene. The recommended options are:

- <img src="/docs/creator/images/editor/vscode.webp" alt="VS Code" data-size="line"> [Visual Studio Code](https://code.visualstudio.com/): This is the recommended option for experienced developers.
- <img src="/docs/creator/images/editor/cursor-icon.webp" alt="Cursor" data-size="line"> [Cursor AI](https://www.cursor.com/): This is a powerful code editor that is integrated with AI. It lets you pick different AI models to help you write code; a free tier is available, and more advanced models require a paid plan. This is a good option for developers who are new to Decentraland or TypeScript, or if you want to save time writing code.


> [!WARNING]
> **📔 Note**: If you are on macOS, make sure the code editor app is in the Applications directory.


Once installed, you may need to select your Code Editor in the settings of the Creator Hub. To do this,

1. Click the Creator Hub logo in the top-left corner of the screen and select **Settings**.
2. Open the **EDITOR** tab.
3. Under **Code editor of choice**, select your Code Editor. You may find your editor listed in the dropdown, or you may need to select **Choose from your device...** to find it.

![The App Preferences dialog on the EDITOR tab, showing the Code editor of choice dropdown set to Visual Studio Code.](/docs/creator/images/editor/settings-editor.webp)

## Open a scene's code

Once you installed a code editor on your machine, and selected it in the settings of the Creator Hub, you can click the **< > CODE** button to open it on your scene project.

![](/docs/creator/images/editor/code-button.webp)

This opens a separate window with the code editor. On the left margin you can navigate the files and folder structure of your project.

![](/docs/creator/images/editor/files-on-vs-studio.webp)

Add your custom code in the `index.ts` file under `/src`, inside the `main()` function. You can otherwise add custom code outside that function or create new `.ts` files inside the `/src` folder, but these must be somehow referenced inside the `main()` function of `index.ts`.


> [!WARNING]
> **📔 Note**: If you have VS Code or Cursor installed but the **CODE** button doesn't open it, it may be that VS Code is not properly configured on your machine to open via the command line. In most cases, this is handled as part of the default installation, but in case it's not, see [these instructions from VS](https://code.visualstudio.com/docs/setup/mac#_launching-from-the-command-line) to enable VS Code from the command line.


If you have a preview window open running your scene, whenever you change the code in your files and save, the scene reloads automatically with your changes.

## Using AI

The Creator Hub has a built-in [AI Assistant](/docs/creator/scene-editor/code/ai-assistant/) that sees your open scene and edits it for you, without leaving the app. It is an experimental feature, turned off by default.

You can also leverage AI assistants like Cursor or Claude Code to help you write scene code. For example to use Cursor, do this:

1. Open the Cursor AI assistant by clicking the **AI** button in the top-right of the screen
2. There you can prompt the AI assistant to help you write code. Your prompts can include links to docs pages, paths to specific files in your project, or even images. You can also select a specific model to use from the dropdown.

To give your AI assistant Decentraland context, install the official Decentraland SDK Skills. These are ready-made instruction sets, maintained by the Decentraland Foundation, that teach your AI agent verified SDK7 patterns for every topic: scaffolding entire scenes, adding 3D models, multiplayer, deployment, and more.

```bash
npx skills add decentraland/sdk-skills
```

See [Vibe Coding with AI](/docs/creator/sdk7/getting-started/vibe-coding/) for the full guide, including the complete list of available skills.


> [!WARNING]
> **📔 Note**: Older scene projects may contain a `/dclcontext` folder, downloaded by the deprecated `npx sdk-commands get-context-files` command. That context is no longer maintained and is superseded by the SDK Skills. You can safely delete the folder.


## Version control

We recommend that you create a repo for your project on GitHub, and use it to keep track of your project's versions and to work collaboratively with others.

If you're not familiar with how to do this, see [Quickstart for repositories](https://docs.github.com/en/repositories/creating-and-managing-repositories/quickstart-for-repositories), or use the [GitHub desktop application](https://desktop.github.com/download/) for a simpler UI-based flow.


> [!WARNING]
> **📔 Note**: Upload the entire project folder to a GitHub repo, but make sure the `/node_modules` or `/bin` folders and the `package-lock.json` file are all included in the `.gitignore` file, to avoid syncing them. This should be the case if you configure the repo to be of type `node`. These files are all auto-generated, and the content may differ for different machines.


## See also

- [Vibe Coding with AI](/docs/creator/sdk7/getting-started/vibe-coding/): build scenes by describing what you want to an AI assistant.
- [Smart items - Basics](/docs/creator/scene-editor/interactivity/smart-items/)
- [Smart items - Advanced](/docs/creator/scene-editor/interactivity/smart-items-advanced/)
- [States and conditions](/docs/creator/scene-editor/interactivity/states-and-conditions/)
- [Making any item smart](/docs/creator/scene-editor/interactivity/make-any-item-smart/)
- [SDK Quick start](/docs/creator/sdk7/getting-started/sdk-101/): follow this mini tutorial for a quick crash course.
- [Development workflow](/docs/creator/sdk7/getting-started/dev-workflow/): read this to understand scene creation from end to end.
- [Examples](https://studios.decentraland.org/resources?sdk_version=SDK7): dive right into working example scenes.
