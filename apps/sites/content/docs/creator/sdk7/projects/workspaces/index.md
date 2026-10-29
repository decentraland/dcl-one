> Run multiple DCL projects at a time

# Workspaces

Run multiple Decentraland projects in preview by grouping these into a workspace. Run multiple adjacent scenes to see how they fit, or also run multiple [smart wearables](/docs/creator/sdk7/projects/smart-wearables/) together to see how they interact with each other and with different scenes.

Running multiple projects in a workspace provides a much more complete testing alternative, to ensure different content works well together. A workspace is a debugging feature, it doesn't affect the experience in the published scene.


> [!WARNING]
> **📔 Note**: The **Creator Hub** doesn't currently support handling Workspaces.


## Create a workspace

1. Download the [Goerli Plaza](https://github.com/decentraland/sdk7-goerli-plaza) repo.
2. Create a separate top-level folder to hold the workspace.
3. From the Goerli Plaza repo, copy the following files over to your workspace:
   * `dcl-workspace.json`
   * `package.json`
   * `.gitignore`
4.  Inside this folder, add one folder at root level for each project you want to work with. You can drag in existing folders with scenes or smart wearables. For new folders, run `npx @dcl/sdk-commands init` inside each, to create a Decentraland project.

    > Note: Make sure that the parcels on each of the scenes don't overlap.
5.  Standing on the workspace folder, run the following, to create the necessary files:

    `npm run update-parcels && npm run sync && npm run test && npm run format`

You can confirm which projects are part of the workspace by opening the `dcl-workspace.json` file and checking the paths listed in its `folders` array.

## Run a workspace

Run `npm run start` on the root folder of the workspace. This runs all of the projects at the same time, viewable in a single preview window. This preview behaves just like when previewing a single scene.

Any smart wearables in the workspace are available to try on by looking for them in the backpack.

## Add projects

Once a workspace is created, you can add additional projects by editing the `dcl-workspace.json` file manually. Add an entry to the `folders` array with the relative path to the folder you want to add. For example `my-other-example`.

You can also add a project that is not inside the workspace folder, by using the absolute path.


> [!WARNING]
> **📔 Note**: The folder must already contain a decentraland project initatied with `npx @dcl/sdk-commands init`. It can't be an empty folder.


To add or remove projects, edit the `dcl-workspace.json` file to include the relative paths to each of the projects in the workspace in the `folders` array.

```json
{
	"folders": [
		{
			"path": "example-scene"
		},
		{
			"path": "example-scene2"
		}
	]
}
```
