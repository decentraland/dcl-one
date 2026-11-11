> 关于头像骨架和可下载文件的特性。

# 骨架特性

本文档解释了 Rig 1.0 的设置、其控制项和功能。


> [!NOTE]
> **💡 提示**: 安装 [Decentraland Tools Blender 插件](https://extensions.blender.org/add-ons/decentraland-tools/). 它包含多个实用功能，可帮助你编辑和导出 3D 模型、可穿戴物品和表情动作。


### 骨架变换

这些是对象模式下骨架的变换，以及控制项的设置。 **不要以任何方式编辑此项**。这个骨架只能在姿势模式下操作。为避免意外编辑，已在对象模式下锁定这些变换。

*Rig 1.0 的变换。*


> [!WARNING]
> ⚠️ **警告**: **绝不要在对象模式下编辑骨架。**


## 骨骼朝向

这是 Rig 1.0 的骨骼朝向。

*骨骼朝向的轴。*

*镜像姿势时的行为。*

### 镜像姿势

这个骨架的左侧和右侧骨骼布局方式不同，因此一种镜像技巧并不适用于整个身体。先从 **姿势 > 复制姿势**开始，然后 **粘贴 X 翻转**，最后再手动完成上半身。

**腿和脚会在粘贴后再处理。** **粘贴 X 翻转** 能在以下对象上得到正确结果： `CTRL_FK_Avatar_UpLeg`, `CTRL_FK_Avatar_Leg`, `CTRL_FK_Avatar_Foot`, `CTRL_FK_Avatar_ToeBase`, `CTRL_IK_Foot`, `CTRL_IK_Avatar_ToeBase` 和 `CTRL_Avatar_Knee`。对于 IK 腿来说结果是完全准确的；对于 FK 腿来说，脚部会有大约 3 厘米的偏差，因为左右静止骨骼的滚转略有不同。如果需要完全精确，就凭视觉稍微调整 FK 脚部即可。

**肩膀、手臂、手和手指还需要再多一步。** **粘贴 X 翻转** 单独使用会让另一只手偏离你想要的位置 40 到 70 厘米。粘贴后，请逐个处理这些控制项，并 **将 X 和 Z 值取反，保留 Y**:

* `CTRL_Avatar_Shoulder`
* `CTRL_FK_Avatar_Arm`, `CTRL_FK_Avatar_ForeArm`, `CTRL_FK_Avatar_Hand`
* `CTRL_IK_Avatar_Hand`, `CTRL_IK_Avatar_Elbow`
* 全部 30 个 `CTRL_Avatar_Hand*` 手指控制项

无论你使用四元数还是欧拉角，都将其应用到旋转上。两种情况下符号规则都相同。对于 IK 手部和肘部控制项，也要将其应用到位置上。

例如，粘贴后的手臂若显示为 X -0.5、Z 0.1，则变为 X 0.5、Z -0.1。

这样可以让整条手臂链完全正确。

**为什么上半身不同：** 右腿的骨骼帧与左腿的是完全镜像，这也是 **粘贴 X 翻转** 所假设的情况。右臂的帧是左臂的镜像，但局部 **Z** 轴翻转了而不是 X，因此翻转粘贴后会以错误的方向旋转。将 X 和 Z 取反可以准确修正这一点。

#### 改为直接设置数值

如果你不是通过粘贴，而是直接根据左侧原始数值来设置右侧数值，请使用以下符号翻转：

| 控制项 | 旋转 `(w, x, y, z)` | 位置           |
| --- | ----------------- | ------------ |
| 上半身 | `(w, -x, -y, z)`  | `(x, y, -z)` |
| 腿部  | `(w, x, -y, -z)`  | `(-x, y, z)` |

这是近似方式：手臂误差大约在 1 毫米以内，但 FK 腿会偏差 5 到 10 厘米。 **粘贴 X 翻转** 也会修正左右静止骨骼之间很小的滚转差异，而直接翻转符号则不会。优先使用上面的“先粘贴再修正”方法。


> [!WARNING]
> **📔 注**：起始姿势本来就不是完全对称的，所以把姿势从一侧镜像到另一侧会覆盖掉那一点小的不对称。将结果设置关键帧之前，先用眼睛检查一下。


### 旋转手臂

上臂控制项 `CTRL_FK_Avatar_Arm`具有倾斜的局部 X 轴。围绕它旋转会让手臂扫过胸前，这也是手臂本应抬起却最终交叉在一起的最常见原因。

* 要 **指向** 手臂，请改为围绕世界坐标轴旋转：世界 X 会将其向前抬起，世界 Z 会让其向内或向外摆动。
* 要 **扭转** 手臂，使手掌朝上或朝下、肘部向内或向外，请围绕骨骼自身的局部 Y 轴旋转。

### 每个关节在哪个轴上弯曲

肘部和膝盖围绕各自的局部 X 轴弯曲。手腕和手指则不是：它们围绕局部 Z 轴卷曲。围绕手指的局部 X 轴旋转会让它向侧面摆动，这也是最终表情动作里手指折错方向的最常见原因。

| Control                                               | 让它沿着           | 其他轴                      |
| ----------------------------------------------------- | -------------- | ------------------------ |
| 肘部 `CTRL_FK_Avatar_ForeArm`、膝盖 `CTRL_FK_Avatar_Leg`   | 局部 X           | Y 和 Z 已锁定                |
| 手腕 `CTRL_FK_Avatar_Hand`                              | 局部 Z 使手朝手心方向弯曲 | 局部 X 让它向侧面倾斜             |
| 手指 `CTRL_Avatar_Hand{Index,Middle,Ring,Pinky}{1,2,3}` | 局部 Z 使手指朝手心卷曲  | 局部 X 让它在手掌平面内向侧面摆动。Y 已锁定 |
| 拇指根部 `CTRL_Avatar_HandThumb1`                         | 局部 X 让拇指横跨手掌摆动 | Y 已锁定                    |
| 拇指 `CTRL_Avatar_HandThumb2`, `CTRL_Avatar_HandThumb3` | 局部 Z 使拇指卷曲     | Y 已锁定                    |

卷曲在双手上的方向相同。肘部弯曲、手腕倾斜以及拇指横跨手掌的摆动在左右两侧的方向相反，因此在一侧有效的数值，在另一侧需要翻转符号。

### 骨骼集合

为了避免意外，并让识别控制项更容易，这个骨架被组织在骨骼集合中，可在 Blender 的 *数据属性* 选项卡中访问。这些集合的可见性可通过点击 *眼睛图标* 来切换。

骨架数据属性选项卡。

骨骼是这样被分配到各个集合中的：

* Global/Switch：全局控制项，例如根部和脊柱控制项，以及肩膀。带有任何自定义属性的控制项也在这个集合中。
* FK Upper：所有上半身 FK 设置控制项。
* FK Lower：所有下半身 FK 设置控制项。
* IK Upper：所有上半身 IK 设置控制项。
* IK Lower：所有下半身 IK 设置控制项。
* Fingers：双手手指的控制项。
* Deformation Bones：变形骨骼存放在这里。


> [!WARNING]
> **⚠️ 注意！**
> 
> DON'T TOUCH 集合保存着 IK 和其他骨架约束的设置，应保持隐藏。编辑这些骨骼可能会破坏骨架功能。


## 控制项与分组

控制项是不发生变形的骨骼，用来驱动基础骨架。它们根据类别具有不同的颜色：

* 黄色：全局控制项以及带有自定义属性的控制项
* 绿色：髋部（在脊柱层级中更容易识别）
* 蓝色：具有 FK 行为的控制项
* 红色：IK 控制项
* 粉色：左侧控制项
* 橙色：右侧控制项

*所有控制项及其颜色。*

## 自定义属性与设置

### FK/IK 混合

虽然手臂通常设为 FK、腿部通常设为 IK，但某些情况下需要不同的设置。如果手部必须保持某个位置，比如做俯卧撑或攀爬时，IK 是最佳选择。至于腿部，在空中、游泳或翻滚时，FK 最合适。为了在动画中获得更大的灵活性和自由度，这个骨架在 UpperBody 控制项中提供了 FK/IK 混合，数值为 0 时完全 FK，数值为 1 时完全 IK。中间的任何其他值都会是两者的混合。

![双臂和双腿的 FK/IK 混合。](/docs/_remote/efb0c0633809e34cc3499adb.webp)

*双臂和双腿的 FK/IK 混合。*

![FK > IK 切换如何工作。](/docs/_remote/b7bd249527c142b1c626e967.webp)

*FK > IK 切换如何工作。*

一条简单规则适用于大多数情况：如果肢体保持与某物接触，就保持 IK；如果失去接触并自由摆动，就切换为 FK。对于坐姿：

* **双脚踩地**：保持腿部为 IK。用 `CTRL_IK_Foot`移动脚部，然后降低 `CTRL_Avatar_UpperBody` 来放下身体。脚会保持在地面上。用 `CTRL_Avatar_Knee`.
* **一条腿搭在另一条腿上**：只把抬起的那条腿切换为 FK。
* **双脚悬空**：把两条腿都切换为 FK。

使用 `CTRL_Avatar_UpperBody` 来上下移动整个身体。它是重心所在。 `CTRL_Avatar_Hips` 只移动骨盆和腿部，这会拉伸躯干，所以它不是坐下时该用的控制项。


> [!WARNING]
> **📔 注**：避免在片段中间将某个肢体在 FK 和 IK 之间切换。除非两条链在那一帧完全对齐，否则肢体会突然跳变。如果你确实需要切换，请将 FK > IK 属性本身设置关键帧，这样变化就会出现在你想要的位置。


#### 用 IK 触碰脸部或目标

要让手触碰某物，比如脸、道具或另一只手，请把那只手臂切换为 IK。旋转 FK 关节无法可靠地把指尖落在目标上。请记住，IK 手部控制项位于手腕处，因此把控制项放到目标上会让手腕到那里，而不是指尖。移动控制项，直到指尖落在你想要的位置。

FK > IK 属性会在关键帧之间逐渐混合。在切换它的关键帧上，将 FK 手臂摆到接近 IK 姿势的位置。否则，挂在一边的 FK 手臂在混合过程中会穿过胸前摆过去。

使用默认角色头部的这些测量值（在角色处于待机姿势时测得）将手放到脸上：

| 标志点                 | 离地高度                 | 相对于 `Avatar_Head` 骨骼的位置           |
| ------------------- | -------------------- | --------------------------------- |
| `Avatar_Head` 骨骼的位置 | 1.72 米               | 这是脸部中间，不是颅骨底部                     |
| 鼻尖                  | 1.66 米（男性身体为 1.64 米） | 向前 0.19 米，向下 0.06 米（男性身体为 0.08 米） |
| 下巴                  | 1.60 米               | 向下 0.12 米                         |
| 头顶                  | 1.90 米               | 向上 0.18 米                         |

头部宽约 0.30 米。将手定位在高出头骨 0.12 米的位置时，会落在额头上。如果头部已旋转，请在头部旋转后的坐标系中应用这些偏移。

### 隔离旋转 FK 混合

UpperBody 控制项中的另一个自定义属性是隔离旋转，它允许你选择骨骼是否继承父级的旋转（在 FK 模式下）。数值为 0 时，骨骼不会继承旋转；数值为 1 时，则会完全遵循父级的行为。中间的任何其他值都会是两者的混合。这是一个很有趣的工具，因为它会让 FK 骨骼保持其位置，表现得有点像 IK。

![手臂的隔离旋转属性。](/docs/_remote/627c80a7f1680d2c4d02c6d4.webp)

*手臂的隔离旋转属性。*

![手臂的 IsoRot 属性如何工作。](/docs/_remote/517ca0821f2dd011b2c186f1.webp)

*手臂的 IsoRot 属性如何工作。*

Head 控制项也有这个属性。例如在走路循环中它非常有用，因为即使躯干在扭转，头部也会保持其旋转，从而确保它始终面向前方。如果没有这个选项，动画师就必须在躯干每次扭转时手动旋转头部，才能让它保持笔直并朝前看。

![头部的隔离旋转属性。](/docs/_remote/4afc891b59733db2550998de.webp)

*头部的隔离旋转属性。*

![头部的 IsoRot 属性如何工作。](/docs/_remote/4c87d322918f5fad671253d0.webp)

*头部的 IsoRot 属性如何工作。*


> [!WARNING]
> ⚠️ **警告**：在较旧版本的 Blender 中，即使所有控制项都已被选中并设置了关键帧，这些自定义属性也不会自动设置关键帧。请务必在每个属性上手动插入关键帧，以免丢失你创建的姿势/动作。在 Blender 4.4 中，按 I 会为所有属性和自定义属性设置关键帧。


![在之前版本的 Blender 中，请确保为所有控制项和自定义属性都设置关键帧！](/docs/_remote/a66e6f888ab4d3128c2395ee.webp)

*在之前版本的 Blender 中，请确保为所有控制项和自定义属性都设置关键帧！*

*在 Blender 4.4 中，按 I 可自动为位置、旋转、缩放和自定义属性设置关键帧。*

为自定义属性设置关键帧的另一种方法是选择 ***关键帧设置*** 在时间轴选项卡下，并在 ***活动关键帧集*** 中选择 Location、Rotation、Scale & Custom Properties，如下方动图所示。这样，每次按 I 时都会创建关键帧，而不会弹出菜单。由于有些动画师更喜欢使用菜单，因此默认情况下该选项未启用。但请随意选择最适合你的方法。

![使用关键帧设置选项进行关键帧制作。](/docs/_remote/296a3a5045569233102e0836.webp)

*使用关键帧设置选项进行关键帧制作。*

### 反向 IK 脚部设置

制作 FK 脚部动画非常直接：只需抓住任一控制项并旋转即可。由于脚部和脚趾各有一个控制项，动画师可以完全控制动作。然而，IK 就没那么简单了。脚必须贴在地面上，同时还要能够围绕前脚掌、脚跟以及左右方向旋转。

这个骨架的设置方式是在不失去 IK 系统优势的前提下，给予动画师脚部移动的自由。它由四个控制项组成：

* 脚部滚转：这个控制项可让脚前后和左右旋转。为避免在脚跟或前脚掌上弯曲过多，设置了限制，以免脚部骨架被破坏。当达到该限制时，脚将停止旋转。

![脚部滚转：在 X 轴上旋转会前后移动；在 Z 轴上旋转会左右移动。](/docs/_remote/28fab31b129fde23a5c5f90d.webp)

*脚部滚转：在 X 轴上旋转会前后移动；在 Z 轴上旋转会左右移动。*

* 脚尖滚转：从脚趾尖部旋转脚部。它只会向前旋转。

![脚尖滚转只会在正 X 方向旋转。](/docs/_remote/68e7725b2019cbf56dd0f028.webp)

*脚尖滚转只会在正 X 方向旋转。*

* 脚趾控制项：从前脚掌处旋转脚趾。

![脚趾可以向任意方向旋转。](/docs/_remote/9c173333785f1d51960159db.webp)

*脚趾可以向任意方向旋转。*

* 脚部控制项：这是一个全局控制项，负责整体移动脚部。由于它是所有其他脚部控制项的父级，因此它会保留任何变换，同时也可以被抓取并旋转。

![脚部控制项如何工作。](/docs/_remote/aa913f0950d1fe1e5d55b393.webp)

*脚部控制项如何工作。*

### 锁定的变换

某些控制项的变换参数旁可能会出现锁符号，这表示这些数值不能更改。这样做是针对那些只能以特定方式运行、并且要避免任何不必要变换的控制项。比如，肘部和膝盖应该只围绕一个轴旋转，在这里就是 X 轴，因此其他轴已被锁定。其他带锁定属性的控制项示例包括 IK 肘部和膝盖、手指、脚部滚转以及脚尖滚转。

建议保持这些锁定状态，但如果你想要更大的移动自由度，只需点击锁图标将其解锁。

锁定只适用于 Blender 的交互式工具。直接写入骨骼旋转的 Python 脚本会忽略这些锁，因此如果你用脚本摆姿势，请自行保持每个旋转都落在关节的自由轴上。

![控制项中的锁定变换。](/docs/_remote/52bbc9e002370ab201046aeb.webp)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/biao-qing-dong-zuo/rig-features.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
