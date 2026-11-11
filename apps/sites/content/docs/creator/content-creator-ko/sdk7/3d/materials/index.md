# 재질

단순한 형태의 엔티티에 재질과 텍스처를 추가하는 방법을 알아봅니다.

## 재질

원시 도형(큐브, 구, 평면 등)을 사용하는 엔티티에는 다음을 추가해 재질을 적용할 수 있습니다: `재질` 컴포넌트입니다. 이 컴포넌트에는 재질의 속성을 설정하고, 텍스처를 추가하는 등의 여러 필드가 있습니다.

*glTF* 모델에는 모델과 함께 암시적으로 씬에 가져와지는 자체 재질이 포함됩니다. 이러한 재질을 수정하거나 덮어쓰려면 다음을 사용하세요: `GltfNodeModifiers` 컴포넌트를 사용하세요. 참조: [glTF 재질 수정](#modify-gltf-materials) 에서 자세한 내용을 확인하세요.

자체 재질이 있는 3D 모델을 가져올 때는 모든 셰이더가 Decentraland 엔진에서 지원되는 것은 아니라는 점을 유의하세요. 표준 재질과 PBR(물리 기반 렌더링) 재질만 지원됩니다. 참조: [외부 3D 모델 고려 사항](https://github.com/decentraland/docs/blob/main/creator/3d-modeling/materials.md) 에서 자세한 내용을 확인하세요.

지원되는 재질에는 여러 종류가 있습니다:

* PBR(Physically Based Rendering): Decentraland에서 가장 일반적인 종류의 재질입니다. 단색이나 텍스처를 지원하며, 금속성, 발광, 투명도 등 다양한 속성을 지원합니다. 자세한 내용은 다음을 참조하세요: [PBR](https://en.wikipedia.org/wiki/Physically_based_rendering).
* 기본 재질: 빛과 그림자에 반응하지 않기 때문에 빌보드 이미지를 표시하는 데 이상적입니다.

## Creator Hub의 Scene Editor를 사용하세요

엔티티에 Material을 적용하는 가장 쉬운 방법은 Scene Editor를 사용하는 것입니다. 다음을 추가할 수 있습니다: **재질** 컴포넌트를 엔티티에 추가한 다음 Scene Editor UI에서 사용 가능한 모든 필드를 구성하세요. 참조: [컴포넌트 추가](/docs/creator/content-creator-ko/scene-editor/build/components/#add-components).

## 재질 추가

다음 예제는 PBR 재질을 만들고, 일부 필드를 설정하여 빨간색과 금속성 속성을 부여합니다. 이 재질은 상자 모양도 가진 엔티티에 추가되므로, 이 재질로 상자의 색이 지정됩니다.

```ts
//엔티티 생성 및 모양 지정
const meshEntity = engine.addEntity()
Transform.create(meshEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(meshEntity)

//재질을 생성하고 필드 구성
Material.setPbrMaterial(meshEntity, {
	albedoColor: Color4.Red(),
	metallic: 0.8,
	roughness: 0.1,
})
```

이미 다음이 있는 엔티티의 재질을 변경하려면 `재질` 컴포넌트가 있는 경우, 다음을 실행하세요: `Material.setPbrMaterial()` 또는 다른 헬퍼 함수를 사용하면 원래 재질을 덮어씁니다. 원래 `재질` 를 제거하거나 고급 문법을 사용할 필요는 없습니다.

```ts
//엔티티 생성 및 모양 지정
const meshEntity = engine.addEntity()
Transform.create(meshEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(meshEntity)

//재질을 생성하고 필드 구성
Material.setPbrMaterial(meshEntity, {
	albedoColor: Color4.Red(),
})

//새 재질 컴포넌트로 덮어쓰기
Material.setPbrMaterial(meshEntity, {
	albedoColor: Color4.Blue(),
})
```


> [!WARNING]
> **📔 참고** :  `재질` 컴포넌트는 다음을 통해 import해야 합니다:
> 
> > `import { Material } from "@dcl/sdk/ecs"`
> 
> 다음을 참조하세요 [가져오기](/docs/creator/content-creator-ko/sdk7/getting-started/coding-scenes/#imports) 를 쉽게 다루는 방법을 확인하세요.


## 재질 색상

재질에 단색을 지정하세요. PBR Material에서는 다음을 설정합니다: `albedoColor` 필드입니다. 알베도 색상은 빛에 반응하며 음영을 포함할 수 있습니다.

색상 값의 타입은 `Color4`이며, *r*, *g* 및 *b* 값(빨강, 초록, 파랑)으로 구성됩니다. 각 값은 0에서 1 사이의 값을 가집니다. 이 값들을 다르게 설정하면 보이는 모든 색을 구성할 수 있습니다. 검은색은 세 값을 모두 0으로 설정하세요. 흰색은 모두 1로 설정하세요.


> [!WARNING]
> **📔 참고**: 만약 `albedoColor` 에서 어떤 색상을 *1*보다 큰 값으로 설정되면 *발광*으로 보이며, 값이 클수록 더 강한 발광을 냅니다. 예를 들어, `{r: 15, g: 0, b: 0}` 는 매우 밝은 빨간색 빛을 만듭니다.


다음을 참조하세요 [색상 유형](/docs/creator/content-creator-ko/sdk7/3d/color-types/) 에 대해 자세히 알아보세요.

색상이 어떻게 인식되는지 미세 조정하려면 PBR Material에서 다음 필드를 편집할 수도 있습니다:

* *emissiveColor*: 재질에서 방출되는 색상.
* *reflectivityColor*: 즉, *정반사 색상* 이라고도 합니다.

환경의 빛과 그림자의 영향을 받지 않는 단색 재질을 만들려면 PBR 재질 대신 기본 재질을 만드세요.

```ts
Material.setBasicMaterial(myEntity, {
	diffuseColor: Color4.Black(),
})
```

## 텍스처 사용

다음의 `texture` 매개변수를 설정하여 이미지 파일을 재질의 텍스처로 지정하세요.

```ts
//엔티티 생성 및 모양 지정
const meshEntity = engine.addEntity()
Transform.create(meshEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(meshEntity)

//재질을 생성하고 필드 구성
Material.setPbrMaterial(meshEntity, {
	texture: Material.Texture.Common({
		src: 'assets/materials/wood.png',
	}),
})
```

위의 예제에서 재질 이미지 파일은 `assets/materials` 폴더에 있으며, 이 폴더는 씬 프로젝트 폴더의 루트 수준에 있습니다.


> [!NOTE]
> **💡 팁**: 텍스처 이미지 파일은 씬 내부의 `/assets` 폴더 어딘가에 두는 것을 권장합니다.


텍스처를 만드는 동안 추가 매개변수도 전달할 수 있습니다:

* `filterMode`: 렌더링될 때 텍스처의 픽셀이 어떻게 늘어나거나 압축되는지 결정합니다. 이 값은 다음에서 가져옵니다: `TextureFilterMode` 열거형. 참조: [텍스처 스케일링](#texture-scaling).
* `wrapMode`: 텍스처가 오브젝트에 어떻게 타일링되는지 결정합니다. 이 값은 다음에서 가져옵니다: `TextureWrapMode` 열거형. 참조: [텍스처 래핑](#texture-wrapping).

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'assets/materials/wood.png',
		filterMode: TextureFilterMode.TFM_BILINEAR,
		wrapMode: TextureWrapMode.TWM_CLAMP,
	}),
})
```

환경의 빛과 그림자의 영향을 받지 않는 텍스처를 만들려면 PBR 재질 대신 기본 재질을 만드세요.

```ts
Material.setBasicMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'assets/materials/wood.png',
	}),
})
```

### 외부 URL의 텍스처

씬 프로젝트 내부 경로 대신 외부 URL을 재질의 텍스처로 지정할 수 있습니다.

```ts
Material.setBasicMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'https://wearable-api.decentraland.org/v2/collections/community_contest/wearables/cw_tuxedo_tshirt_upper_body/thumbnail',
	}),
})
```

URL은 다음으로 시작해야 합니다: `https`, `http` URL은 지원되지 않습니다.

### 텍스처 래핑

텍스처가 표면에 어떻게 맞춰질지 설정할 수 있습니다. 기본적으로 텍스처는 표면 전체를 한 번 덮도록 늘어나지만, 크기 조정과 오프셋도 할 수 있습니다.

다음 필드는 모든 텍스처에서 사용할 수 있습니다:

* `offset`: 텍스처의 정렬을 변경하기 위해 텍스처를 이동합니다. 값은 Vector2이며, 두 축 모두 0에서 1까지입니다. 1은 텍스처의 전체 너비 또는 높이를 의미합니다.
* `tiling`: 텍스처를 크기 조정합니다. 기본값은 Vector 2 `[1, 1]`이며, 이미지가 표면 전체를 덮도록 한 번 반복되게 합니다.
* `TextureWrapMode`: 이미지 타일링이 표면 전체를 덮지 못할 경우 어떻게 되는지 결정합니다. 이 속성의 값은 다음에서 가져옵니다: `TextureWrapMode` 열거형으로, 다음 값을 사용할 수 있습니다:
  * `TextureWrapMode.TWM_CLAMP`: 텍스처는 지정된 크기로 한 번만 표시됩니다. 메쉬 표면의 나머지 부분은 투명하게 남습니다. `tiling` 의 값은 무시됩니다.
  * `TextureWrapMode.TWM_REPEAT`: 텍스처는 지정된 크기에 맞는 만큼 메쉬에 여러 번 반복됩니다.
  * `TextureWrapMode.TWM_MIRROR`: wrap과 마찬가지로 텍스처가 들어가는 만큼 반복되지만, 각 반복의 방향이 반전됩니다.

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
> **📔 참고** :  `offset` 및 `tiling` 속성은 DCL 2.0 데스크톱 클라이언트에서만 지원됩니다.


이 기능을 사용해 큰 표면을 타일 패턴으로 덮을 수 있습니다. 예를 들어 다음 이미지를 반복하세요:

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

아래 예제에서 텍스처는 *미러* 래핑 모드를 사용하며, 각 텍스처 반복은 표면의 1/4만 차지합니다. 즉, 양쪽 축에서 서로 반전된 이미지 4개가 보이게 됩니다.

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'materials/atlas.png',
		wrapMode: TextureWrapMode.TWM_MIRROR,
		tiling: Vector2.create(0.25, 0.25),
	}),
})
```

