> What you can see in a scene's preview

# Preview Your Scene

Once you have [built a new scene](/docs/creator/sdk7/getting-started/preview-scene/#create-your-first-scene) or downloaded a [scene example](https://studios.decentraland.org/resources?sdk_version=SDK7) you can preview it locally.

## Using the Scene Editor in Creator Hub

Make sure you've [installed the Creator Hub](/docs/creator/scene-editor/get-started/editor-installation/).

1. Open your scene project.
2. Click the **Preview** button on the top-right corner. This will open a new window with the Decentraland Desktop Explorer, running just your scene. There you can move around the scene and interact with interactive items.

![](/docs/creator/images/editor/preview-button.webp)

Configure different preview options from the dropdown menu next to the **Preview** button:

- **Preview with**: Choose between **Desktop Client** (the default Decentraland Explorer) and **Bevy (Web)**, which opens the preview in your browser using the Bevy Web client. The Bevy Web option is equivalent to the `--web` CLI flag.
- **Open Debug Console**: Shows the scene's console output inside the Creator Hub while the preview runs. This is useful to debug errors in the scene. See [The Creator Hub console](#the-creator-hub-console).
- **Skip Auth Screen**: Skips the account selection screen and automatically logs you in with your currently logged in account. This is disabled by default, enable it if you want to test multiple accounts.
- **Landscape Terrain Enabled**: Toggles the landscape around the scene. This is enabled by default, disable it to lower the scene's memory footprint.
- **Enable MCP Server**: Launches the Explorer with the MCP automation server enabled, so AI agents can see and control the running preview. Only visible when your project's SDK version supports it. See [Vibe Coding with AI](/docs/creator/sdk7/getting-started/vibe-coding/#let-the-ai-see-your-scene-in-world) for the full workflow.
- **Optimize Assets**: Previews the scene with locally generated asset bundles, matching how it renders in production after [asset bundle conversion](/docs/creator/sdk7/optimizing/performance-optimization/#asset-bundle-conversion). The first run converts all assets, which can take several minutes on large scenes. Only available with the Desktop Client (not Bevy Web). See [Preview with optimized assets](/docs/creator/sdk7/getting-started/preview-scene/#preview-with-optimized-assets).
- **Show QR Code for Mobile**: Displays a QR code that opens your scene preview in the [Decentraland mobile app](/docs/creator/build-for-mobile/mobile-client/overview/). Scan the code with a phone on the same Wi-Fi network as your computer. See [Preview on mobile](/docs/creator/build-for-mobile/develop/preview-on-mobile/) for details.


> [!NOTE]
> **Tip:** You can also preview your scene directly on the Decentraland mobile app. Use the **Show QR Code for Mobile** option in Creator Hub, or run `npm run start -- --mobile` from the CLI. See [Preview on mobile](/docs/creator/build-for-mobile/develop/preview-on-mobile/) for details.


## Using the CLI

To preview a scene run the following command on the scene's main folder:

```bash
npm run start
```

Any dependencies that are missing are installed and then the CLI creates a local web server in your system and launches the scene in the Decentraland Desktop client via a `decentraland://` deeplink. The Desktop client is the default preview target.

To preview in a browser tab instead, add `-- --web` (or `-- --bevy-web`) to open the scene in the Bevy Web client at `decentraland.org/bevy-web/`.

Every time you make changes to the scene, the preview reloads and updates automatically, so there's no need to run the command again.


> [!WARNING]
> **📔 Note**: Some scenes depend on communicating with an external server to carry out custom logic or store and retrieve data. When previewing one of these scenes, you'll likely have to also run the server locally on another port. Check the scene's readme for instructions on how to launch the server as well as the scene.


### Parameters of the preview command

You can add the following flags to the `npm run start` command to change its behavior:

- `-- --web` (alias `-- --bevy-web`) Opens the preview in the Bevy Web browser client at `decentraland.org/bevy-web/` instead of the Desktop Explorer. Chromium-based browsers (Chrome 142+) require the Local Network Access permission for the hosted page to reach your local preview server — when the browser asks to access apps on your device, click "Allow".
- `-- --mobile` (alias `-- -m`) Shows a QR code in the terminal that opens your scene in the [Decentraland mobile app](/docs/creator/build-for-mobile/mobile-client/overview/) on a phone connected to the same Wi-Fi network. See [Preview on mobile](/docs/creator/build-for-mobile/develop/preview-on-mobile/).
- `-- --skip-build` Skip build and only serve the files in preview mode.
- `-- --port` (alias `-- -p`) to assign a specific port to run the scene. Otherwise it will use whatever port is available.
- `-- --no-browser` (alias `-- -b`) to prevent the preview from opening a new browser tab.
- `-- -w` or `-- --no-watch` to not watch for filesystem changes and avoid hot-reload whenever the scene's code changes.
- `-- --ci` To run the parcel previewer on a remote unix server.
- `-- --multi-instance` Allow running multiple Explorer instances simultaneously.
- `-- --local-ab` Preview with optimized asset bundles. The Desktop Explorer converts the scene's assets into asset bundles itself during preview, matching how the scene renders in production. Equivalent to the **Optimize Assets** option in Creator Hub. Only available with the Desktop Client (not Bevy Web). See [Preview with optimized assets](/docs/creator/sdk7/getting-started/preview-scene/#preview-with-optimized-assets).
- `-- --no-client` Suppress every auto-launch (desktop Explorer deeplink, browser open, mobile QR). The file watcher still notifies a desktop Explorer if it connects on its own. Useful when an external tool owns the Explorer process.
- `-- --mcp` Enable the MCP server in the Explorer (forwarded as a deep link parameter).
- `-- --mcp-port` Port for the MCP server in the Explorer (forwarded as a deep link parameter). For example: `npm run start -- --mcp --mcp-port 3001`.


> [!WARNING]
> **📔 Note**: Parameters need to be added with two series of dashes, for example `npm run start -- --web3`.


## Preview with optimized assets

When you publish a scene, the Decentraland servers convert all of its 3D models to asset bundles, an optimized format that is much lighter to load and render. See [Asset bundle conversion](/docs/creator/sdk7/optimizing/performance-optimization/#asset-bundle-conversion). By default, local previews skip this step and load the raw models instead.

Enable optimized assets to run this same conversion locally when you preview your scene. This has two benefits:

- The preview loads faster and runs smoother, especially on scenes with heavy 3D models.
- You see the models exactly as players will see them after publishing, so you can catch any issues with the asset compression before you publish.

This option only affects your local preview. The published scene is always converted on the servers, whether you use this option or not.

To enable it:

- **In the Creator Hub**: check **Optimize Assets** in the dropdown menu next to the **Preview** button.
- **From the CLI**: run `npm run start -- --local-ab`.

![](/docs/creator/images/editor/optimize-assets-checkbox.webp)

With this option enabled, the Decentraland explorer converts the scene's 3D models to asset bundles on your machine before loading them. The conversion is near instant for most scenes, but the first preview can take longer if the scene has very heavy assets. Converted models are cached, so on later previews only new or modified assets need to be converted.


> [!NOTE]
> **💡 Tip**: If the conversion of an asset fails for any reason, the preview falls back to loading the raw 3D models, just like a regular preview.


## Upload a scene to decentraland

Once you're happy with your scene, you can upload it and publish it to Decentraland. For this you must own LAND, a Decentraland NAME, or an ETH ENS name, or have permissions given by someone that does. See [publishing](/docs/creator/sdk7/publishing/publishing/) for instructions on how to do that.

## Preview scene size

The scene size shown in the preview is based on the scene's configuration.

Edit this on the second tab of the scene menu in the Scene Editor.

![](/docs/creator/images/editor/scene-parcels-3x3.webp)

Use the dropdowns and click **Apply Layout** to change the dimensions of your scene. You can also click each individual parcel to toggle it off from your layout.

![](/docs/creator/images/editor/scene-parcels-toggled.webp)

You can also edit the _scene.json_ file to list multiple parcels in the "parcels" field. See [set parcels via the command line](/docs/creator/sdk7/projects/scene-metadata/#scene-parcels) for more details.


> [!NOTE]
> **💡 Tip**: While running the preview, the parcel coordinates don't need to match those that your scene will really use, as long as they're adjacent and are arranged into the same shape. You will have to replace these with the actual coordinates later when you [deploy the scene](/docs/creator/sdk7/getting-started/preview-scene/#upload-a-scene-to-decentraland).


## View the scene console

Open the console by clicking the ![](/docs/creator/images/console-icon.webp) icon on the top-right corner. Here you can see any error messages, and also any text that your scene prints to the console via `console.log()`.

You can also open it by pressing the **\`** key on your keyboard. You can also press Shift + **\`** to open the console even wider, in case you need to view more text.

## The Creator Hub console

The Creator Hub can show the same output without leaving the editor. Turn on **Open Debug Console** in the preview options, then run a preview. A **CONSOLE** tab appears next to **FILE SYSTEM** and **ASSET PACKS** in the panel at the bottom of the editor.

To move the console out of the way, click the pop-out icon in its header, labeled **Open console in a separate window**. The console opens as its own window titled **Console**, which you can move to a second monitor and keep visible while you edit.

The separate window has a **Clear console** button, and a **Dock back in the editor** button that returns the console to the bottom panel. Meanwhile the **CONSOLE** tab in the editor reads **Console opened in a separate window** and offers a **Dock back here** button that does the same thing.


> [!NOTE]
> **💡 Tip**: The console keeps collecting output while it's detached, so docking it back doesn't lose anything. When no preview is running it reads **Run a scene to see debug output**.


## Test a multiplayer scene locally

If you launch a scene preview and open it in two (or more) different explorer windows, each open window will be interpreted as a separate player, and a mock communications server will keep these players in sync.

Interact with the scene on one window, then switch to the other to see that the effects of that interaction are also visible there.

Using the Creator Hub, click the Preview button a second time, and that opens a second Decentraland explorer window. You must connect on both windows with different addresses. The same sessions will remain open as the scene reloads.

![](/docs/creator/images/editor/preview-button.webp)

As an alternative, you can open a second Decentraland explorer window by writing the following into a browser URL:

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

### Advanced: Fast iteration with remote asset bundles


> [!NOTE]
> **💡 Tip**: For most scenes, the simplest way to preview with asset bundles is to enable [optimized assets](/docs/creator/sdk7/getting-started/preview-scene/#preview-with-optimized-assets), which converts your models locally and always reflects their latest version. The mode below is an alternative that reuses the bundles already published on the servers.


For heavy scenes with many 3D models, you can speed up scene loading and reloading by reusing the [asset bundles](/docs/creator/sdk7/optimizing/performance-optimization/#asset-bundle-conversion) that are already published on Decentraland's servers, instead of loading the raw unoptimized 3D models. This is especially useful when iterating on code-only changes.

To enable this mode, launch the Decentraland Desktop client with the following arguments:

```bash
npm run start -- --realm http://127.0.0.1:8000/ --position 0,0 --local-scene true --debug --skip-version-check true --lsd-use-remote-ab <ab-source>
```

The `<ab-source>` argument changes depending on where the scene is already published:

- **In Genesis City**: `--lsd-remote-ab-server Genesis`
- **In a World**: `--lsd-remote-ab-world <world-name>.dcl.eth`

For example, to preview a local copy of a scene that's already deployed to a World:

```bash
npm run start -- --realm http://127.0.0.1:8000/ --position 0,0 --local-scene true --debug --skip-version-check true --lsd-use-remote-ab --lsd-remote-ab-world myworld.dcl.eth
```

In both cases, `--realm http://127.0.0.1:8000/` points the client at your local preview server (run `npm run start` first to start it), and `--local-scene true` tells the client to load the scene's code from there.


> [!WARNING]
> **📔 Important**: When using this mode, it's recommended that **all** of its art are already published, with their asset bundles fully processed by the content servers. If you've added any new assets, you'll miss out on the optimized loading as they will be loaded as raw gltf files, as happens when you normally run a preview. But if you locally modified an asset that was already published, maintaining the same file name, then you'll be seeing the old published version of that asset.
> 
> In that case, redeploy the scene first, wait a few minutes for the asset bundles to be generated (see [Asset bundle conversion](/docs/creator/sdk7/optimizing/performance-optimization/#asset-bundle-conversion)), and then resume using this mode for code-only iteration.

