# 材质

了解如何向具有基础形状的实体添加材质和纹理。

## 材质

可以通过添加一个 `材质` 组件，将材质应用到使用基本形状（立方体、球体、平面等）的实体上。此组件有多个字段，可让你配置材质属性、添加纹理等。

*glTF* 模型包含它们自己的材质，这些材质会在导入模型时与模型一起隐式导入到场景中。要修改或覆盖这些材质，请使用 `GltfNodeModifiers` 组件。参见 [修改 glTF 材质](#modify-gltf-materials) 了解更多详情。

导入带有自身材质的 3D 模型时，请记住，并非所有着色器都受 Decentraland 引擎支持。仅支持标准材质和 PBR（基于物理的渲染）材质。参见 [外部 3D 模型注意事项](https://github.com/decentraland/docs/blob/main/creator/3d-modeling/materials.md) 了解更多详情。

支持的材质有不同类型：

* PBR（基于物理的渲染）：Decentraland 中最常见的材质类型。它支持纯色或纹理，以及金属度、自发光、透明度等不同属性。了解更多关于 [PBR](https://en.wikipedia.org/wiki/Physically_based_rendering).
* 基础材质：它们不响应光照和阴影，因此非常适合显示公告牌图像。

## 在 Creator Hub 中使用场景编辑器

给实体添加材质最简单的方法是使用场景编辑器。你可以向你的实体添加一个 **材质** 组件，然后在场景编辑器 UI 上配置所有可用字段。参见 [添加组件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/components/#add-components).

## 添加材质

下面的示例创建了一个 PBR 材质，并设置了其中一些字段，使其呈现红色和金属属性。该材质被添加到一个同样具有盒状形状的实体上，因此它会用该材质为盒子着色。

```ts
//创建实体并分配形状
const meshEntity = engine.addEntity()
Transform.create(meshEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(meshEntity)

//创建材质并配置其字段
Material.setPbrMaterial(meshEntity, {
	albedoColor: Color4.Red(),
	metallic: 0.8,
	roughness: 0.1,
})
```

要更改一个已经有 `材质` 组件的实体的材质，请运行 `Material.setPbrMaterial()` 或任何其他辅助函数，它都会覆盖原始材质。无需移除原始 `材质` 或使用高级语法。

```ts
//创建实体并分配形状
const meshEntity = engine.addEntity()
Transform.create(meshEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(meshEntity)

//创建材质并配置其字段
Material.setPbrMaterial(meshEntity, {
	albedoColor: Color4.Red(),
})

//使用新的材质组件覆盖
Material.setPbrMaterial(meshEntity, {
	albedoColor: Color4.Blue(),
})
```


> [!WARNING]
> **📔 注意**： `材质` 组件必须通过以下方式导入
> 
> > `import { Material } from "@dcl/sdk/ecs"`
> 
> 参见 [导入](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes/#imports) 以了解如何轻松处理这些内容。


## 材质颜色

为材质赋予纯色。在 PBR 材质中，你设置 `albedoColor` 字段。反照率颜色会响应光照，并且可以包含阴影。

颜色值的类型是 `Color4`，由 *r*, *g* 和 *b* 值（红、绿、蓝）组成。这些值都介于 0 和 1 之间。通过为它们设置不同的值，你可以组合出任何可见颜色。黑色时，三个值都设为 0。白色时，全部设为 1。


> [!WARNING]
> **📔 注意**：如果你将 `albedoColor` 中的任何颜色设为高于 *1*的值，它会显示为 *自发光*，数值越高强度越大。例如， `{r: 15, g: 0, b: 0}` 会产生非常明亮的红色光晕。


参见 [颜色类型](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/color-types/) 了解如何设置颜色的更多细节。

你也可以编辑 PBR 材质中的以下字段，以微调其颜色的呈现方式：

* *emissiveColor*：材质发出的颜色。
* *reflectivityColor*：也称为 *高光颜色* ，在其他命名体系中。

要创建一个不受环境光照和阴影影响的纯色材质，请改用基础材质而不是 PBR 材质。

```ts
Material.setBasicMaterial(myEntity, {
	diffuseColor: Color4.Black(),
})
```

## 使用纹理

通过设置 `texture` 参数，将图像文件设置为材质的纹理。

```ts
//创建实体并分配形状
const meshEntity = engine.addEntity()
Transform.create(meshEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(meshEntity)

//创建材质并配置其字段
Material.setPbrMaterial(meshEntity, {
	texture: Material.Texture.Common({
		src: 'assets/materials/wood.png',
	}),
})
```

在上面的示例中，材质的图像位于一个 `assets/materials` 文件夹中，该文件夹位于场景项目文件夹的根目录。


> [!NOTE]
> **💡 提示**：我们建议将纹理图像文件放在场景内的 `/assets` 文件夹中的某个位置。


在创建纹理时，你也可以传入额外参数：

* `filterMode`：决定纹理在渲染时像素如何拉伸或压缩。该值取自 `TextureFilterMode` 枚举。参见 [纹理缩放](#texture-scaling).
* `wrapMode`：决定纹理如何平铺到对象上。该值取自 `TextureWrapMode` 枚举。参见 [纹理包裹](#texture-wrapping).

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'assets/materials/wood.png',
		filterMode: TextureFilterMode.TFM_BILINEAR,
		wrapMode: TextureWrapMode.TWM_CLAMP,
	}),
})
```

要创建一个不受环境光照和阴影影响的纹理，请改用基础材质而不是 PBR 材质。

```ts
Material.setBasicMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'assets/materials/wood.png',
	}),
})
```

### 来自外部 URL 的纹理

你可以将材质的纹理指向外部 URL，而不是场景项目中的内部路径。

```ts
Material.setBasicMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'https://wearable-api.decentraland.org/v2/collections/community_contest/wearables/cw_tuxedo_tshirt_upper_body/thumbnail',
	}),
})
```

URL 必须以 `https`, `http` 开头。URL 不受支持。

### 纹理包裹

你可以设置纹理与表面的对齐方式。默认情况下，纹理会被拉伸以一次覆盖整个表面，但你可以缩放它并调整其偏移。

所有纹理都可使用以下字段：

* `offset`：移动纹理以更改其对齐方式。该值是一个 Vector2，两个轴都从 0 到 1，其中 1 表示纹理的完整宽度或高度。
* `tiling`：缩放纹理。默认值为 Vector2 `[1, 1]`，它会让图像重复一次并覆盖整个表面。
* `TextureWrapMode`：决定如果图像平铺没有覆盖整个表面时会发生什么。该属性取值自 `TextureWrapMode` 枚举，它允许以下值：
  * `TextureWrapMode.TWM_CLAMP`：纹理仅以指定大小显示一次。网格表面的其余部分保持透明。 `tiling` 的值会被忽略。
  * `TextureWrapMode.TWM_REPEAT`：纹理会按指定大小重复尽可能多次，直到适合网格为止。
  * `TextureWrapMode.TWM_MIRROR`：与重复包裹类似，纹理会重复尽可能多次以适配，但这些重复的方向会镜像反转。

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'assets/materials/wood.png',
		wrapMode: TextureWrapMode.TWM_REPEAT,
		offset: Vector2.create(0, 0.2),
		tiling: Vector2.create(1, 1),
	}),
})
```