### 텍스처 트윈

다음과 같은 `Tween` 컴포넌트를 사용하고 `TextureMove` 모드로 설정하여 텍스처를 부드럽게 움직이세요. 트윈은 `offset` 또는 다음: `tiling` 속성의 값을 일정 시간에 걸쳐 부드럽고 최적화된 방식으로 점진적으로 변경합니다.


> [!WARNING]
> **📔 참고**: 텍스처 트윈은 DCL 2.0 데스크톱 클라이언트에서만 지원되는 기능입니다.


다음을 사용하세요 `Tween` 컴포넌트와 `setTextureMove` 함수를 사용해 텍스처를 두 위치 사이로 이동하세요.

```ts
Tween.setTextureMove(myEntity, Vector2.create(0, 0), Vector2.create(1, 0), 2000)
```

텍스처 트윈에는 다음 정보가 포함됩니다:

* `엔티티에`: 이동시킬 텍스처의 엔티티
* `start`: 시작 위치를 나타내는 Vector2
* `end`: 종료 위치를 나타내는 Vector2
* `duration`: 두 위치 사이를 이동하는 데 걸리는 밀리초 수

다음 선택적 매개변수도 사용할 수 있습니다:

* `movementType`: 이동이 다음에서 이루어질지 정의합니다: `offset` 또는 다음: `tiling` 필드입니다. 기본적으로 다음을 사용합니다: `offset`.
* `easingFunction`: 시간에 따른 변화율 곡선이며, 기본값은 `EasingFunction.EF_LINEAR`입니다. 다른 값은 변화가 서로 다른 속도로 가속 및/또는 감속되게 합니다.

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

