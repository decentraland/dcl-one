# pointers

Pointers are unique, case-insensitive strings that reference an active [entity](/docs/contributor/content/entities/). Content servers can resolve these pointers to obtain the entity's identifier.

Remember that both [entities](/docs/contributor/content/entities/) and their [files](/docs/contributor/content/file-system/) are immutable in the Decentraland content system, and their identifiers change when replacement versions are uploaded. Pointers, on the other hand, are stable references that persist across replacements. This is achieved by automatically redirecting the pointer to a new entity when the owner uploads a replacement.

Multiple pointers can resolve to the same [entity](/docs/contributor/content/entities/), as is often the case with [scenes](/docs/contributor/content/entity-types/scenes/).

As you may gather, the most common use of pointers is obtaining the ID for the active version (i.e. latest replacement) of an [entity](/docs/contributor/content/entities/), in order to download it.

### Pointer Types

There are 4 types of pointers that content servers can resolve to entities, each with its own syntax:

* [Scene pointers](/docs/contributor/content/entity-types/scenes/#pointers) are parcel coordinates, such as `"0,0"`.
* [Profile pointers](/docs/contributor/content/entity-types/profiles/#pointers) are the Ethereum address of the owner.
* [Wearable pointers](/docs/contributor/content/entity-types/wearables/#pointers) and [emote pointers](/docs/contributor/content/entity-types/emotes/#pointers) are either [collection](/docs/contributor/content/collections/) item or off-chain URNs.
* [Store pointers](/docs/contributor/content/entity-types/stores/#pointers) are off-chain URNs with the Ethereum address of the owner.
* [Outfits pointers](/docs/contributor/content/entity-types/outfits/#pointers) are the Ethereum address of the owner suffixed by `:outfits`.

You can find more details in their specific sections.

### Resolving Pointers

Content servers have an endpoint that can resolve pointers into their active entity's manifest, by making a `GET` request to:

```
https://<content-base-url>/entities/<entity-type>?pointer=<pointer>
```


> [!NOTE]
> For historical reasons, the response from this endpoint is an array containing one element.


Some useful examples, using the Foundation's content server:

```bash
# An on-chain collection pointer to a wearable:
curl "https://peer.decentraland.org/content/entities/wearables/?pointer=urn:decentraland:matic:collections-v2:0x30517529cb5c16f686c6d0b48faae5d250d43005:0"

# An off-chain collection pointer to a default asset:
curl https://peer.decentraland.org/content/entities/wearables/?pointer=urn:decentraland:off-chain:base-avatars:BaseFemale

# A parcel pointer:
curl https://peer.decentraland.org/content/entities/wearables/?pointer=0,0

# A profile pointer:
curl https://peer.decentraland.org/content/entities/profiles/?pointer=0xe2c2b80ca5ad868f4b30fa83cca2bf12cc95b4fe
```

Note that, if you already know the entity's ID, you can [download the manifest](/docs/contributor/content/file-system/#downloading) file directly.
