> Configure any item to behave like a smart item.

# Make Any Item Smart



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/wnnEU8GCLjc" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="https://www.youtube.com/watch?v=wnnEU8GCLjc">Watch on YouTube</a></p>



Smart items are just regular items with an **Action** and/or **Trigger** component. You can add these components to any item in your scene. You can also import your own custom 3D models and add the same to those.

To add components to an item click the **Plus Icon** next to the item name, and select what component to add from the dropdown list.

![The add component menu open on an entity, listing components grouped under 3D Content and Interaction, including Actions and Triggers.](/docs/creator/images/editor/add-component.webp)

This allows for a huge amount of creative possibilities. Turn a candle into a lever that opens up a secret passage behind a book shelf, play mysterious sounds from inside a well, make diamonds into collectable items that shrink to 0 when clicked. There are tons of imaginative ways to combine these mechanics!

## Interactivity

You can make an item react to different actions of the player.


> [!NOTE]
> **💡 Tip**: When a player interacts with an item, you should show some kind of feedback to make that interaction clear. If the model doesn't have any animations, consider at least playing a sound. In some cases it might work to make the item do a slight tween in scale and then return to its original scale, as a form of feedback.


Add a **Trigger** component, to detect to different actions from the player:

* **Pointer events**: When the player clicks or presses a key while aiming their cursor at the item.
* **Global button events** When the player presses a key, wherever they are in the scene.
* **Player proximity**: When the player walks into the item's position.

The **Trigger** component can be configured to be aware of any of these types of triggers. Every time a trigger happens, it can call Actions from their own **Actions** component, or from the **Actions** components of other items in the scene. See [Smart items - Advanced](/docs/creator/scene-editor/interactivity/smart-items-advanced/).


> [!NOTE]
> **💡 Tip**: You can also combine these triggers with [conditional logic](/docs/creator/scene-editor/interactivity/states-and-conditions/), so that the actions don't get called every time the trigger happens, only if the conditions are true.
> 
> For example, you could add a **Pointer Events** trigger to a door, so that it opens when clicked, but include conditional logic so that it only opens if it's unlocked.


### Pointer events

Add a **Trigger** component with **On Click** or **On Input Action** Trigger events.

* **On Click** reacts to every time the player clicks the left-mouse button while pointing at the item.
* **On Input Action** reacts to every time the player presses the Primary Button (E) while pointing at the item.

![](/docs/creator/images/editor/on_click.webp)

**Colliders**

It's important that for an item to be clickable, it must have a **Collider**. Otherwise your clicks will go right through the model, and try to interact with whatever is behind. The 3D models in the default Asset Packs should all have colliders, but if you create your own model or source if from elsewhere, you may find it's missing one.

If your model is lacking colliders, any of the following should fix it:

* Add a **Mesh Collider** component. This will create a collider with a [primitive shape](/docs/creator/sdk7/3d-essentials/shape-components/#primitive-shapes) (cube, plane, cylinder, sphere).
* Change the properties of the **Collisions** section on the **GLTF** component. The **Visible layer** should be assigned to **Pointer**.
* Edit the 3D model in Blender to include an invisible collider geometry (any mesh with a name that ends in `_collider`). See [colliders](/docs/creator/3d-modeling/colliders/).


> [!NOTE]
> **💡 Tip**: If you used the **Mesh Renderer** component to give your model a primitive shape, that alone won't give it a collider. You must also assign it a **Mesh Collider** component.


**Customize pointer events**

You can override the default settings that are used when an item has an **On Click** or an **On Input Action** Trigger Action.

* **Hover text**: Change the hint that players see next to the cursor when hovering over the item. This can be very helpful for clarifying what your item does.
* **Max distance**: How far away can the player be when interacting with your item.
* **Show feedback**: If unchecked, the item has no hover-hint when the player passes their cursor on it.
* **Button**: If using the **On Input Action** Trigger Action, you can reassign the default **Primary (E)** to another key. The hover-hint will include an icon to clarify what key to use. You can use **Secondary (F)**, or **Actions 3 to 6** (number keys 1 to 4).

### Global button events

Add a **Trigger** component with **On Global Click**, **On Global Primary** or **On Global Secondary** Triggers events.

* **On Global Click** reacts to every time the player clicks the left-mouse button, anywhere in the scene.
* **On Global Primary** reacts to every time the player presses the Primary Button (E), anywhere in the scene.
* **On Global Secondary** reacts to every time the player presses the Secondary Button (F), anywhere in the scene.


> [!NOTE]
> **💡 Tip**: It often makes sense to combine this with [States and conditions](/docs/creator/scene-editor/interactivity/states-and-conditions/), so that the items only react to the button event if the player is in the room, or some other condition.


### Player position

Add a **Trigger** component with **Player Enters Area**, **Player Leaves Area** Triggers events.

This will react to when the player enters or leaves an area of a default size of 1x1x1, positioned at the center of the item.


> [!NOTE]
> **💡 Tip**: It's often better to use the [**Trigger Area**](/docs/creator/scene-editor/interactivity/smart-items/#trigger-areas) smart item instead, since this item's dimensions can be clearly visualized in the Scene Editor.


## See also

* [Smart items - Basics](/docs/creator/scene-editor/interactivity/smart-items/)
* [Smart items - Advanced](/docs/creator/scene-editor/interactivity/smart-items-advanced/)
* [States and conditions](/docs/creator/scene-editor/interactivity/states-and-conditions/)
* [Combine with code](/docs/creator/scene-editor/code/overview/)