위의 예제는 1초 동안 지속되는 트윈을 실행하며 텍스처를 한 번만 이동합니다. 연속적인 움직임, 예를 들어 폭포가 떨어지는 모습을 시뮬레이션하려면 다음을 사용해야 합니다: `setTextureMoveContinuous`.

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

위의 예제는 `setTextureMoveContinuous`를 사용하며, 방향은 `(0, 1)`이고 속도는 초당 1단위입니다.

텍스처 연속 트윈에는 다음 정보가 포함됩니다:

* `엔티티에`: 이동시킬 텍스처의 엔티티
* `direction`: 이동 방향을 나타내는 Vector2
* `speed`: 엔티티가 초당 이동할 단위 수

다음 선택적 매개변수도 사용할 수 있습니다:

* `movementType`: 이동이 다음에서 이루어질지 정의합니다:
* `duration`: 이동을 유지할 밀리초 수입니다. 이 시간이 지나면 이동이 중지됩니다.

**복잡한 트윈 시퀀스**

원하는 만큼 많은 단계로 텍스처 이동을 복잡한 시퀀스로 구성할 수도 있습니다. 다음의 `sequence` 필드를 사용하여 원하는 만큼 트윈을 나열하면, 이들은 `Tween` 컴포넌트.

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

TweenSequence 안에서 트윈을 정의할 때는 `Tween.Mode.TextureMove` 의 더 자세한 형식을 사용해 트윈을 정의해야 합니다.