> [!WARNING]
> **📔 注意**： `offset` 和 `tiling` 这些属性仅在 DCL 2.0 桌面客户端中受支持。


使用此功能可以用平铺图案覆盖大面积表面。例如，重复以下图像：

![](/docs/_remote/5a88e3a6f74d3fb726a05771.webp)

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'assets/materials/wood.png',
		wrapMode: TextureWrapMode.TWM_REPEAT,
		tiling: Vector2.create(8, 8),
	}),
})
```

![](/docs/_remote/5b75756c2234feb52d56048a.webp)

在下面的示例中，纹理使用了 *镜像* 包裹模式，每次重复的纹理只占表面的 1/4。这意味着我们会看到 4 张图像副本，它们在两个轴上彼此镜像。

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'materials/atlas.png',
		wrapMode: TextureWrapMode.TWM_MIRROR,
		tiling: Vector2.create(0.25, 0.25),
	}),
})
```

### 纹理补间

使用一个 `Tween` 组件，并设置为 `TextureMove` 模式，让纹理平滑滑动。补间会以平滑且经过优化的方式，在一段时间内逐步改变 `offset` 或 `tiling` 纹理属性


> [!WARNING]
> **📔 注意**：纹理补间是仅在 DCL 2.0 桌面客户端中受支持的功能。


