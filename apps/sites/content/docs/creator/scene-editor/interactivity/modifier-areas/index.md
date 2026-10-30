> Use modifier areas in the Scene Editor to change avatar behavior or force a camera mode inside a region of your scene.

# Modifier Areas

Modifier areas are invisible regions that change how players experience part of your scene. There are two types:

- **Avatar Modifier Area**: Changes how avatars behave or appear for players inside the region, for example hiding avatars or disabling passport popups.
- **Camera Modifier Area**: Forces the player's camera into first person or third person while they are inside the region.

Both types revert to normal behavior when the player walks out.

## Adding a modifier area

You can add a modifier area in two ways:

**From the asset catalog:** Open the asset catalog and search for **Avatar Modifier Area** or **Camera Modifier Area** under the **Utils** category. Drag it into the scene. The item appears as a translucent placeholder cube that is only visible in the editor, not in the running scene.

**From the components menu:** Select any existing entity, click the **+** button at the top of the properties panel, and choose **Avatar Modifier Area** or **Camera Modifier Area** from the list. See [Add components](/docs/creator/scene-editor/build/components/#add-components) for more details.

![The add component menu open on an entity named Stage area, listing components under the 3D Content and Interaction groups, including Avatar Modifier Area and Camera Modifier Area.](/docs/creator/images/editor/add-component.webp)

## Sizing the area

The size of a modifier area is controlled by the entity's **Transform scale**. If you scale the entity to 4 x 3 x 4, the modifier region becomes a 4 x 3 x 4 meter box centered on the entity's position. Use the scale gizmo or type exact values in the Transform component to set the size you need.


> [!WARNING]
> **Note:** There is no separate "Area" field in the editor panel. The editor keeps the internal area property in sync with the entity's scale automatically, so resizing the entity is all you need to do.


## Avatar Modifier Area

The Avatar Modifier Area panel has two settings:

![The Avatar Modifier Area component panel, with a Modifiers dropdown showing Hide Avatars selected, an Exclude Player IDs section, and an Add Player ID button.](/docs/creator/images/editor/avatar-modifier-area-component.webp)

### Modifiers

A multi-select dropdown with the following options:

- **Hide Avatars**: Avatars inside the area become invisible. Players inside cannot see each other, but players outside the area still see them normally.
- **Disable Passports**: Clicking on a player's avatar inside the area no longer opens their profile popup.

You can select one or both modifiers on the same area. When you add the component, **Hide Avatars** is selected by default. Click the **x** on a modifier to remove it.

### Exclude Player IDs

A list of wallet addresses that are not affected by the modifiers. Players whose address is in this list behave normally even while inside the area.

Click **Add Player ID** to add an entry, then paste the wallet address. To remove one, click the three-dot menu next to it and select **Remove Player ID**.

This is useful for events where you want to hide audience avatars on a stage but keep the performers visible.


> [!WARNING]
> **Note:** Make sure wallet addresses are lowercase. The modifier area uses exact string matching.


### Use cases

- **Privacy zones**: Hide avatars in a backstage or changing area.
- **Clean stages**: Hide audience avatars on a stage so only performers (listed in Exclude Player IDs) are visible.
- **Game arenas**: Disable passports in a combat area so players don't accidentally open profiles during gameplay.

## Camera Modifier Area

The Camera Modifier Area panel has one setting:

![The Camera Modifier Area component panel, with a Camera Mode dropdown set to First Person.](/docs/creator/images/editor/camera-modifier-area-component.webp)

### Camera Mode

A dropdown with two options:

- **First Person**: Forces the player into first-person view while inside the area.
- **Third Person**: Forces the player into third-person view while inside the area.

The default is **First Person**. When a player leaves the area, their previous camera mode is restored.


> [!NOTE]
> **Tip:** Cinematic camera mode is not available here. To control the camera beyond first/third person switching, use a [Virtual Camera](/docs/creator/sdk7/3d-essentials/camera/#using-virtual-cameras) with code.


### Use cases

- **Narrow corridors or puzzle rooms**: Force first person for precise clicking on small objects.
- **Scenic overlooks**: Force third person so players can see their avatar against the backdrop.
- **Tutorial zones**: Ensure all players use the same camera mode during a guided experience.

## Combining modifier areas

You can place both types of modifier area on the same entity, or overlap separate modifier areas in the same space. Each area applies its effects independently.


> [!WARNING]
> **Note:** Keep the number of modifier areas in your scene low. Too many can affect performance. A handful is fine for most scenes.


## Using modifier areas with code

The Scene Editor components correspond directly to the SDK7 `AvatarModifierArea` and `CameraModeArea` components. For full details on the available modifiers, runtime behavior, and advanced options like dynamically changing the exclude list, see:

- [Avatar modifier areas](/docs/creator/sdk7/interactivity/avatars/modifier-areas/) (SDK7)
- [Camera mode areas](/docs/creator/sdk7/3d-essentials/camera/#1st-and-3rd-person-camera-modes) (SDK7)
