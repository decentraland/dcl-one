> Guidelines to add props and sounds to the emotes.

# Props and Sounds

In order to take your Decentraland Emotes to the next level you can add props (3d geometry) and/or sounds to them, doing the emotes much more fun and engaging! In this guideline you will find everything you need to know to export them correctly!


> [!NOTE]
> **💡 Tip**: Install the [Decentraland Tools Blender plugin](https://extensions.blender.org/add-ons/decentraland-tools/). It includes several handy functions to help you edit and export 3D models, wearables, and emotes.


## **The Basics and Limitations**

To start adding the props to your emotes it's important to use the [Decentraland Template File](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/BaseMale_Rig_1.0.blend) which will have the rig for the avatar and also the Ground Reference to keep your work inside the allowed space boundaries.

**Currently, the props animations only work with Armature/Bones Animations meaning that \_transform animations**\_\*\* are not allowed.\*\*

The emote with their props must be exported all together in one single _.glb_ file (Avatar_Armature + Props_Armature with its animations).

- No more than 3 MB in total.
- No more than 3k tris for props in total.
- No more than 2 materials and 2 textures for props.
- No more than 62 bones for the prop armature.
- The emote must have one animation for the avatar and one animation for the prop. _Currently multiple animations are not allowed._
- Both animations (Avatar and Prop) must have the same keyframe length.
- Animations cannot exceed 300 frames or 10 seconds.
- Space boundaries are 4 square meters. Props and particles should stay within the reference cube provided in the Avatar File. For avatar movement, check [Ground Reference and Animation Area](/docs/creator/wearables-and-emotes/emotes/creating-emotes/#ground-reference-and-animation-area). section.

## **Naming Conventions:**

Naming conventions must be strictly followed for the emotes to work! Otherwise they will not play correctly neither in the builder nor in world.

#### Armatures Name Conventions:

**For Avatar:**

`Armature`

**For Props:**

`Armature_Prop`

#### Animations Name Conventions:

**For Avatar:**

`AnimationName_Avatar`

- Example: `TennisServe_Avatar`, `GunShoot_Avatar`

**For Props:**

`AnimationName_Prop`

- Example: `TennisServe_Prop`, `GunShoot_Prop`

## **Getting Started**

Before starting you animation, you will have to create a rig for the prop. If you’re not familiar with the process, check [Create a Rig](/docs/creator/3d-modeling/create-a-rig/) for more information on how to do it.

Ensure that the prop object and armature have their origins located at the 0,0 location within Blender. Additionally, apply transformations to the prop object and armature, ensuring they are frozen at a scale of 1,1,1. This is crucial to prevent any potential issues with the prop's behavior when being utilized within the world or during animations.

#### Making the Prop Follow the Avatar Rig

Some props might have to be attached to certain body parts, like a tennis racket to the hand. That can be done by simply adding a constraint. To do so, in _**Pose Mode**_, select the prop bone (the tennis racket one, for example), press _**CTRL + Shift + C**_ on your keyborad and select _**Child of**_ or just click on the _**Bone Constraint Properties**_ tab and, in the drop down menu, select _**Child of**_.

_Add a constraint by pressing `Ctrl + Shift + C` on your keyboard._

Then, in _**Target**_, select the avatar armature and in _**Bone**_ select the bone you want the prop to follow. To maintain the prop’s original position, click on _**Set Inverse**_ once you add the constraint. If the influence is 1, the prop will fully follow the selected bone, if it’s 0, the constraint will be disabled. You can set keyframes on the influence to turn it on and off throughout the animation. To do that, just press I while the cursor is on top of _**Influence**_.

_**Chlid of** constraint menu. Keyframe the influence to turn it on and off._


> [!NOTE]
> **💡Animation Tip!** If you use the slide to turn off the Influence, the prop will not maintain its previous position, making it hard to keep the animation fluid. To avoid having to manually fix the position, instead of using the slide, click on the X next to Influence, set a keyframe on it and another one on all the transform attributes. This way the prop will keep the same poistion as when the Influence was on!



> [!NOTE]
> **💡Animation Tip**
> 
> Don’t leave the prop visible from the start! To avoid spoiling what’s about to happen and an abrupt transition, start the animation with the prop scaled down to 0.001 and only turn it to 1 when you want it to appear. Remember to scale back down to 0 by the end of the action. This will make the transitions much more fluid and cool!


### Animation Slots

Blender 4.4 introduced a new feature: animation slots. According to Blender documentation, “the purpose of slots is to allow an action to store distinct animation data for multiple data-blocks”. In a nutshell, slots make it possible to store the animation of multiple things in the same Action. How does it affect emotes 2.0?

Blender 4.4 new feature: animation slots.

Even though it’s possible to have both the avatar and prop sharing the same action clip, because of the naming convention and number of animation clips involved in Emotes 2.0, it won’t work. So the pipeline for this would be:

1. Create an animation clip for the avatar, or rename the one provided (_**Starting_Pose**_). It already has an animation slot, but feel free to use it (_**Avatar_Animation**_) or create a new one.
2. Rename the animation clip _**AnimationName_Avatar**_
3. Create an animation clip for the prop and rename it _**AnimationName_Prop**_
4. Click on _**New**_ button to create an animation slot for it (it will receive an automatic name: _**Armature_Prop**_)
5. Animate as you would do in previous Blender versions.

Creating and action clip and a slot for the prop animation.

## **NLA Tracks**

In order for all the animations to be exported, the clips should be added to the NLA Tracks. Make sure there’s only one animation clip for the avatar and another one for the prop, **they must have the exact same number of frames.**

In _**Object Mode**_, select the avatar armature, got to _**Pose Mode**_, select the respective animation clip in the Browse Action menu, click on _**Action**_ and then the _**Push Down**_ option.

Then, change back to _**Object Mode**_, select the prop armature, go to _**Pose mode**_, select the respective animation clip in the Browse Action menu, click on _**Action**_ and then the _**Push Down**_ option.

Pushing actions down to the NLA tracks.


> [!WARNING]
> ⚠️ Be careful when pushing actions down . Make sure you select the desired armature with the respective animation. Don’t just change the animation and push it down before selecting the other armature or else you will be assigning two actions to an armature and none to the other.


The NLA tracks should look like this: one animation for each armature.


> [!NOTE]
> **🔥 Optimization Tip**
> 
> **Before this step make sure to do a backup of your project.**
> 
> If you have different objects for your props you can merge them together in one single mesh. You can do this by simply selecting the objects and pressing the shortcut ctrl + J.
> 
> This would help to reduce the draw calls in game making the emote more performant.
> 
> Keep in mind that this won’t work for particles, though.
> 
> _Select objects and press `Ctrl+J` to merge them together._


## **Exporting**

Emotes 2.0 are exported the same way as common emotes. Make sure only the avatar armature, prop armature and prop meshes are visible and hide everything else.

Have only avatar armature, prop armature and prop mesh visible for exporting.

To export, go to File > Export > glTF2.0 (.glb, .gltf)&#x20;

For the export settings, expand Include and in Limit to toggle Visible Objects. Then, expand the Data tab, expand Armature and enable Export Deformation Bones Only.

|     |     |
| --- | --- |

Hit Export and you are done!

## **Add Audio to the Emotes**

### Format and Limitations for Audio Clips

- The correct format to export sounds for your emotes are `.mp3` and `.ogg`.
- The audio clip must have the same duration as the emote.
- While there is no limitation for size in the audio, the emote with props and sounds cannot be bigger than 3mb.


> [!NOTE]
> **📔 Note**: If the emote has sound (mp3 or ogg), it must be zipped with the .glb. After that, just drag and drop the .zip to the builder. More details can be found here: [Uploading emote with sound](/docs/creator/wearables-and-emotes/manage-collections/uploading-emotes/#uploading-emotes-using-a-zip-file)



> [!NOTE]
> **💡 Attention!** Take into consideration that audio clips used in the emote must be original IP (Intellectual Property), having the rights for reproducing and follows the [Content Policy](https://decentraland.org/content/)criteria.


### Editing Sounds

To add sounds to your emotes you can do it in different ways:

1. **Edit your sounds directly on Blender**

One way to add sounds to your emotes is using the video sequencer editor that Blender provides.

To start adding sounds go to _Editor Type> Video Sequencer._

Drag and Drop you sounds to the channels interface.

Press the shortcut `N` to see more options to handle your sounds like displaying waveform, make your sounds Mono or changing the volume.


> [!NOTE]
> If you want to fade in and out you can simply do it by adding keyframes from 0 to 1 and viceversa to the volume property.


Once you finished to edit your sounds you can export it going to _Render> Render Audio_. In the exporting option you need to select `.mp3` or `.ogg` format in the _Container_ section and then _Mixdown_. **Only the audio within the frame range will be exported.**

2. **Render animation and add sound with a sound edit software**

While editing sounds directly in Blender can be convenient, it is not very flexible because the software is not primarily focused on sound editing. The available tools are very basic. If you want to add a more professional touch to your sounds, we recommend using dedicated sound editing software of your choice.

There are several software options you can use, such as [Audacity](https://www.audacityteam.org/) (Free and OpenSource), Adobe Audition, Ableton Live, or ProTools. Using dedicated sound editing software will provide you with a wider range of tools, functionalities, and sound effects, allowing you to enhance your sounds and give them a more professional feel.

To render your emote you can simply add a camera to your Blender scene and position it in a way you can see all the elements as clearly as possible to later have a good reference to add sounds.

When rendering an emote, it is important to only include the frame range of your emote and not more. Choose an aspect ratio that suits your needs and select the output folder where you want the video or image sequence to be saved.


> [!NOTE]
> **Hint!**
> 
> _Before rendering make sure you do a low sampling rendering to save time in your render!_


Once this step is completed, use your video as a reference to create the corresponding sounds using your preferred sound editing software. **Ensure that the video sequence matches the animation's framerate of 30 frames per second (fps)**