使用 `Tween` 组件与 `setTextureMove` 函数来在两个位置之间移动纹理。

```ts
Tween.setTextureMove(myEntity, Vector2.create(0, 0), Vector2.create(1, 0), 2000)
```

纹理补间包含以下信息：

* `实体`：要移动纹理的实体
* `start`：一个表示起始位置的 Vector2
* `end`：一个表示结束位置的 Vector2
* `duration`：在两个位置之间移动所需的毫秒数

也可使用以下其他可选参数：

* `movementType`：定义移动是在 `offset` 或 `tiling` 字段上。默认使用 `offset`.
* `easingFunction`：时间变化速率的曲线，默认值是 `EasingFunction.EF_LINEAR`。其他值会使变化以不同速率加速和/或减速。

```ts
const myEntity = engine.addEntity()

MeshRenderer.setPlane(myEntity)

Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})

Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'materials/water.png',
		wrapMode: TextureWrapMode.TWM_REPEAT,
	}),
})

Tween.setTextureMove(myEntity, Vector2.create(0, 0), Vector2.create(0, 1), 1000)
```

上面的示例会运行一个持续 1 秒、并且只移动一次纹理的补间。要实现连续移动，例如模拟瀑布下落，你需要使用 `setTextureMoveContinuous`.

```ts
const myEntity = engine.addEntity()

MeshRenderer.setPlane(myEntity)

Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})

Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'materials/water.png',
		wrapMode: TextureWrapMode.TWM_REPEAT,
	}),
})

Tween.setTextureMoveContinuous(myEntity, Vector2.create(0, 1), 1)
```

上面的示例使用了 `setTextureMoveContinuous`，方向为 `(0, 1)`，速度为每秒 1 个单位。

纹理连续补间包含以下信息：

* `实体`：要移动纹理的实体
* `direction`：表示移动的 Vector2
* `speed`：实体每秒将移动多少个单位

也可使用以下其他可选参数：

* `movementType`：定义移动是在 offset 还是 tiling 字段上。默认使用 offset。
* `duration`：维持移动的毫秒数。此时间后，移动将停止。

**复杂补间序列**

你也可以让纹理移动遵循一个包含任意多个步骤的复杂序列。使用 `sequence` 字段列出你想要的任意多个补间，它们会在第一个补间之后按顺序执行，而第一个补间在 `Tween` 组件。

```ts
//(...)
Tween.setTextureMove(myEntity, Vector2.create(0, 0), Vector2.create(0, 1), 1000)

TweenSequence.create(myEntity, {
	sequence: [
		{
			mode: Tween.Mode.TextureMove({
				start: Vector2.create(0, 1),
				end: Vector2.create(1, 1),
			}),
			duration: 1000,
			easingFunction: EasingFunction.EF_LINEAR,
		},
		{
			mode: Tween.Mode.TextureMove({
				start: Vector2.create(1, 1),
				end: Vector2.create(1, 0),
			}),
			duration: 1000,
			easingFunction: EasingFunction.EF_LINEAR,
		},
		{
			mode: Tween.Mode.TextureMove({
				start: Vector2.create(1, 0),
				end: Vector2.create(0, 0),
			}),
			duration: 1000,
			easingFunction: EasingFunction.EF_LINEAR,
		},
	],
	loop: TweenLoop.TL_RESTART,
})
```

