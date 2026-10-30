> How to add libraries to a scene

# Manage dependencies

A lot of times, your scene might need functionality that is already encapsulated in a reusable library. Using external libraries can make your work a lot easier.

You can import external libraries into a Decentraland project and then reference the systems, components, or functions described in these libraries.

Check the [Examples page](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library) to find a series of libraries, both created by the Decentraland Foundation and by community members, that solve common problems.

## Via the CLI

### Install

To install a library in your project folder, run `npm i` and the library name in your project's root path. For example:

`npm i @dcl-sdk/utils`

> **📔 Note**: You may need to run `npm run start` or `npm run build` after installing the library for the library's dependencies to get installed.

### Update

If your `package.json` file lists the version of the library as `@latest`, then run `npm i` to update all libraries to their corresponding latest versions.

If your `package.json` references a specific version number, you can update it by running the command to install the library, adding `@` and the version number in the end. For example:

`npm i @dcl-sdk/utils@1.7.5`

### Uninstall

To delete a library from your scene's dependencies run `npm rm` and the library name. For example:

`npm rm @dcl-sdk/utils`

It's a good practice to remove any libraries that you're not using. Unused libraries still occupy space in the scene, affecting the player's experience of downloading and running your scene.
