> Fixes for common problems

# Troubleshooting

## Debug with an AI assistant

Before digging through the issues below, consider handing the problem to an AI assistant, like Cursor's chat, GitHub Copilot, or Claude Code. Paste the error message from the console, or describe what's not behaving as expected, and it can usually find the problem in your scene's code and fix it for you.

For good results, make sure the AI has the Decentraland SDK skills installed. The skills teach it the SDK's patterns and constraints, so it doesn't guess based on generic or outdated information. Install them by running this command in your scene project:

```bash
npx skills add decentraland/sdk-skills
```

See [Vibe Coding with AI](/docs/creator/sdk7/getting-started/vibe-coding/) for more details on setting up and prompting AI assistants.

You can also let the AI debug the scene _while it's running_. The Decentraland desktop client can expose an MCP server that lets an agent take its own screenshots, read the scene's console output, walk the player around, and click on objects — so instead of you reproducing the bug and pasting the error, the agent reproduces it itself, sees what happens, and iterates until it's fixed. Launch the scene with `npm run start -- --mcp` and connect your agent to it.

See [Let the AI see your scene in-world](/docs/creator/sdk7/getting-started/vibe-coding/#let-the-ai-see-your-scene-in-world) for the full setup, and install the `unity-explorer-mcp` skill so the agent knows the workflow:

```bash
npx skills add decentraland/sdk-skills --skill unity-explorer-mcp
```

## Issues when running preview

#### Issue: Can't run any scene preview, error message says mentions **Permissions denied** or **EACCES**

Your operating system doesn't allow you edit permissions on the folder where you want to run the project. When running the scene, some dependencies need to be installed, but it's forbidden. You need to configure the folder's permissions to allow your Windows/Mac/Linux user account to edit files in them.

Useful resources:

- [docs.npmjs](https://docs.npmjs.com/resolving-eacces-permissions-errors-when-installing-packages-globally)
- [letscodepare](https://letscodepare.com/blog/npm-resolving-eacces-permissions-denied)

#### Issue: Can't run a particular scene preview, error says **Error: Error while building the project**

If you're running a scene that was shared with you, make sure that this scene wasn't shared containing a `node_modules` or `bin` folder, or a `package-lock.json` file. These files contain dependencies that use versions that are specific to your OS and machine, they should be generated when running the scene for the first time. Delete these folders & file manually, then run `npm run start` again.

#### Issue: Running `npm run start` runs, no error message, but no browser window opens and no URL in the output to open the preview

Make sure your Node version is up to date. It must be 20 or newer.

#### Issue: Running `npm run start` opens a browser tab, but the loading screen never finishes loading, or I see a red error banner that says "critical error".

- Make sure you have the latest version of the Decentraland SDK installed in your project. Run:

  `npm i @dcl/sdk@latest`

#### Issue: The scene runs, in the console I see `Cyclic dependencies` warnings.

These refer to files in your scene that reference each other mutually. This is not necessarily a problem, but is not a recommendable pattern for writing software, as it can lead to hard to debug race conditions and other issues. Your scene is likely to work well in spite of these warnings.

Ideally, the loading of the code in your scene should follow a clear sequential order. Code that has cyclic dependencies might suffer a chicken & the egg problem, where the compiler doesn't know which to initiate first. Often this is resolved without issues, but it's a good practice to avoid.

To fix these dependencies, you often must resort to calling functions or object constructors passing references to already instanced entities/objects in the function arguments; Instead of hard-coding references to these entities/objects in the function, which may or may not already be instanced.

## Issues when deploying

#### Issue: You don't have permissions to deploy to these parcels

- Make sure that the `scene.json` file of your scene correctly lists the coordinates where you want to deploy.
- Make sure that Metamask is correctly set up to use the right wallet to sign the transaction. This may either be the wallet that owns the LAND tokens, or might have operator permissions granted by the owner.

#### Issue: Running `npm run deploy` fails

- Check the spawn points of your scene, all three x,y,z coordinates of a spawn point must either be a number or a range. Either all three are numbers or all three are ranges. It's not supported to have ranges for some but numbers for others.

  For example this is not supported:

  `"position": {"x": [1,4], "y": 0, "z": [1,4]}`

  This is supported:

  `"position": {"x": [1,4], "y": [0,0], "z": [1,4]}`

- The default catalyst server that you're assigned to deploy to might be down or having issues. You can force the `npm run deploy` command to deploy to a specific catalyst server instead. To deploy to a specific server on the Decentraland Editor:

  1. Open your scene and click **Publish**
  2. Select the option **Publish to a different server** on the bottom.
  3. On the dropdown, select **Custom Server**
  4. Enter the address of the server, for example `peer-ec1.decentraland.org`
  5. Click **Publish to custom server**
  6. Approve the transaction as with a normal deployment.

  To do this via the CLI:

  `npm run deploy -- --target-content <server-name>`

  For example:

  `npm run deploy -- --target-content peer-ec1.decentraland.org`

  See [catalyst-monitor](https://decentraland.github.io/catalyst-monitor/) for a status check of all the servers in the catalyst network. You can also copy the addresses of each one, from the top of each card.

- Check your scene's `package.json`. A common problem is that there's a `bundleDependencies` and also a `bundledDependencies` (extra d) section. This can sometimes result from running different Node versions on the same project at different times, or from sharing the project between people that ran it with different Node versions installed. Delete `bundleDependencies`, which relates to older Node versions.

Also ensure you have your Node version up to date, at least version 20.

#### Issue: Running `npm run deploy` or `npm run build` reports type errors

Your scene might have type errors reported by TypeScript, for example stating that a certain variable might be type `any` or that `undefined` or `null` are not allowed. When running `npm run deploy`, it also runs `npm run build`, which is a bit more strict with these checks than `npm run start`.

Unlike JavaScript, TypeScript enforces strict typing of all variables. Even though your scene is written in such a way that for example a certain value will never be `undefined`, TypeScript needs to know what would happen in that scenario, or you need to explicitly clarify that the value can only be for example a string.

As an alternative, you can run `npm run deploy -- --skip-build` to skip the running of `npm run build`, and prevent these checks from running.

#### Issue: I deployed my scene but I don't see the changes when I enter Decentraland

- Keep in mind that it can take a few minutes if the scene contains heavy assets.
- While the new version's 3D models are being converted to asset bundles, players are deliberately served the last fully-working version of your scene. This usually only takes a few seconds, but can take longer for very large scenes or when the servers are busy. You can [check the conversion status](/docs/creator/scene-editor/publish/publish-scene/#check-the-conversion-status) of your scene directly.
- Reloading the scene is not enough to pick up a new version: a reload restarts the scene's code, but doesn't fetch the newly published version. Once the conversion is complete, fully quit and relaunch Decentraland, then enter the scene again via a jump link or the `/goto` chat command.

#### Issue: Some players see the new version of my scene, others still see the old one

The conversion of your scene's 3D models finishes at different times for each platform (Windows and Mac), and each player's client also keeps a local cache. To check if the conversion is done for all platforms, [check the conversion status](/docs/creator/scene-editor/publish/publish-scene/#check-the-conversion-status) and compare the `windows` and `mac` values under `assetBundles`. Once both are `complete`, ask the affected players to fully quit and relaunch Decentraland.

#### Issue: The publication is stuck on the Converting stage, the Jump In button never appears

Your scene is either queued behind other scenes being converted, or the conversion failed.

- Open `https://asset-bundle-registry.decentraland.org/queues/status` and look for your scene's entity ID. You can find that ID in the `entityId` field when you [check the conversion status](/docs/creator/scene-editor/publish/publish-scene/#check-the-conversion-status) using your scene's coordinates. If your ID is listed in the queue, the scene is queued and you just need to wait.
- If it's not in the queue, [check the conversion status](/docs/creator/scene-editor/publish/publish-scene/#check-the-conversion-status) of your scene. If a platform shows `failed`, publish the scene again. If it fails a second time, [report the problem](/docs/creator/sdk7/debugging/report-bug/) and include the entity ID.

#### Issue: Once deployed, some 3D models are missing, black, or untextured

- If you just published, the conversion of your models to asset bundles may not be done yet. Type `/detectabs` into the chat window: models tinted red are not yet converted. [Check the conversion status](/docs/creator/scene-editor/publish/publish-scene/#check-the-conversion-status) and wait for it to complete.
- Make sure the 3D models are all within the scene boundaries, even their bounding boxes. If any part of your models extend beyond these limits when running a preview, these parts that extend will be cut off and not rendered, both when running a preview and on the published scene.
- If textures are the problem, keep in mind that textures in 3D models are capped to a maximum size of 1024x1024 pixels during the conversion.

#### Issue: The scene looks fine up close, but is broken or missing when seen from a distance

The low Level of Detail (LOD) versions of your assets, used to render the scene from far away, are generated in the last stage of the publication and may not be finished yet. This doesn't block testing your scene from up close. [Check the conversion status](/docs/creator/scene-editor/publish/publish-scene/#check-the-conversion-status) and look at the values under `lods`, or simply wait and check again later.

#### Issue: Once deployed, my 3D models look different

- If the textures look different, keep in mind that textures in 3D models are capped to a maximum size of 1024x1024 pixels. This conversion is carried out to ensure that Decentraland runs smoothly for everyone.
- If models look different, there could be an issue with the conversion of the models to asset bundles. Read more about asset bundle compression [here](/docs/creator/sdk7/optimizing/performance-optimization/#asset-bundle-conversion).

  To validate this, try running the scene with the URL parameter `&DISABLE_ASSET_BUNDLES`. If the models look fine with this flag, the issue must be related to a bug in the conversion of the model. In that case, [report the problem](/docs/creator/sdk7/debugging/report-bug/) and include the entity ID of your deployment.

  Note that the generation of compressed asset-bundle versions of your models takes the servers a little time (usually a few minutes, longer for very large scenes or when the servers are busy). You can check if the models are being loaded as compressed asset bundles or not by writing the following command into the chat window `/detectabs`. Compressed models are tinted green, non-compressed are tinted red.

  You can also reproduce this conversion locally before publishing, by enabling [optimized assets in the preview](/docs/creator/sdk7/getting-started/preview-scene/#preview-with-optimized-assets). This lets you catch and debug these issues without having to deploy the scene.

#### Issue: My scene vanished entirely

- If the scene is in a World: your Worlds storage budget may be exceeded, for example after selling or transferring NAMEs, LAND, or MANA. You have 48 hours to free up space or increase your budget before your Worlds become inaccessible. Check your budget in the **Manage** tab of the Creator Hub or in the **Worlds** tab of the [Builder](https://decentraland.org/builder/worlds), then republish. See [Worlds size limits](/docs/creator/sdk7/projects/kinds-of-project/#size-limits).
- If the scene is on LAND: someone with deploy permissions may have published a new scene over your parcels, which erases the previous content. See [scene overwriting](/docs/creator/sdk7/publishing/publishing/#scene-overwriting).
- In rare cases, content that violates Decentraland's content policy can be denylisted from the content servers. If you believe this happened by mistake, reach out through the [Decentraland Discord](https://decentraland.org/discord).

#### Issue: My scene has poor FPS in production, even though it runs smoothly in preview.

Your scene's performance could be affected by neighboring scenes that follow bad practices, as they also run in parallel. You can validate that this is the case by opening the settings and setting the line of sight to a minimum, so that only 1 parcel around your current scene is loaded.

You can reduce the line of sight even further by running your scene with the parameter `&LOS=0`, to not load any surrounding scenes at all.

If you just deployed your scene, the burden when loading the scene might also be reduced once the servers convert the 3D models in the scene to compressed asset bundles. You can check if the models are being loaded as compressed asset bundles or not by writing the following command into the chat window `/detectabs`. Compressed models are tinted green, non-compressed are tinted red.

### Report a bug

If you encounter a problem that is not with your scene, but instead with the Decentraland SDK in general, please see [Report a bug](/docs/creator/sdk7/debugging/report-bug/).