请注意，在 TweenSequence 中定义补间时，你需要使用更详细的 `Tween.Mode.TextureMove` 格式来定义补间。

### 多层纹理

你可以使用多个图像文件作为图层来组合更逼真的纹理，例如包含一个 `bumpTexture` 和一个 `emissiveTexture`.

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'materials/wood.png',
	}),
	bumpTexture: Material.Texture.Common({
		src: 'materials/woodBump.png',
	}),
	emissiveTexture: Material.Texture.Common({
		src: 'materials/glow.png',
	}),
})
```

该 `bumpTexture` 可以通过修改表面法线在每个像素上的行为，来模拟表面的凹凸和皱纹。

![](/docs/_remote/b02dcfc858055419931f396c.webp)

该 `emissiveTexture` 可以增强材质某些部分的发光效果，以实现非常有趣的效果。

**设置 UV**

更改纹理缩放或对齐方式的另一种方法是配置 *uv* 属性在 [MeshRenderer 组件](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/shape-components/).

你可以设置 *u* 和 *v* 坐标在纹理的 2D 图像上与形状的顶点对应。实体拥有的顶点越多，就需要定义越多的 *uv* 坐标，例如一个平面需要有 8 个 *uv* 点被定义，两个面各 4 个。

```ts
const meshEntity = engine.addEntity()
Transform.create(meshEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setPlane(
	meshEntity,
	[
		0, 0.75,

		0.25, 0.75,

		0.25, 1,

		0, 1,

		0, 0.75,

		0.25, 0.75,

		0.25, 1,

		0, 1,
	]
)

Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'materials/wood.png',
		wrapMode: TextureWrapMode.TWM_REPEAT,
	}),
})
```

以下示例包含一个简化设置 uv 的函数。这里定义的 `setUVs` 函数接收行数和列数作为参数，并设置 uv，使纹理图像重复指定次数。

```ts
const meshEntity = engine.addEntity()
Transform.create(meshEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(meshEntity, setUVs(3, 3))

Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'materials/atlas.png',
		wrapMode: TextureWrapMode.TWM_REPEAT,
	}),
})

function setUVs(rows: number, cols: number) {
	return [
		//未旋转平面的北侧
		0, //左下角
		0,

		cols, //右下角
		0,

		cols, //右上角
		rows,

		0, //左上角
		rows,

		//未旋转平面的南侧
		cols, //右下角
		0,

		0, //左下角
		0,

		0, //左上角
		rows,

		cols, //右上角
		rows,
	]
}
```

为 `盒子` 网格形状设置 UV 时，结构也是一样的。立方体的 6 个面中的每个面都采用 4 对坐标，每个角一个。所有这 48 个值都作为一个数组列出。


> [!WARNING]
> **📔 注意**：Uv 属性目前仅在 `平面` 和 `盒子` 形状上可用。此外， *uv* 值会同等影响所有纹理层，因为它们设置在 *形状*.


### 纹理缩放

当纹理被拉伸或缩小到与原始纹理图像不同的尺寸时，有时会产生伪影。在 3D 环境中，透视效果会自然导致这种情况。存在各种 [纹理过滤](https://en.wikipedia.org/wiki/Texture_filtering) 算法，可通过不同方式对此进行补偿。

该 `材质` 对象默认使用 *双线性* 算法，但你可以通过设置纹理的 *最近邻* 或 *三线性* 算法来改用其他方式。 `filterMode` 属性。该值取自 `TextureFilterMode` 枚举：

* `TextureFilterMode.TFM_POINT`：使用“最近邻”算法。该设置非常适合像素艺术风格图形，因为当纹理在屏幕上看起来更大时，轮廓仍会保持清晰锐利，而不是模糊。
* `TextureFilterMode.TFM_BILINEAR`：使用双线性算法来估算每个像素的颜色。
* `TextureFilterMode.TFM_TRILINEAR`：使用三线性算法来估算每个像素的颜色。

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'materials/atlas.png',
		filterMode: TextureFilterMode.TFM_BILINEAR,
	}),
})
```

