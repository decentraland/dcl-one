> The available kinds of projects you can create in Decentraland.

# Kinds of Project

There are different types of content that you can make for Decentraland.

![](/docs/creator/images/content-types.webp)

## Scenes

Decentraland scenes can either be hosted in Parcels in Genesis City or in WORLDs.

### Publish to LAND

Scenes that are published to Parcels can be found at specific coordinates inside Decentraland's open world.

These scenes are linked ot LAND ore Estate tokens. Each parcel takes up 16x16 meters. Multiple adjacent parcels can be used up by a single scene, these can be arranged into any shape, as long as the borders touch.

LAND tokens can be bought in the [Marketplace](https://decentraland.org/marketplace/lands). There's a limited supply of them, covering the map of Genesis City.

Scenes published to LAND are easier to discover, as players may run into them while visiting nearby content or exploring.

Scenes published to LAND can use up to 15 MB of space per each parcel in the scene. The more parcels, the more room available. This is to prevent overloading the player's CPU, since players may be experiencing many nearby scenes at the same time. See [size limitations](/docs/creator/sdk7/optimizing/scene-limitations/).

### Publish to WORLDs

Scenes published to a World must be accessed via a link.

These scenes are linked to NAME tokens. NAME tokens can be bought in the [Marketplace](https://decentraland.org/marketplace/names/claim). You can claim any name you want as long as it's not claimed yet. Alternatively, you can use an [ENS domain](https://ens.domains) to create a World.

Scenes published to a World have a parcel limitation of 300x300. The same [size limitations](/docs/creator/sdk7/optimizing/scene-limitations/) per parcel apply as in scenes published to LAND parcels, but you can add more parcels to your scene without any cost.

If it's a Multi-Scene WORLD, different Collaborators (addresses) can have publishing rights to specific parcels of the WORLD, defined by the World Owner (the address that claimed the World NAME). Also, access to a World can be **Public** or **Private**. More information about [Managing Worlds](/docs/creator/scene-editor/publish/publish-scene/#managing-worlds)

A World can be **Public** or **Private**

#### Size Limits

The maximum file size you can upload to your World depends on whether you're using a Decentraland NAME or an ENS domain.

**Worlds from Decentraland NAMEs**

Decentraland NAME holders enjoy dynamic storage capacity within the Foundation Worlds Content Server, which depends on their wallet holdings. The following rules govern this allocation:

- Each Decentraland NAME you own grants 100 MB of storage capacity (as well as a World).
- Each Decentraland LAND parcel you own grants an additional 100 MB of storage capacity.
- For every 2,000 MANA held in your wallet, an additional 100 MB of storage capacity is granted.

The space in the Foundation Worlds Content Server can be used to host scenes as large as users want, utilizing the Decentraland NAMEs they own and the combined space granted by their collective Decentraland assets. For instance, a user with multiple Worlds (granted by owning multiple NAMES) and a combined storage capacity of 500 MB can choose to deploy one World with a 200 MB scene file, another with a 200 MB scene file, and a third with a 100 MB scene file. Alternatively, they could opt to deploy one World with a 300 MB scene file and another with a 200 MB scene file.

You can check how much of your storage budget is used and how much remains in two places:

- The **Manage** section of the Creator Hub. Click **View Details** for a breakdown of how your MANA, LAND, and NAME holdings add up.
- The **Worlds** tab of the [Builder](https://decentraland.org/builder/worlds), which also shows how much space each of your Worlds is using.

**Worlds from ENS Domains**

In contrast, Worlds granted from ENS domains have a fixed maximum scene file size of 36 MB per World, regardless of the user's other Decentraland holdings. Users with Worlds from ENS domains cannot increase their ENS World scene size limit by purchasing additional MANA or LAND.

However, Worlds granted by ENS domain ownership serve as the perfect first step into realizing the creative freedom offered by Decentraland.

#### Differences from LAND Scenes

Worlds offer several advantages over LAND scenes:

- No need to own LAND - add as many parcels as you want to your scene at no cost (up to 300x300)
- Access via link - players can visit your World directly via URL
- More storage capacity - especially for Decentraland NAME holders
- Access control - you can allow only certain players to access your World

See [Worlds](/docs/creator/sdk7/publishing/publishing-options/#decentraland-worlds) for more info, or learn about [publishing to Worlds](/docs/creator/sdk7/publishing/publishing/#publishing-to-worlds) and [configuring World settings](/docs/creator/sdk7/projects/scene-metadata/#world-configuration).

## Global Scenes

Global scenes can transform the already existing landscape of Decentraland, adding layers of interactivity and gameplay. These are scenes that are not constrained to only run on certain parcels of LAND or certain Worlds. Players carry them with them wherever they go.

<!-- #### Portable Experiences

A portable experience is linked to a NAME token. NAME tokens can be bought in the [Marketplace](https://decentraland.org/marketplace/names/claim). You can claim any name you want as long as it's not claimed yet.


> [!WARNING]
> **📔 Note**: If a NAME token is assigned to a World, it can't also be used for a Portable Experience. Prior content will be overwritten.


Portable experiences can be activated as part of the interactive code of a scene (either in LAND or a World).

Players are prompted asking if they want to run this portable experience, and if they do they'll carry it with them wherever they go for the rest of their session. [Learn more](/docs/creator/sdk7/projects/portable-experiences/). -->

### Smart Wearables

Smart wearables are linked to Wearable tokens. These are sold as NFTs and purchased in the [Marketplace](https://decentraland.org/marketplace/browse?section=wearables&vendor=decentraland&page=1&sortBy=newest&status=on_sale).

Smart Wearables are activated whenever the player puts on the associated wearable item. They are turned off if the player takes off the item, or they can also turn off the global scene manually via the UI. Also, the scene that the player is standing in is able to suspend any Smart Wearables that are active in the scene.

- Learn everything about [Creating wearables](/docs/creator/wearables-and-emotes/wearables/creating-wearables/).
- Learn about [smart wearables](/docs/creator/sdk7/projects/smart-wearables/)