### 다층 텍스처

더 현실적인 텍스처를 구성하기 위해 여러 이미지 파일을 레이어로 사용할 수 있습니다. 예를 들어 다음을 포함할 수 있습니다: `bumpTexture` 와 `emissiveTexture`.

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

다음 `bumpTexture` 는 표면의 각 픽셀에서 표면 노멀의 동작을 수정하여 표면의 돌기와 주름을 시뮬레이션할 수 있습니다.

![](/docs/_remote/b02dcfc858055419931f396c.webp)

다음 `emissiveTexture` 는 재질의 특정 부분에서 빛을 더 강조하여 매우 흥미로운 효과를 만들 수 있습니다.

**UV 설정**

텍스처의 크기나 정렬을 바꾸는 또 다른 방법은 다음을 구성하는 것입니다: *uv* 속성을 [MeshRenderer 컴포넌트에](/docs/creator/content-creator-ko/sdk7/3d/shape-components/).

다음과 같이 설정합니다: *u* 및 *v* 텍스처의 2D 이미지에서 좌표를 지정하여 모양의 정점에 대응시킵니다. 엔티티에 정점이 많을수록 정의해야 하는 *uv* 좌표도 더 많아집니다. 예를 들어 평면은 8개의 *uv* 점이 정의되어야 하며, 각 면마다 4개씩입니다.

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

다음 예제에는 UV 설정을 단순화하는 함수가 포함되어 있습니다. 여기서 정의된 `setUVs` 함수는 행과 열의 수를 매개변수로 받아, 텍스처 이미지가 특정 횟수만큼 반복되도록 UV를 설정합니다.

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
		//회전되지 않은 평면의 북쪽 면
		0, //왼쪽 아래 모서리
		0,

		cols, //오른쪽 아래 모서리
		0,

		cols, //오른쪽 위 모서리
		rows,

		0, //왼쪽 위 모서리
		rows,

		//회전되지 않은 평면의 남쪽 면
		cols, // 오른쪽 아래 모서리
		0,

		0, // 왼쪽 아래 모서리
		0,

		0, // 왼쪽 위 모서리
		rows,

		cols, // 오른쪽 위 모서리
		rows,
	]
}
```

의 UV를 설정하려면 `박스` 메쉬 모양에도 같은 구조가 적용됩니다. 큐브의 6개 면 각각은 모서리마다 하나씩 4쌍의 좌표를 사용합니다. 이 48개 값은 모두 하나의 배열로 나열됩니다.


> [!WARNING]
> **📔 참고**: UV 속성은 현재 다음에서만 사용할 수 있습니다: `평면` 과 `박스` 모양. 또한, *uv* 값은 *모양*.


### 텍스처 스케일링

텍스처가 원본 텍스처 이미지와 다른 크기로 늘어나거나 줄어들 때, 때로는 아티팩트가 생길 수 있습니다. 3D 환경에서는 원근 효과 때문에 이런 현상이 자연스럽게 발생합니다. 이를 여러 방식으로 보정하는 다양한 [텍스처 필터링](https://en.wikipedia.org/wiki/Texture_filtering) 알고리즘이 존재합니다.

다음 `재질` 오브젝트는 기본적으로 *바이리니어* 알고리즘을 기본적으로 사용하지만, *최근접 이웃* 또는 *트라이리니어* 알고리즘을 대신 사용하도록 `filterMode` 속성을 설정하여 구성할 수 있습니다. 이 값은 다음에서 가져옵니다: `TextureFilterMode` enum:

* `TextureFilterMode.TFM_POINT`: "nearest neighbor" 알고리즘을 사용합니다. 이 설정은 픽셀 아트 스타일 그래픽에 이상적입니다. 텍스처가 화면에서 더 크게 보일 때 윤곽선이 흐려지지 않고 선명하게 유지되기 때문입니다.
* `TextureFilterMode.TFM_BILINEAR`: 각 픽셀의 색상을 추정하기 위해 bilinear 알고리즘을 사용합니다.
* `TextureFilterMode.TFM_TRILINEAR`: 각 픽셀의 색상을 추정하기 위해 trilinear 알고리즘을 사용합니다.

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'materials/atlas.png',
		filterMode: TextureFilterMode.TFM_BILINEAR,
	}),
})
```

