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

This is the bone orientation for Rig 1.0.

_Axes for bone orientation._

_Behavior when mirroring poses._

### Mirroring a pose

The rig's left and right bones are not laid out the same way, so a single mirroring trick does not work for the whole body. Start with **Pose > Copy Pose**, then **Paste X-Flipped**, and finish the upper body by hand.

**Legs and feet are done after the paste.** **Paste X-Flipped** gives the right result on `CTRL_FK_Avatar_UpLeg`, `CTRL_FK_Avatar_Leg`, `CTRL_FK_Avatar_Foot`, `CTRL_FK_Avatar_ToeBase`, `CTRL_IK_Foot`, `CTRL_IK_Avatar_ToeBase` and `CTRL_Avatar_Knee`. It is exact for IK legs, and lands within about 3 cm at the foot for FK legs, because the left and right rest bones have slightly different rolls. Nudge the FK foot by eye if you need it exact.

**Shoulders, arms, hands and fingers need one more step.** **Paste X-Flipped** alone puts the other hand 40 to 70 cm away from where you want it. After pasting, go through each of these controls and **negate the X and Z values, keeping Y**:

* `CTRL_Avatar_Shoulder`
* `CTRL_FK_Avatar_Arm`, `CTRL_FK_Avatar_ForeArm`, `CTRL_FK_Avatar_Hand`
* `CTRL_IK_Avatar_Hand`, `CTRL_IK_Avatar_Elbow`
* All 30 `CTRL_Avatar_Hand*` finger controls

Apply it to the rotation, whether you work in quaternions or Euler angles. The signs are the same either way. On the IK hand and elbow controls, apply it to the location too.

For example, a pasted arm reading X -0.5, Z 0.1 becomes X 0.5, Z -0.1.

This gets the whole arm chain exactly right.

**Why the upper body is different:** the right leg's bone frames are a plain mirror of the left ones, which is what **Paste X-Flipped** assumes. The right arm's frames mirror the left with the local **Z** axis flipped instead of X, so the flipped paste comes out rotated the wrong way. Negating X and Z corrects exactly that.

#### Setting the values directly instead

If you set the right-side values from the raw left-side values rather than pasting, use these sign flips:

| Controls | Rotation `(w, x, y, z)` | Location |
| --- | --- | --- |
| Upper body | `(w, -x, -y, z)` | `(x, y, -z)` |
| Legs | `(w, x, -y, -z)` | `(-x, y, z)` |

This is the approximate route: within about 1 mm for the arms, but 5 to 10 cm off for FK legs. **Paste X-Flipped** also corrects the small roll differences between the left and right rest bones, and a raw sign flip doesn't. Prefer the paste-then-fix method above.


> [!WARNING]
> **📔 Note**: The starting pose is not perfectly symmetric to begin with, so mirroring a pose from one side to the other overwrites that small asymmetry. Check the result by eye before keyframing it.


### Rotating arms

The upper arm control, `CTRL_FK_Avatar_Arm`, has a tilted local X axis. Rotating around it sweeps the arm across the chest, which is the most common reason arms end up crossed when you meant to raise them.

* To **aim** the arm, rotate around the world axes instead: world X raises it forward, world Z swings it in and out.
* To **twist** the arm, so the palm turns up or down and the elbow points in or out, rotate around the bone's own local Y.

### Which axis each joint bends on

Elbows and knees bend on their local X. Wrists and fingers don't: they curl on their local Z. Rotating a finger around its local X swings it sideways, which is the most common reason fingers fold the wrong way in a finished emote.

| Control | Bend it on | Other axes |
| --- | --- | --- |
| Elbow `CTRL_FK_Avatar_ForeArm`, knee `CTRL_FK_Avatar_Leg` | Local X | Y and Z are locked |
| Wrist `CTRL_FK_Avatar_Hand` | Local Z flexes the hand toward the palm | Local X tilts it sideways |
| Fingers `CTRL_Avatar_Hand{Index,Middle,Ring,Pinky}{1,2,3}` | Local Z curls the finger toward the palm | Local X swings it sideways in the plane of the palm. Y is locked |
| Thumb base `CTRL_Avatar_HandThumb1` | Local X swings the thumb across the palm | Y is locked |
| Thumb `CTRL_Avatar_HandThumb2`, `CTRL_Avatar_HandThumb3` | Local Z curls the thumb | Y is locked |

Curls go the same way on both hands. The elbow bend, the wrist tilt and the thumb's swing across the palm go the opposite way on the left and right sides, so a value that works on one side needs its sign flipped on the other.

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

A simple rule covers most cases: if a limb keeps contact with something, keep it on IK; if it loses contact and swings freely, switch it to FK. For a seated pose:

* **Both feet on the ground**: keep the legs on IK. Move the feet with `CTRL_IK_Foot`, then lower `CTRL_Avatar_UpperBody` to drop the body. The feet stay planted. Aim the knees with `CTRL_Avatar_Knee`.
* **One leg crossed over the other**: switch only the lifted leg to FK.
* **Feet dangling**: switch both legs to FK.

Use `CTRL_Avatar_UpperBody` to move the whole body up and down. It is the center of gravity. `CTRL_Avatar_Hips` moves only the pelvis and the legs, which stretches the torso, so it is the wrong control for sitting down.


> [!WARNING]
> **📔 Note**: Avoid switching a limb between FK and IK in the middle of a clip. Unless the two chains line up exactly on that frame, the limb pops. If you do need to switch, keyframe the FK > IK property itself so the change happens where you want it.


#### Touch the face or a target with IK

To make a hand touch something, like the face, a prop or the other hand, switch that arm to IK. Rotating FK joints can't land a fingertip on a target reliably. Keep in mind that the IK hand control sits at the wrist, so placing the control on the target puts the wrist there, not the fingertip. Move the control until the fingertip lands where you want.

The FK > IK property blends gradually between its keyframes. On the keyframes where you switch it, pose the FK arm close to the IK pose. Otherwise an FK arm left hanging at the side swings through the chest while the blend runs.

Use these measurements of the default avatar's head, taken with the avatar standing in its idle pose, to place a hand on the face:

| Landmark | Height above the floor | Position relative to the `Avatar_Head` bone |
| --- | --- | --- |
| `Avatar_Head` bone | 1.72 m | This is the middle of the face, not the base of the skull |
| Tip of the nose | 1.66 m (1.64 m on the male body) | 0.19 m forward and 0.06 m down (0.08 m on the male body) |
| Chin | 1.60 m | 0.12 m down |
| Top of the head | 1.90 m | 0.18 m up |

The head is about 0.30 m wide. A hand aimed 0.12 m above the head bone lands on the forehead. If the head is rotated, apply these offsets in the head's rotated frame.

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

Locks only apply to Blender's interactive tools. A Python script that writes a bone's rotation directly ignores them, so if you pose the rig with a script, keep each rotation on the joint's free axes yourself.

![Locked transforms in a control.](/docs/_remote/52bbc9e002370ab201046aeb.webp)