## 不受光照影响的材质

大多数情况下，你会希望场景中的材质受到光照条件的影响，包括阴影以及被不同时段的色调变化染色。但在其他情况下，你可能希望显示纯净状态的颜色。这在播放视频时很有用，也适用于需要突出显示、用于向玩家提示信息的抽象标记。

要创建一个不受光照影响的材质，请使用 `Material.setBasicMaterial`。基础材质并不具备与 PBR 材质完全相同的所有属性，它们只包含最基本的：

* `diffuseColor`：表示颜色的 Color4
* `texture`：纹理
* `alphaTexture`：透明层的独立纹理
* `alphaTest`：基于纹理颜色实现透明度的阈值
* `castShadows`：如果为 false，则不会将阴影投射到场景中的其他实体上。

```ts
Material.setBasicMaterial(screen, {
	diffuseColor: Color4.Red(),
})
```

## 头像

要显示任意玩家的缩略头像，请使用 `Material.Texture.Avatar` 在设置材质纹理时，传入一个现有玩家的地址。这样会从该玩家的 256x256 图像生成纹理，显示头部和肩膀。玩家会显示为当前服务器最近记录的穿戴物。

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Avatar({
		userId: '0x517....',
	}),
})
```

![](/docs/_remote/3907d796a3f7f47add13d85a.webp)

你可以获取任意 Decentraland 玩家头像，即使他们当前未连接，也即使他们没有已认领的 Decentraland 名称。

你传入作为参数的对象中支持以下属性：

* `userId`要显示其个人资料的用户 ID
* `filterMode`：决定纹理在渲染时像素如何拉伸或压缩。该值取自 `TextureFilterMode` 枚举。参见 [纹理缩放](#texture-scaling).
* `wrapMode`：决定纹理如何平铺到对象上。该值取自 `TextureWrapMode` 枚举。参见 [纹理包裹](#texture-wrapping).

## 透明材质

要让纯色材质变透明，只需将颜色定义为一个 `Color4`，并将第 4 个值设为介于 *0* 和 *1*。越接近 *1*，它就越不透明。

```typescript
let transparentRed = Color4.create(1, 0, 0, 0.5)