## 언릿 재질

대부분의 경우 씬의 재질이 그림자를 포함한 조명 조건의 영향을 받고, 하루 중 시간대에 따른 색조 변화도 반영되길 원할 것입니다. 하지만 경우에 따라서는 색상을 순수한 상태로 보여주고 싶을 수 있습니다. 이는 비디오를 재생할 때나, 플레이어에게 힌트를 알리기 위해 눈에 띄어야 하는 추상적인 마커에도 유용합니다.

언릿 재질을 만들려면 다음을 사용하세요: `Material.setBasicMaterial`. 기본 재질은 PBR 재질과 모든 속성이 동일하지 않으며, 필수 항목만 가집니다:

* `diffuseColor`: 색상을 위한 Color4
* `texture`: 텍스처
* `alphaTexture`: 투명도 레이어를 위한 별도 텍스처
* `alphaTest`: 텍스처 색상을 기반으로 투명도를 구현하기 위한 임계값
* `castShadows`: false이면 씬의 다른 엔티티에 그림자가 투영되지 않습니다.

```ts
Material.setBasicMaterial(screen, {
	diffuseColor: Color4.Red(),
})
```

## 아바타 초상화

아무 플레이어의 썸네일 이미지를 표시하려면 다음을 사용하세요: `Material.Texture.Avatar` 재질의 텍스처를 설정할 때 현재 플레이어가 아닌 기존 플레이어의 주소를 전달합니다. 그러면 머리와 어깨가 보이는 256x256 크기의 플레이어 텍스처가 생성됩니다. 플레이어는 현재 서버가 마지막으로 기록한 착용 아이템 세트를 착용한 모습으로 표시됩니다.

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Avatar({
		userId: '0x517....',
	}),
})
```

![](/docs/_remote/3907d796a3f7f47add13d85a.webp)

현재 연결되어 있지 않더라도, 그리고 Decentraland 이름을 주장하지 않았더라도 모든 Decentraland 플레이어의 초상화를 가져올 수 있습니다.

전달하는 객체에서 다음 속성이 지원됩니다:

* `userId`: 표시하려는 프로필의 사용자 ID
* `filterMode`: 렌더링될 때 텍스처의 픽셀이 어떻게 늘어나거나 압축되는지 결정합니다. 이 값은 다음에서 가져옵니다: `TextureFilterMode` 열거형. 참조: [텍스처 스케일링](#texture-scaling).
* `wrapMode`: 텍스처가 오브젝트에 어떻게 타일링되는지 결정합니다. 이 값은 다음에서 가져옵니다: `TextureWrapMode` 열거형. 참조: [텍스처 래핑](#texture-wrapping).

## 투명 재질

단색 재질을 투명하게 만들려면 색상을 단순히 다음으로 정의하세요: `Color4`, 그리고 네 번째 값을 다음 사이의 값으로 설정하세요: *0* 및 *1*. 에 가까울수록 *1*더 불투명해집니다.

```typescript
let transparentRed = Color4.create(1, 0, 0, 0.5)

