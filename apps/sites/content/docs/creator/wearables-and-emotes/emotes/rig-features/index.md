> Features about the avatar rig and downloadable file.

# Rig Features

This documentation explains the set up for Rig 1.0, its controls, and features.


> [!NOTE]
> **💡 Tip**: Install the [Decentraland Tools Blender plugin](https://extensions.blender.org/add-ons/decentraland-tools/). It includes several handy functions to help you edit and export 3D models, wearables, and emotes.


### Armature Transforms

These are the armature’s transforms in Object Mode with the controls’ setup. **Do not edit this in any way**. The rig should only be manipulated in Pose Mode. To avoid unwanted editing, the transforms have been locked in Object Mode.

_Rig 1.0 transforms._


> [!WARNING]
> ⚠️ **Warning**: **Never edit the rig in Object Mode.**


## Bone Orientation

This is the bone orientation for Rig 1.0. As it is right now, it’s not possible to mirror behavior on the shoulders, arms, hands, or fingers.

_Axes for bone orientation._

_Behavior when mirrorring poses._

### Bone Collections

To avoid any accidents and to make it easier to identify the controls, this rig is organized in bone collections that can be accessed in the _Data Properties_ tab in Blender. These collections’ visibility can be toggled on and off by clicking on the _Eye Icon._ By default, they are all visible, except for the DON'T TOUCH ones.

Armature Data Properties tab.

This is how the bones were separated into the collections:

* Global/Switch: global controls, such as the root and spine ones, as well as shoulders. Controls with any custom attributes are also in this collection.
* FK Upper: all upper body FK setup controls.
* FK Lower: all lower body FK setup controls.
* IK Upper: all upper body IK setup controls.
* IK Lower: all lower body IK setup controls.
* Fingers: controls for both hands’ fingers.
* Deformation Bones: this is where the deformation bones are stored.


> [!WARNING]
> **⚠️ Attention!**
> 
> The DON'T TOUCH collections hold the set ups for IK and other rig constraints and should remain hidden. Editing these bones could break the functionality of the rig.


## Controls and Grouping

Controls are non-deforming bones that drive the base skeleton. They have different colors depending on their category:

* Yellow: global controls and controls with custom attributes
* Green: hip (easier to identify in the spine hierarchy)
* Blue: controls with FK behavior
* Red: IK controls
* Pink: left side controls
* Orange: right side controls

_All the controls and their colors._

## Custom Attributes and Setup

### FK/IK Blend

Even though arms are usually set as FK and legs as IK, there are certain situations that will require a different setup. If the hand has to maintain a certain position, like during push ups or while climbing, the IK will be the best choice. As for the legs, while in the air, swimming or rolling, FK works best. For more flexibility and freedom in animation, this rig has an FK/IK blend in the UpperBody control, being 0 completely FK and 1 completely IK. Any other value in-between will be a blend of the two.

![FK/IK blend for both arms and legs.](/docs/_remote/efb0c0633809e34cc3499adb.webp)

_FK/IK blend for both arms and legs._

![How the FK > IK Switch works.](/docs/_remote/b7bd249527c142b1c626e967.webp)

_How the FK > IK Switch works._

### Isolate Rotation FK Blend

Another custom attribute in the UpperBody control is the isolate rotation, that allows you to choose if the bone will inherit its parent’s rotation or not (while in FK). While at 0, the bone won’t inherit the rotation, while at 1 it will completely follow the parent’s behavior. Any other value in between will be a blend of the two. This is an interesting tool because it causes the FK bone to maintain its position, behaving a little like an IK.

![Isolate rotation attribute for arms.](/docs/_remote/627c80a7f1680d2c4d02c6d4.webp)

_Isolate rotation attribute for arms._

![How the IsoRot attribute for the arms works.](/docs/_remote/517ca0821f2dd011b2c186f1.webp)

_How the IsoRot attribute for the arms works._

The Head control also has this attribute. It’s really helpful for walk cycles, for example, since the head will keep its rotation even though the torso is twisting, making sure it’s always looking forward. Without this option, the animator would have to manually rotate the head every time the torso twists in order for it to be straight and look forward.

![Isolate rotation attribute for the head.](/docs/_remote/4afc891b59733db2550998de.webp)

_Isolate rotation attribute for the head._

![How the IsoRot attribute for the head works.](/docs/_remote/4c87d322918f5fad671253d0.webp)

_How the IsoRot attribute for the head works._


> [!WARNING]
> ⚠️ **Warning**: In older Blender versions, even if all controls have been selected and key framed, these custom attributes won't be automatically key framed. Make sure to manually insert a keyframe in each attribute so you don't lose the pose/motion you created. In Blender 4.4, by pressing I, a keyframe is set on all attributes and custom properties.


![In previous versions of Blender, make sure to keyframe all the controls and custom attributes!](/docs/_remote/a66e6f888ab4d3128c2395ee.webp)

_In previous versions of Blender, make sure to keyframe all the controls and custom attributes!_

_In Blender 4.4, press I to automatically set a keyframe on Location, Rotation, Scale & Custom Properties.._

Another solution for keyframing custom properties is selecting _**Keying**_ under on the Timeline tab and on _**Active Keying Set**_ select Location, Rotation, Scale & Custom Properties, like shown on the gif below. That way, everytime you press I, a keyframe will be created without the pop-up menu. Since some animators prefer the menu, by default, that option is not enabled. But feel free to choose the method that suits you best.

![Keyframing with the Keying option.](/docs/_remote/296a3a5045569233102e0836.webp)

_Keyframing with the Keying option._

### Reverse IK Foot Setup

Animating an FK foot is pretty straightforward: just grab any of the controls and rotate it. Since there’s a control for the foot and another for the toes, the animator has full control over the movements. However, for the IK it’s not so simple. The foot has to stick to the ground, while also being able to rotate on the ball and heels and side to side.

This rig was set up in a way to give the animator freedom of foot movement without losing the advantages of the IK system. It consists of four controls:

* Foot roll: this control rotates the foot back and forth and side to side. To avoid bending too much on the heel or too much on the ball, a limit was set so the foot rig doesn’t break. When it reaches this limit, the foot will stop rotating.

![Foot roll: rotate in X and it moves back and forth; rotate in Z it moves side to side.](/docs/_remote/28fab31b129fde23a5c5f90d.webp)

_Foot roll: rotate in X and it moves back and forth; rotate in Z it moves side to side._

* Toe tip roll: rotates the foot from the tip of the toes. It only rotates forward.

![The toe tip roll only rotates in positive X.](/docs/_remote/68e7725b2019cbf56dd0f028.webp)

_The toe tip roll only rotates in positive X._

* Toes control: rotates the toes from the ball.

![Toes can be rotated in any direction.](/docs/_remote/9c173333785f1d51960159db.webp)

_Toes can be rotated in any direction._

* Foot control: this is a global control that moves the foot as a whole. Since it’s the parent of all the other foot controls, it’ll keep any transforms while also being able to be grabbed and rotated.

![How the foot control works.](/docs/_remote/aa913f0950d1fe1e5d55b393.webp)

_How the foot control works._

### Locked Transforms

Some controls may have a lock symbol next to the transforms parameters, which means that those values can’t be changed. This was done to controls that should only behave in a certain way and to avoid any unwanted transformation. For example, elbows and knees are meant to rotate on just one axis, which in this case is X, and so the other axes have been locked. Other examples of controls with locked attributes are IK elbows and knees, fingers, foot roll, and toe tip.

It is advised to keep these locked, but in case you want more freedom of movement, just click on the lock icon to unlock it.

![Locked transforms in a control.](/docs/_remote/52bbc9e002370ab201046aeb.webp)