Material.setPbrMaterial(meshEntity, {
	albedoColor: transparentRed,
})
```

如果材质使用了包含透明度的 .png 纹理，它默认会是不透明的，但你可以通过设置 `transparencyMode` 设置为 `MaterialTransparencyMode.MTM_ALPHA_BLEND`.

![](/docs/_remote/98a85f0b352d710c0c48036c.webp)

```typescript
Material.setPbrMaterial(floor, {
	texture: Material.Texture.Common({
		src: 'assets/scene/transparent-image.png',
	}),
	transparencyMode: MaterialTransparencyMode.MTM_ALPHA_BLEND,
})
```

该 `transparencyMode` 可以具有以下值：

* `MaterialTransparencyMode.MTM_OPAQUE`：完全不透明
* `MaterialTransparencyMode.MTM_ALPHA_TEST`：每个像素要么完全不透明，要么完全透明，取决于阈值。
* `MaterialTransparencyMode.MTM_ALPHA_BLEND`：可根据每个像素的值产生中间值。
* `MaterialTransparencyMode.MTM_ALPHA_TEST_AND_ALPHA_BLEND`：结合了这两种方法。
* `MaterialTransparencyMode.MTM_AUTO`：根据提供的纹理来确定方法。


> [!WARNING]
> **📔 注意**：始终设置 `transparencyMode` 当你希望纹理自身的 alpha 通道显示出来时，请自行设置。在桌面客户端上，默认 `MTM_AUTO` 模式会忽略纹理的 alpha，并且只在 `albedoColor` alpha 低于 1 时进行混合，所以一个透明的 `.png` 会变成完全不透明。使用 `MTM_ALPHA_BLEND` 或 `MTM_ALPHA_TEST`时，透明度是纹理的 alpha 乘以 `albedoColor` alpha。


如果你设置 `transparencyMode` 设置为 `MaterialTransparencyMode.MTM_ALPHA_TEST`，你可以微调用于判断每个像素是否透明的阈值。将该 `alphaTest` 属性设置在 *0* 和 *1*。默认值为 *0.5*.

```ts
// 使用 alpha 测试
Material.setPbrMaterial(meshEntity1, {
	texture: Material.Texture.Common({
		src: 'images/myTexture.png',
	}),
	transparencyMode: MaterialTransparencyMode.MTM_ALPHA_TEST,
	alphaTest: 1,
})
```

当使用 [无光照材质](#unlit-materials)时，你可以添加一个 `alphaTexture` ，使材质只有某些区域根据纹理变为透明。


> [!WARNING]
> **📔 注意**：在材质应显示的地方将遮罩涂成白色，在应消失的地方涂成黑色。客户端读取的是遮罩颜色通道的亮度，而不是它的 alpha 通道，因此任何一个通道满值都算不透明：纯红色遮罩也同样是不透明。不要把你的颜色纹理重复用作它自己的遮罩，否则其中每个较暗的像素都会变得透明，并吞掉你的轮廓和阴影。


![](/docs/_remote/9b242e1d8862ee66017fbceb.webp)

```ts
// 使用遮罩图片将一个屏幕裁成圆形
Material.setBasicMaterial(meshEntity1, {
	texture: Material.Texture.Common({
		src: 'images/myTexture.png',
	}),
	alphaTexture: Material.Texture.Common({
		src: 'assets/scene/circle_mask.png',
		wrapMode: TextureWrapMode.TWM_MIRROR,
	}),
})
```

关于 `alphaTexture` 在桌面客户端上，还有两件事要知道：

* **它仅适用于无光照材质。** `Material.setPbrMaterial` 接收一个 `alphaTexture` ，但从不使用它。对于 PBR 材质，请将透明度放在颜色纹理自身的 alpha 通道中，并设置 `transparencyMode` 来代替。
* **没有 `alphaTexture` 的无光照材质是完全不透明的**，以及 `alphaTest` 对它没有影响。一个 `texture` 加上 `alphaTest` 单独绘制透明像素的颜色，通常是黑色。

这可以与视频结合以非常有趣的方式使用。参见 [视频播放](/docs/creator/content-creator-zh/chang-jing-sdk7/mei-ti/video-playing/).

## 视频播放

要将来自 URL 的视频流式传输到材质中，或播放存储在场景中的文件视频，请参见 [视频播放](/docs/creator/content-creator-zh/chang-jing-sdk7/mei-ti/video-playing/).

视频作为材质上的纹理使用，你可以设置材质的任何其他属性来改变视频屏幕的外观。

## 高级语法

创建一个 `材质` 组件的完整语法，不使用任何简化它的辅助函数，看起来是这样的：

```ts
Material.create(myEntity, {
	material: {
		$case: 'pbr',
		pbr: {
			texture: {
				tex: {
					$case: 'texture',
					texture: {
						src: 'images/scene-thumbnail.png',
					},
				},
			},
		},
	},
})