Material.setPbrMaterial(meshEntity, {
	albedoColor: transparentRed,
})
```

.png 텍스처에 투명도가 포함되어 있으면 기본적으로 불투명하지만, 다음을 설정하여 투명도를 활성화할 수 있습니다: `transparencyMode` 에서 `MaterialTransparencyMode.MTM_ALPHA_BLEND`.

![](/docs/_remote/98a85f0b352d710c0c48036c.webp)

```typescript
Material.setPbrMaterial(floor, {
	texture: Material.Texture.Common({
		src: 'assets/scene/transparent-image.png',
	}),
	transparencyMode: MaterialTransparencyMode.MTM_ALPHA_BLEND,
})
```

다음 `transparencyMode` 다음 값들을 가질 수 있습니다:

* `MaterialTransparencyMode.MTM_OPAQUE`: 투명도 없음
* `MaterialTransparencyMode.MTM_ALPHA_TEST`: 각 픽셀은 임계값에 따라 완전히 불투명하거나 완전히 투명합니다.
* `MaterialTransparencyMode.MTM_ALPHA_BLEND`: 각 픽셀의 값에 따라 중간값도 가능합니다.
* `MaterialTransparencyMode.MTM_ALPHA_TEST_AND_ALPHA_BLEND`: 두 방법을 모두 조합하여 사용합니다.
* `MaterialTransparencyMode.MTM_AUTO`: 제공된 텍스처에 따라 방법을 결정합니다.


> [!WARNING]
> **📔 참고**: 항상 설정하세요 `transparencyMode` 텍스처 자체의 알파 채널이 보이게 하려는 경우 직접 설정하세요. 데스크톱 클라이언트에서는 기본 `MTM_AUTO` 모드는 텍스처의 알파를 무시하고, 다음 경우에만 블렌딩합니다 `albedoColor` 알파가 1 미만일 때만 블렌딩하므로, 투명한 `.png` 파일은 완전히 불투명하게 표시됩니다.  `MTM_ALPHA_BLEND` 또는 `MTM_ALPHA_TEST`, 투명도는 텍스처의 알파에 `albedoColor` 알파를 곱한 값입니다.


다음을 설정하면 `transparencyMode` 에서 `MaterialTransparencyMode.MTM_ALPHA_TEST`, 각 픽셀이 투명한지 여부를 판단하는 데 사용되는 임계값을 세밀하게 조정할 수 있습니다. 다음을 설정하세요 `alphaTest` 속성을 다음 범위로 *0* 및 *1*. 기본값은 *0.5*.

```ts
// 알파 테스트 사용
Material.setPbrMaterial(meshEntity1, {
	texture: Material.Texture.Common({
		src: 'images/myTexture.png',
	}),
	transparencyMode: MaterialTransparencyMode.MTM_ALPHA_TEST,
	alphaTest: 1,
})
```

다음을 사용할 때 [언릿 머티리얼](#unlit-materials), 다음을 추가하여 `alphaTexture` 텍스처를 기준으로 머티리얼의 특정 영역만 투명하게 만들 수 있습니다.


> [!WARNING]
> **📔 참고**: 머티리얼이 보여야 하는 부분은 흰색으로, 사라져야 하는 부분은 검은색으로 마스크를 칠하세요. 클라이언트는 마스크의 알파 채널이 아니라 색상 채널의 밝기를 읽으므로, 어떤 채널이든 최대값이면 불투명으로 간주됩니다. 순수한 빨간색 마스크도 불투명합니다. 색상 텍스처를 자기 자신의 마스크로 재사용하지 마세요. 그러면 그 안의 어두운 픽셀이 모두 투명해져 윤곽선과 음영이 사라집니다.


![](/docs/_remote/9b242e1d8862ee66017fbceb.webp)

```ts
// 마스크 이미지로 화면을 원형으로 자르기
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

에 대해 알아둘 점 두 가지 `alphaTexture` 데스크톱 클라이언트에서:

* **언릿 머티리얼에서만 작동합니다.** `Material.setPbrMaterial` 을 받지만 `alphaTexture` 실제로는 사용하지 않습니다. PBR 머티리얼의 경우, 투명도는 색상 텍스처 자체의 알파 채널에 넣고 다음을 설정하세요 `transparencyMode` 대신.
* **다음이 없는 언릿 머티리얼은 `alphaTexture` 완전히 불투명합니다**, 그리고 `alphaTest` 에는 영향이 없습니다.  `texture` 에 더해 `alphaTest` 만으로는 투명한 픽셀의 색상을 그리는데, 보통 검은색입니다.

이것은 비디오와 함께 매우 흥미로운 방식으로 사용할 수 있습니다. 다음을 참조하세요 [비디오 재생](/docs/creator/content-creator-ko/sdk7/media/video-playing/).

## 비디오 재생

URL에서 비디오를 머티리얼로 스트리밍하거나, 씬에 저장된 파일에서 비디오를 재생하려면 다음을 참조하세요 [비디오 재생](/docs/creator/content-creator-ko/sdk7/media/video-playing/).

비디오는 머티리얼의 텍스처로 사용되며, 머티리얼의 다른 속성들을 설정해 비디오 화면의 모양을 바꿀 수 있습니다.

## 고급 문법

다음을 생성하는 전체 문법은 `재질` 컴포넌트, 헬퍼 없이 보면 다음과 같습니다:

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

