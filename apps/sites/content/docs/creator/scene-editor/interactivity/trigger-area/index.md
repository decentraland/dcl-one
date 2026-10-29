> React to the player's position

# Trigger Area

To make something happen when the player walks into or out of an area, use a Trigger Area [smart item](/docs/creator/scene-editor/interactivity/smart-items/).

![](/docs/creator/images/editor/trigger.webp)

The orange cube you see while editing your scene is only visible in the Scene Editor, it becomes invisible when running a preview of the scene. You can easily adjust and scale the orange cube to cover exactly the area you need.

Drag it into your scene and resize it with the scale gizmo. The box or sphere you see is only drawn in the editor, players never see it.

If any part of the player's avatar overlaps with this orange cube, the assigned event will be called. Trigger areas react only to the player on the local machine, not to other players' avatars — each player fires the trigger on their own instance of the scene.


> [!NOTE]
> **💡 Tip**: If the trigger areas in your scene start getting in the way of editing other content, remember you can always lock and/or hide them from the [Entity Tree](/docs/creator/scene-editor/get-started/scene-editor-essentials/#the-entity-tree).
> 
> <img src="/docs/creator/images/editor/hide-trigger.webp" alt="" data-size="original">


Multiple trigger areas can overlap, and don't affect each other.

You describe what should happen and the [AI Assistant](/docs/creator/scene-editor/code/ai-assistant/) writes a small script for it.

### Settings

The item's **Script** component has two fields:

| Field            | What it does                           | Options                                                                                     | Default     |
| ---------------- | -------------------------------------- | ------------------------------------------------------------------------------------------- | ----------- |
| **Activated By** | What sets the area off.                | `my player`, `all players`, `any collider`, `clickable`, `custom 1`, `custom 2`, `custom 3` | `my player` |
| **Shape**        | Whether the volume is a box or a ball. | `box`, `sphere`                                                                             | `box`       |

`my player` reacts only to the avatar of the player running the scene. `all players` reacts to any avatar, including other people in the scene. The remaining options match other [collision layers](/docs/creator/sdk7/3d-essentials/colliders/), so you can have the area react to physical objects, to clickable ones, or to your own custom layers.

Switching **Shape** swaps the editor placeholder between a box and a sphere, so what you see while editing is the volume you get.

### Describe what should happen

The area fires two events: **enter** and **exit**. In the **Reactions** section of the Script component there is a button for each one:

- **When a player enters…**
- **When a player leaves…**

Click one and the Creator Hub writes the opening of a sentence into the AI Assistant's message box, naming the item, for example `When a player enters "Trigger Area", `. The prompt uses the name of the item you see on the entity tree. Finish the sentence with what you want to happen and send it. The assistant writes a separate script and attaches it to the same entity, alongside the detector.

The three chips next to the buttons (**Play a sound**, **Show a message**, **Score points**) fill in a complete sentence for you, so you can send them as they are.


> [!NOTE]
> **💡 Tip**: Reactions need the [AI Assistant](/docs/creator/scene-editor/code/ai-assistant/) to be turned on. If it isn't, the Creator Hub tells you where to enable it instead of opening the chat.