Material.create(myEntity, {
	material: {
		$case: 'pbr',
		pbr: {
			texture: {
				tex: {
					$case: 'avatarTexture',
					avatarTexture: {
						userId: '0x517....',
					},
				},
			},
		},
	},
})
```

基础协议是这样解释 Materials 组件的。辅助函数将其抽象掉，并提供更友好的语法，但在幕后它们输出的就是这种语法。

该 `$case` 字段允许你指定一种允许的类型。每种类型都支持一组不同的参数。这里有两层 `$case` ：

* 该 `material` 字段支持以下值 `pbr` 和 `unlit`，用于定义材质的类型。
* 该 `tex` 字段在纹理内部支持以下值，用于定义纹理的类型：
  * `texture`
  * `avatarTexture`
  * `videoTexture`

根据 `$case`的值，定义相应类型的对象并传入任何相关属性是有效的。

要向可能已经有此组件实例的实体添加一个 `材质` 组件，请使用 `Material.createOrReplace()`。像 `Material.setPbrMaterial()` 这样的辅助函数会处理覆盖组件的现有实例，但运行 `Material.create()` 在已经有此组件的实体上会返回错误。

## 修改 glTF 材质

使用 `GltfNodeModifiers` 组件来修改一个 *glTF* 模型的材质。此组件允许你用自己的材质覆盖一个 *glTF* 模型的材质。你可以使用 `材质` 组件的任何属性，包括纹理、视频纹理、无光照材质等。

使用 `GltfNodeModifiers` 组件：

* 有两种方式：通过保留 `path` 属性为空字符串来修改整个模型的材质。
* 通过将 `path` 属性设置为节点路径来修改模型中某个特定节点（或多个节点）的材质。

### 修改整个模型的材质

以下示例展示了如何修改一个 *glTF* 模型的材质。在这种情况下，整个模型的材质被修改为红色。

```ts
import { GltfNodeModifiers, GltfContainer, Transform } from '@dcl/sdk/ecs'

const myEntity = engine.addEntity()

GltfContainer.create(myEntity, {
	src: 'models/myModel.glb',
})

Transform.create(myEntity, {
	position: Vector3.create(4, 0, 4),
})

GltfNodeModifiers.create(myEntity, {
	modifiers: [
		{
			path: '',
			material: {
				material: {
					$case: 'pbr',
					pbr: {
						albedoColor: Color4.Red(),
					},
				},
			},
		},
	],
})
```

该 `GltfNodeModifiers` 组件具有以下属性：

* `modifiers`：一个修饰器数组。每个修饰器具有以下属性：
  * `path`：要修改的模型中节点的路径。
  * `material`：要使用的材质。

该 `path` 属性是一个字符串，表示模型中要修改的节点路径 *glTF* 。如果你想修改整个模型的材质，可以使用空字符串。如果你想修改某个特定节点的材质，可以使用该节点的路径。该路径必须指向一个网格节点，而不是顶点节点。


> [!NOTE]
> **💡 提示**：你可以使用 [Babylon Sandbox 应用](https://sandbox.babylonjs.com/) 来检查 *glTF* 模型并找到你想修改的节点路径。
> 
> 不过，在某些模型中，Babylon sandbox 可能会列出属于顶点而不是网格的路径，这些路径将无法工作。如果你尝试使用无效路径，场景控制台将显示一条错误消息，其中包含该模型上所有有效路径的完整列表。


该 `material` 属性是一个表示要使用的材质的对象。它需要使用 [高级语法](#advanced-syntax) 来编写，如上例所示。像 `Material.setPbrMaterial()` 这样的辅助函数不能在这里使用。

### 修改模型中特定节点的材质

以下示例展示了如何修改 *glTF* 模型中某个特定节点的材质。在这种情况下，头部的材质被修改为使用替代纹理。

```ts
import { GltfNodeModifiers, GltfContainer, Transform } from '@dcl/sdk/ecs'

const myEntity = engine.addEntity()

GltfContainer.create(myEntity, {
	src: 'models/myModel.glb',
})

Transform.create(myEntity, {
	position: Vector3.create(4, 0, 4),
})

GltfNodeModifiers.create(myEntity, {
	modifiers: [
		{
			path: 'M_Head_BaseMesh',
			material: {
				material: {
					$case: 'pbr',
					pbr: {
						texture: Material.Texture.Common({
							src: 'assets/scene/images/blinking-head.png',
						}),
					},
				},
			},
		},
	],
})
```

一个 `GltfNodeModifiers` 可以包含多个修饰器，每个修饰器修改模型中的不同节点。以下示例展示了如何修改一个 *glTF* 模型的头部和身体的材质。

```ts
import { GltfNodeModifiers, GltfContainer, Transform } from '@dcl/sdk/ecs'

