> How to install the scene editor.

# Creator Hub Installation

![Creator Hub](/docs/creator/images/editor/icon-creator-hub.webp)

Download the Creator Hub [HERE](https://decentraland.org/download/creator-hub).

## Updating the Creator Hub

The Creator Hub application checks for updates every time you open it, and self-updates if there's a new version available.

You can also check for updates manually. For this,

1. Click the Creator Hub logo in the top-left corner of the screen to open the main menu.
2. Click **Check for Updates**.

![The Creator Hub main menu, opened from the logo in the top-left corner, with the options About Creator Hub, Check for Updates, Settings, Report Bug, and Help & Support.](/docs/creator/images/editor/main-menu.webp)

The main menu is also where you find the app **Settings**, and links to report a bug or get support.

## Editing code

If you also plan on reading and editing code in your scene, you'll also need to install either:

* <img src="/docs/creator/images/editor/vscode.webp" alt="VS Code" data-size="line"> [Visual Studio Code](https://code.visualstudio.com/): This is the recommended option for experienced developers.
* <img src="/docs/creator/images/editor/cursor-icon.webp" alt="Cursor" data-size="line"> [Cursor AI](https://www.cursor.com/): This is a powerful code editor that is integrated with AI. It lets you pick different AI models to help you write code; a free tier is available, and more advanced models require a paid plan. This is a good option for developers who are new to Decentraland or TypeScript, or if you want to save time writing code.

You may need to select your Code Editor in the settings of the Creator Hub. To do this,

1. Click the Creator Hub logo in the top-left corner of the screen and select **Settings**.
2. Open the **EDITOR** tab.
3. Under **Code editor of choice**, select your Code Editor. You may find your editor listed in the dropdown, or you may need to select **Choose from your device...** to find it.

![The App Preferences dialog on the EDITOR tab, showing the Code editor of choice dropdown set to Visual Studio Code, the Preview Options checkboxes, and the App Warnings checkbox.](/docs/creator/images/editor/settings-editor.webp)

### AI skills

If you plan to use an AI agent to help you write code, we recommend installing the Decentraland SDK skills. These are ready-made instruction sets that teach your AI agent how to work with the Decentraland SDK, so it makes fewer mistakes and gives better results from the very first prompt. To install all of them, run the following command in your scene project's folder:

```bash
npx skills add decentraland/sdk-skills --all
```

See [Vibe coding](/docs/creator/sdk7/getting-started/vibe-coding/) for more details on installing specific skills and how to use them.

## Troubleshooting

If you run into issues, see the [troubleshooting](/docs/creator/sdk7/debugging/troubleshooting/) section.