기본 프로토콜은 Materials 컴포넌트를 이렇게 해석합니다. 헬퍼 함수는 이를 추상화해 더 편한 문법을 제공하지만, 내부적으로는 이 문법을 출력합니다.

다음 `$case` 필드를 사용하면 허용된 유형 중 하나를 지정할 수 있습니다. 각 유형은 서로 다른 매개변수 집합을 지원합니다. 다음에는 두 단계의 `$case` 여기:

* 다음 `머티리얼` 필드는 다음 값을 지원합니다 `pbr` 및 `unlit`, 머티리얼 종류를 정의합니다.
* 다음 `tex` 텍스처 내부의 필드는 텍스처 종류를 정의하기 위해 다음 값을 지원합니다:
  * `texture`
  * `avatarTexture`
  * `videoTexture`

다음 값에 따라 `$case`, 해당 종류에 맞는 객체를 관련 속성과 함께 정의할 수 있습니다.

다음을 추가하려면 `재질` 이미 이 컴포넌트의 인스턴스가 있을 수 있는 엔티티에 컴포넌트를 추가하려면 다음을 사용하세요 `Material.createOrReplace()`. 다음과 같은 헬퍼 함수는 `Material.setPbrMaterial()` 기존 컴포넌트 인스턴스를 덮어쓰는 것을 처리하지만, 다음을 실행하면 `Material.create()` 이 컴포넌트가 이미 있는 엔티티에서 실행하면 오류가 반환됩니다.

## glTF 재질 수정

다음을 사용하세요 `GltfNodeModifiers` 컴포넌트는 다음의 머티리얼을 수정하는 데 사용됩니다: *glTF* 모델. 이 컴포넌트를 사용하면 다음의 머티리얼을 재정의할 수 있습니다: *glTF* 모델을 자신의 머티리얼로 바꿀 수 있습니다. 다음의 속성들을 모두 사용할 수 있습니다: `재질` 컴포넌트에는 텍스처, 비디오 텍스처, 언릿 머티리얼 등이 포함됩니다.

다음을 사용하는 방법은 두 가지입니다 `GltfNodeModifiers` 컴포넌트:

* 다음 값을 비워 두어 전체 모델의 머티리얼을 수정합니다 `path` 속성을 빈 문자열로 둡니다.
* 다음 값을 설정하여 모델의 특정 노드(또는 여러 노드)의 머티리얼을 수정합니다 `path` 속성을 해당 노드의 경로로 설정합니다.

### 전체 모델의 머티리얼 수정

다음 예시는 다음의 머티리얼을 수정하는 방법을 보여줍니다: *glTF* 모델. 이 경우 전체 모델의 머티리얼이 빨간색으로 수정됩니다.

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

다음 `GltfNodeModifiers` 컴포넌트에는 다음 속성들이 있습니다:

* `modifiers`: 수정자 배열입니다. 각 수정자는 다음 속성들을 가집니다:
  * `path`: 수정할 모델 내 노드의 경로입니다.
  * `머티리얼`: 사용할 머티리얼입니다.

다음 `path` 속성은 다음의 노드 경로를 나타내는 문자열입니다: *glTF* 수정할 모델. 전체 모델의 머티리얼을 수정하려면 빈 문자열을 사용할 수 있습니다. 특정 노드의 머티리얼을 수정하려면 해당 노드의 경로를 사용할 수 있습니다. 경로는 버텍스 노드가 아니라 메시 노드를 가리켜야 합니다.