const myEntity = engine.addEntity()

GltfContainer.create(myEntity, {
	src: 'models/myModel.glb',
})

Transform.create(myEntity, {
	position: Vector3.create(4, 0, 4),
})

GltfNodeModifiers.create(myEntity, {
	modifiers: [
		{
			path: 'M_Head_BaseMesh',
			material: {
				material: {
					$case: 'pbr',
					pbr: {
						albedoColor: Color4.Red(),
					},
				},
			},
		},
		{
			path: 'M_Body_BaseMesh',
			material: {
				material: {
					$case: 'pbr',
					pbr: {
						albedoColor: Color4.Blue(),
					},
				},
			},
		},
	],
})
```

### 修改现有材质中的字段

Material 组件提供了一个简化接口，用于访问和修改 Material 组件属性。它消除了深入嵌套联合结构（`PBR` 与 `Unlit`, `texture` 与 `avatarTexture` 与 `videoTexture`）的需要，使材质操作更加直观且更不易出错。

以下方法可让你轻松操作 `材质` 组件：

* `Material.getFlat(entity: Entity): ReadonlyFlatMaterial`：它返回一个 **只读** `FlatMaterial` 访问器对象，可直接读取材质属性。如果实体没有 `材质` 组件，它将返回错误。为避免这一点，可使用后续方法作为替代。

```ts
const src = Material.getFlat(entity).texture.src
```

* `Material.getFlatOrNull(entity: Entity): ReadonlyFlatMaterial | null`：它返回一个 **只读或空**，具体取决于实体是否具有一个 `材质` 组件。

```ts
const src = Material.getFlatOrNull(entity)?.texture.src
```

* `Material.getFlatMutable(entity: Entity): FlatMaterial`：它返回一个 **读写** `FlatMaterial`对象，允许修改其属性。如果被调用的实体没有 `材质` 组件。

```ts
const src = Material.getFlatMutable(entity).texture.src
```

* `Material.getFlatMutableOrNull(entity: Entity): FlatMaterial | null`：它返回一个 **读写或空** `FlatMaterial` 对象，允许修改其属性。如果实体没有 `Material 组件`，它将返回 `null`.

```ts
const flatMaterial = Material.getFlatMutableOrNull(entity)
if (flatMaterial) {
	flatMaterial.texture.src = myNewTextureFile
}
```

### 从 glTF 模型中移除阴影

要从一个 *glTF* 模型中移除阴影，你可以将 `castShadows` 属性设置为 `false` ，位于 `GltfNodeModifiers` 对象。这会保留模型的原始材质，但阻止它投射阴影。这对不需要投射阴影的模型很有用，例如光束。

```ts
import { GltfNodeModifiers } from '@dcl/sdk/ecs'

GltfNodeModifiers.create(myEntity, {
	modifiers: [
		{
			path: '',
			castShadows: false,
		},
	],
})
```


> [!NOTE]
> **💡 提示**：材质中使用的纹理也可以应用到粒子效果上。参见 [粒子系统](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/particle-system/) ，了解如何在粒子中使用自定义纹理和精灵图集的详细信息。



> [!NOTE]
> **💡 提示**：有关材质的工作示例，请参见 [`52,-52-testing-gallery`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/52,-52-testing-gallery) 测试场景，它并排展示 PBR 金属度/粗糙度/自发光/法线贴图组合， [`74,-8-gltfnodemodifier`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/74,-8-gltfnodemodifier)，它会覆盖单个 GLTF 节点上的 PBR 和无光照材质，以及 [`0,3-texture-movement`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,3-texture-movement)，它会为纹理偏移和平铺制作动画。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