> [!NOTE]
> **💡 팁**: 다음을 사용할 수 있습니다 [Babylon Sandbox 앱](https://sandbox.babylonjs.com/) 를 사용해 다음을 검사하고 *glTF* 모델에서 수정하려는 노드의 경로를 찾을 수 있습니다.
> 
> 하지만 일부 모델에서는 Babylon Sandbox가 메시가 아니라 정점에 속하는 경로를 나열할 수 있는데, 이는 작동하지 않습니다. 유효하지 않은 경로를 사용하려고 하면 씬의 콘솔에 해당 모델의 유효한 경로 전체 목록이 포함된 오류 메시지가 표시됩니다.


다음 `머티리얼` 속성은 사용할 머티리얼을 나타내는 객체입니다. 다음을 사용해 작성해야 합니다: [고급 문법](#advanced-syntax) 위 예시에서 보인 것처럼 머티리얼에 대한 문법입니다. 다음과 같은 헬퍼 함수는 `Material.setPbrMaterial()` 여기서는 사용할 수 없습니다.

### 모델의 특정 노드의 머티리얼 수정

다음 예시는 다음의 특정 노드의 머티리얼을 수정하는 방법을 보여줍니다: *glTF* 모델. 이 경우 머리의 머티리얼이 대체 텍스처를 사용하도록 수정됩니다.

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

하나의 `GltfNodeModifiers` 는 여러 수정자를 포함할 수 있으며, 각 수정자는 모델의 서로 다른 노드를 수정합니다. 다음 예시는 다음의 머리와 몸체의 머티리얼을 수정하는 방법을 보여줍니다: *glTF* 모델.

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

### 기존 머티리얼의 필드 수정

Material 컴포넌트는 Material 컴포넌트 속성에 접근하고 수정할 수 있는 간소화된 인터페이스를 제공합니다. 복잡하게 중첩된 유니온 구조(`PBR` 대 `Unlit`, `texture` 대 `avatarTexture` 대 `videoTexture`)를 탐색할 필요가 없어져, 머티리얼 조작이 더 직관적이고 오류 가능성이 줄어듭니다.

다음 메서드를 사용하면 다음을 쉽게 조작할 수 있습니다 `재질` 컴포넌트:

* `Material.getFlat(entity: Entity): ReadonlyFlatMaterial`: 다음을 반환합니다 **읽기 전용** `FlatMaterial` 액세서 객체로, 머티리얼 속성에 직접 읽기 접근을 제공합니다. 엔티티에 다음이 없으면 오류를 반환합니다 `재질` 컴포넌트. 이를 피하려면 다음 메서드를 대안으로 사용할 수 있습니다.

```ts
const src = Material.getFlat(entity).texture.src
```

* `Material.getFlatOrNull(entity: Entity): ReadonlyFlatMaterial | null`: 다음을 반환합니다 **읽기 전용 또는 null**, 엔티티에 다음이 있는지 없는지에 따라 `재질` 컴포넌트.

```ts
const src = Material.getFlatOrNull(entity)?.texture.src
```

* `Material.getFlatMutable(entity: Entity): FlatMaterial`: 다음을 반환합니다 **읽기 및 쓰기** `FlatMaterial`객체로, 속성을 수정할 수 있습니다. 호출된 엔티티에 다음이 없으면 오류를 발생시킵니다 `재질` 컴포넌트.

```ts
const src = Material.getFlatMutable(entity).texture.src
```

* `Material.getFlatMutableOrNull(entity: Entity): FlatMaterial | null`: 다음을 반환합니다 **읽기 및 쓰기 또는 null** `FlatMaterial` 객체로, 속성을 수정할 수 있습니다. 엔티티에 다음이 없는 경우 `Material 컴포넌트`, 다음을 반환합니다 `null`.

```ts
const flatMaterial = Material.getFlatMutableOrNull(entity)
if (flatMaterial) {
	flatMaterial.texture.src = myNewTextureFile
}
```

### glTF 모델에서 그림자 제거

다음에서 그림자를 제거하려면 *glTF* 모델의 경우 다음을 설정할 수 있습니다 `castShadows` 속성을 다음으로 `false` 를 `GltfNodeModifiers` 객체로 설정하세요. 이렇게 하면 모델의 원래 머티리얼은 유지되지만 그림자를 드리우지 않게 됩니다. 이는 광선처럼 그림자를 드리울 의도가 없는 모델에 유용합니다.

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
> **💡 팁**: 머티리얼에 사용된 텍스처는 파티클 효과에도 적용할 수 있습니다. 다음을 참조하세요 [Particle System](/docs/creator/content-creator-ko/sdk7/3d/particle-system/) 에서 파티클에 사용자 지정 텍스처와 스프라이트 시트를 사용하는 방법을 자세히 확인하세요.



> [!NOTE]
> **💡 팁**: 머티리얼의 동작 예시는 다음을 참조하세요 [`52,-52-testing-gallery`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/52,-52-testing-gallery) 테스트 씬. 여기서는 PBR의 금속성/거칠기/발광/노멀 맵 조합을 나란히 비교합니다, [`74,-8-gltfnodemodifier`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/74,-8-gltfnodemodifier)개별 GLTF 노드에서 PBR 및 언릿 머티리얼을 재정의하며,  [`0,3-texture-movement`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,3-texture-movement)텍스처 오프셋과 타일링을 애니메이션합니다.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/3d/materials.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
