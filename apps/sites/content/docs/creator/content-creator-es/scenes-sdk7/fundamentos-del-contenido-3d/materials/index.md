# Materiales

Aprende cómo añadir materiales y texturas a entidades con formas primitivas.

## Materials

Los Materials pueden aplicarse a Entities que usan formas primitivas (cubo, esfera, plano, etc.) agregando un `Material` component. Este component tiene varios campos que te permiten configurar las propiedades del material, agregar una texture, etc.

*glTF* los modelos incluyen sus propios Materials que se importan implícitamente en una Scene junto con el modelo. Para modificar o sobrescribir estos Materials, usa el `GltfNodeModifiers` component. Consulta [Modificar materiales glTF](#modify-gltf-materials) para más detalles.

Al importar un modelo 3D con sus propios materials, ten en cuenta que no todos los shaders son compatibles con el engine de Decentraland. Solo se admiten materials estándar y materials PBR (physically based rendering). Consulta [consideraciones sobre modelos 3D externos](https://github.com/decentraland/docs/blob/main/creator/3d-modeling/materials.md) para más detalles.

Hay distintos tipos de materials compatibles:

* PBR (Physically Based Rendering): El tipo de Material más común en Decentraland. Admite colores planos o textures, y distintas propiedades como metallic, emissive, transparency, etc. Lee más sobre [PBR](https://en.wikipedia.org/wiki/Physically_based_rendering).
* Basic materials: no responden a las luces ni a las sombras, lo que los hace ideales para mostrar imágenes de billboard.

## Usa el Scene Editor en Creator Hub

La forma más fácil de darle a una Entity un Material es usar el Scene Editor. Puedes agregar un **Material** component a tu Entity y luego configurar todos los campos disponibles en la UI del Scene Editor. Consulta [Add Components](/docs/creator/content-creator-es/scene-editor/construir/components/#add-components).

## Agregar un Material

El siguiente ejemplo crea un material PBR y establece algunos de sus campos para darle un color rojo y propiedades metálicas. Este material se agrega a una Entity que también tiene una forma de caja, por lo que coloreará la caja con este material.

```ts
//Crear Entity y asignar forma
const meshEntity = engine.addEntity()
Transform.create(meshEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(meshEntity)

//Crear Material y configurar sus campos
Material.setPbrMaterial(meshEntity, {
	albedoColor: Color4.Red(),
	metallic: 0.8,
	roughness: 0.1,
})
```

Para cambiar el material de una Entity que ya tiene un `Material` component, ejecuta `Material.setPbrMaterial()` o cualquiera de las otras helper functions y sobrescribirá el material original. No es necesario eliminar el original `Material` ni usar la sintaxis avanzada.

```ts
//Crear Entity y asignar forma
const meshEntity = engine.addEntity()
Transform.create(meshEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(meshEntity)

//Crear Material y configurar sus campos
Material.setPbrMaterial(meshEntity, {
	albedoColor: Color4.Red(),
})

//Sobrescribir con un nuevo component Material
Material.setPbrMaterial(meshEntity, {
	albedoColor: Color4.Blue(),
})
```


> [!WARNING]
> **📔 Nota**: Los `Material` component debe importarse mediante
> 
> > `import { Material } from "@dcl/sdk/ecs"`
> 
> Consulta [Importaciones](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes/#imports) para ver cómo manejarlos fácilmente.


## Colores del Material

Dale a un material un color plano. En un Material PBR, estableces el `albedoColor` field. Los colores Albedo responden a la luz y pueden incluir matices.

Los valores de color son de tipo `Color4`, compuesto por *r*, *g* y *b* valores (rojo, verde y azul). Cada uno acepta valores entre 0 y 1. Al establecer distintos valores para ellos, puedes componer cualquier color visible. Para negro, establece los tres en 0. Para blanco, establece todos en 1.


> [!WARNING]
> **📔 Nota**: Si estableces cualquier color en `albedoColor` a un valor mayor que *1*, aparecerá como *emisivo*, con más intensidad cuanto mayor sea el valor. Así que, por ejemplo, `{r: 15, g: 0, b: 0}` produce un resplandor rojo muy brillante.


Consulta [tipos de color](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/color-types/) para más detalles sobre cómo establecer colores.

También puedes editar los siguientes campos en un Material PBR para ajustar cómo se percibe su color:

* *emissiveColor*: El color emitido por el material.
* *reflectivityColor*: también conocido como *Color especular* en otra nomenclatura.

Para crear un material de color plano que no se vea afectado por la luz ni las sombras del entorno, crea un basic material en lugar de un PBR material.

```ts
Material.setBasicMaterial(myEntity, {
	diffuseColor: Color4.Black(),
})
```

## Uso de textures

Establece un archivo de imagen como texture en un material configurando el `texture` parámetro.

```ts
//Crear Entity y asignar forma
const meshEntity = engine.addEntity()
Transform.create(meshEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(meshEntity)

//Crear Material y configurar sus campos
Material.setPbrMaterial(meshEntity, {
	texture: Material.Texture.Common({
		src: 'assets/materials/wood.png',
	}),
})
```

En el ejemplo anterior, la imagen del material se encuentra en una `assets/materials` carpeta, que se encuentra en la raíz de la carpeta del proyecto de la Scene.


> [!NOTE]
> **💡 Consejo**: Recomendamos mantener los archivos de imagen de texture en algún lugar de la `/assets` carpeta dentro de tu Scene.


Al crear una texture, también puedes pasar parámetros adicionales:

* `filterMode`: Determina cómo se estiran o comprimen los píxeles de la texture cuando se renderiza. Toma un valor del `TextureFilterMode` enum. Consulta [Escalado de Texture](#texture-scaling).
* `wrapMode`: Determina cómo se aplica en mosaico una texture sobre un objeto. Toma un valor del `TextureWrapMode` enum. Consulta [Ajuste de Texture](#texture-wrapping).

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'assets/materials/wood.png',
		filterMode: TextureFilterMode.TFM_BILINEAR,
		wrapMode: TextureWrapMode.TWM_CLAMP,
	}),
})
```

Para crear una texture que no se vea afectada por la luz ni las sombras del entorno, crea un material básico en lugar de un Material PBR.

```ts
Material.setBasicMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'assets/materials/wood.png',
	}),
})
```

### Textures desde una URL externa

Puedes apuntar la texture de tu material a una URL externa en lugar de a una ruta interna en el proyecto de la Scene.

```ts
Material.setBasicMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'https://wearable-api.decentraland.org/v2/collections/community_contest/wearables/cw_tuxedo_tshirt_upper_body/thumbnail',
	}),
})
```

La URL debe comenzar con `https`, `http` Las URLs no son compatibles.

### Ajuste de Texture

Puedes definir cómo se alinea una texture con una superficie. De forma predeterminada, la texture se estira para ocupar la superficie una vez, pero puedes escalarla y desplazarla.

Los siguientes campos están disponibles en todas las textures:

* `offset`: Desplaza la texture para cambiar su alineación. El valor es un Vector2, donde ambos ejes van de 0 a 1, y 1 es el ancho o alto completo de la texture.
* `tiling`: Escala la texture. El valor predeterminado es el Vector2 `[1, 1]`, lo que hace que la imagen se repita una vez cubriendo toda la superficie.
* `TextureWrapMode`: Determina qué sucede si el mosaico de la imagen no cubre toda la superficie. Esta propiedad toma sus valores del `TextureWrapMode` enum, que permite los siguientes valores:
  * `TextureWrapMode.TWM_CLAMP`: La texture solo se muestra una vez con el tamaño especificado. El resto de la superficie del mesh queda transparente. El valor de `tiling` se ignora.
  * `TextureWrapMode.TWM_REPEAT`: La texture se repite tantas veces como quepa en el mesh, usando el tamaño especificado.
  * `TextureWrapMode.TWM_MIRROR`: Al igual que en wrap, la texture se repite tantas veces como quepa, pero la orientación de estas repeticiones se refleja en espejo.

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
> **📔 Nota**: Los `offset` y `tiling` estas propiedades solo son compatibles con el cliente de escritorio DCL 2.0.


Usa esta función para cubrir una gran superficie con un patrón en mosaico. Por ejemplo, repite la siguiente imagen:

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

En el ejemplo de abajo, la texture usa un *espejo* modo de ajuste, y cada repetición de la texture ocupa solo 1/4 de la superficie. Esto significa que veremos 4 copias de la imagen, reflejadas entre sí en ambos ejes.

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'materials/atlas.png',
		wrapMode: TextureWrapMode.TWM_MIRROR,
		tiling: Vector2.create(0.25, 0.25),
	}),
})
```

### Tweens de texture

Haz que una texture se deslice suavemente usando un `Tween` component, configurado con el `TextureMove` modo. El tween cambia gradualmente el valor de los `offset` o los `tiling` propiedades de una texture a lo largo de un período de tiempo, de forma fluida y optimizada.


> [!WARNING]
> **📔 Nota**: Los Texture Tweens son una función que solo es compatible con el cliente de escritorio DCL 2.0.


Usa la `Tween` component con el `setTextureMove` función para mover la texture entre dos posiciones.

```ts
Tween.setTextureMove(myEntity, Vector2.create(0, 0), Vector2.create(1, 0), 2000)
```

El texture tween toma la siguiente información:

* `entity`: La Entity de la que mover la texture
* `inicio`: Un Vector2 para la posición de inicio
* `fin`: Un Vector2 para la posición final
* `duración`: Cuántos milisegundos tarda en moverse entre las dos posiciones

También está disponible este otro parámetro opcional:

* `movementType`: define si el movimiento será sobre el `offset` o los `tiling` campo. De forma predeterminada usa `offset`.
* `easingFunction`: La curva de la tasa de cambio a lo largo del tiempo; el valor predeterminado es `EasingFunction.EF_LINEAR`. Otros valores hacen que el cambio acelere y/o desacelere a diferentes ritmos.

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

El ejemplo anterior ejecuta un tween que dura 1 segundo y mueve la texture solo una vez. Para lograr un movimiento continuo, por ejemplo para simular la caída de una cascada, necesitas usar `setTextureMoveContinuous`.

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

El ejemplo anterior usa `setTextureMoveContinuous`, con una dirección de `(0, 1)`, y una velocidad de 1 unidad por segundo.

El texture tween continuo toma la siguiente información:

* `entity`: La Entity de la que mover la texture
* `dirección`: Un Vector2 para el movimiento
* `velocidad`: Cuántas unidades por segundo se moverá la Entity

También están disponibles estos otros parámetros opcionales:

* `movementType`: define si el movimiento será sobre el campo offset o el campo tiling. De forma predeterminada usa offset.
* `duración`: Cuántos milisegundos mantener el movimiento. Después de este tiempo, el movimiento se detendrá.

**Secuencias complejas de tween**

También puedes hacer que los movimientos de la texture sigan una secuencia compleja con tantos pasos como quieras. Usa el `sequence` campo para listar tantos tweens como quieras; se ejecutarán secuencialmente después del primer tween descrito en el `Tween` componente.

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

Ten en cuenta que, al definir un tween dentro de un TweenSequence, necesitas usar el formato más explícito de `Tween.Mode.TextureMove` para definir el tween.

### Textures de varias capas

Puedes usar varios archivos de imagen como capas para componer textures más realistas, por ejemplo incluyendo una `bumpTexture` y una `emissiveTexture`.

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

El `bumpTexture` puede simular bultos y arrugas en una superficie, modificando cómo se comportan las normals de la superficie en cada píxel.

![](/docs/_remote/b02dcfc858055419931f396c.webp)

El `emissiveTexture` puede acentuar el brillo en ciertas partes de un material, para lograr efectos muy interesantes.

**Establecer UVs**

Otra alternativa para cambiar la escala o la alineación de una texture es configurar *uv* propiedades en el [component MeshRenderer](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/shape-components/).

Estableces *u* y *v* coordenadas en la imagen 2D de la texture para que correspondan a los vertices de la forma. Cuantos más vertices tenga la Entity, más *uv* coordenadas deben definirse en la texture; por ejemplo, un plano necesita tener 8 *uv* puntos definidos, 4 para cada una de sus dos caras.

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

El siguiente ejemplo incluye una función que simplifica la configuración de las uvs. La `setUVs` función definida aquí recibe un número de filas y columnas como parámetros, y establece las uvs para que la imagen de la texture se repita un número específico de veces.

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
		//Lado norte del plano sin rotar
		0, // esquina inferior izquierda
		0,

		cols, // esquina inferior derecha
		0,

		cols, // esquina superior derecha
		rows,

		0, // esquina superior izquierda
		rows,

		//Lado sur del plano sin rotar
		cols, // esquina inferior derecha
		0,

		0, // esquina inferior izquierda
		0,

		0, // esquina superior izquierda
		rows,

		cols, // esquina superior derecha
		rows,
	]
}
```

Para establecer las UVs de una `caja` mesh shape, se aplica la misma estructura. Cada una de las 6 caras del cubo toma 4 pares de coordenadas, uno para cada esquina. Todos estos 48 valores se listan en un solo array.


> [!WARNING]
> **📔 Nota**: Las propiedades UV solo están disponibles actualmente en `plano` y en `caja` formas. Además, *uv* los valores afectan por igual a todas las capas de texture, ya que se establecen en la *forma*.


### Escalado de Texture

Cuando las textures se estiran o se encogen a un tamaño distinto del de la imagen original de texture, a veces esto puede crear artefactos. En un entorno 3D, los efectos de la perspectiva provocan esto de forma natural. Existen diversos [filtrado de texture](https://en.wikipedia.org/wiki/Texture_filtering) algoritmos que existen para compensar esto de diferentes maneras.

El `Material` objeto usa el *bilinear* algoritmo de forma predeterminada, pero te permite configurarlo para usar el *vecino más cercano* o *trilineal* algoritmos en su lugar configurando el `filterMode` property de la texture. Toma un valor del `TextureFilterMode` enum:

* `TextureFilterMode.TFM_POINT`: Usa un algoritmo de "vecino más cercano". Esta configuración es ideal para gráficos de estilo pixel art, ya que los contornos permanecerán bien definidos cuando la texture se vea más grande en pantalla en lugar de verse borrosa.
* `TextureFilterMode.TFM_BILINEAR`: Usa un algoritmo bilinear para estimar el color de cada píxel.
* `TextureFilterMode.TFM_TRILINEAR`: Usa un algoritmo trilineal para estimar el color de cada píxel.

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'materials/atlas.png',
		filterMode: TextureFilterMode.TFM_BILINEAR,
	}),
})
```

## Materials sin iluminación

La mayoría de las veces querrás que los materials de tu Scene se vean afectados por las condiciones de iluminación, incluidas las sombras y el tinte por los cambios de tono de las distintas horas del día. Pero en otros casos quizá quieras mostrar los colores en su estado puro. Esto es útil al reproducir videos, o también para marcadores abstractos que necesitan destacar y que están pensados para señalar pistas al jugador.

Para crear un material sin iluminación, usa `Material.setBasicMaterial`. Los basic materials no tienen todas las mismas propiedades que los PBR materials; solo tienen las esenciales:

* `diffuseColor`: Color4 para el color
* `texture`: Texture
* `alphaTexture`: Texture separada para la capa de transparencia
* `alphaTest`: Umbral para lograr transparencia basado en el color de la texture
* `castShadows`: Si es false, no se proyectan sombras sobre otras Entities en la Scene.

```ts
Material.setBasicMaterial(screen, {
	diffuseColor: Color4.Red(),
})
```

## Retratos de Avatar

Para mostrar una imagen en miniatura de cualquier jugador, usa `Material.Texture.Avatar` al establecer la texture de tu material, pasando la dirección de un jugador existente. Esto crea una texture a partir de una imagen 256x256 del jugador, mostrando la cabeza y los hombros. El jugador se muestra con el conjunto de wearables que el servidor actual registró por última vez.

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Avatar({
		userId: '0x517....',
	}),
})
```

![](/docs/_remote/3907d796a3f7f47add13d85a.webp)

Puedes obtener el retrato de cualquier jugador de Decentraland, incluso si no está conectado en este momento y aunque no tenga un nombre de Decentraland reclamado.

Las siguientes propiedades son compatibles dentro del objeto que pasas como argumento:

* `userId`: ID del usuario cuyo perfil quieres mostrar
* `filterMode`: Determina cómo se estiran o comprimen los píxeles de la texture cuando se renderiza. Toma un valor del `TextureFilterMode` enum. Consulta [Escalado de Texture](#texture-scaling).
* `wrapMode`: Determina cómo se aplica en mosaico una texture sobre un objeto. Toma un valor del `TextureWrapMode` enum. Consulta [Ajuste de Texture](#texture-wrapping).

## Materials transparentes

Para hacer transparente un material de color plano, simplemente define el color como un `Color4`, y establece el cuarto valor en algo entre *0* y *1*. Cuanto más cerca de *1*, más opaco será.

```typescript
let transparentRed = Color4.create(1, 0, 0, 0.5)

Material.setPbrMaterial(meshEntity, {
	albedoColor: transparentRed,
})
```

Si un material usa una texture .png que incluye transparencia, será opaco por defecto, pero puedes activar su transparencia estableciendo el `transparencyMode` para `MaterialTransparencyMode.MTM_ALPHA_BLEND`.

![](/docs/_remote/98a85f0b352d710c0c48036c.webp)

```typescript
Material.setPbrMaterial(floor, {
	texture: Material.Texture.Common({
		src: 'assets/scene/transparent-image.png',
	}),
	transparencyMode: MaterialTransparencyMode.MTM_ALPHA_BLEND,
})
```

El `transparencyMode` puede tener los siguientes valores:

* `MaterialTransparencyMode.MTM_OPAQUE`: Sin transparencia en absoluto
* `MaterialTransparencyMode.MTM_ALPHA_TEST`: Cada píxel es completamente opaco o completamente transparente, según un umbral.
* `MaterialTransparencyMode.MTM_ALPHA_BLEND`: Son posibles valores intermedios según el valor de cada píxel.
* `MaterialTransparencyMode.MTM_ALPHA_TEST_AND_ALPHA_BLEND`: Usa una combinación de ambos métodos.
* `MaterialTransparencyMode.MTM_AUTO`: Determina el método según la textura proporcionada.


> [!WARNING]
> **📔 Nota**: Configúralo siempre `transparencyMode` tú mismo cuando quieras que se muestre el propio canal alfa de una textura. En el cliente de escritorio, el valor predeterminado `MTM_AUTO` modo ignora el alfa de la textura y solo mezcla cuando el `albedoColor` alfa está por debajo de 1, así que una imagen transparente `.png` sale completamente opaca. Con `MTM_ALPHA_BLEND` o `MTM_ALPHA_TEST`, la transparencia es el alfa de la textura multiplicado por el `albedoColor` alfa.


Si estableces el `transparencyMode` para `MaterialTransparencyMode.MTM_ALPHA_TEST`, puedes ajustar con precisión el umbral usado para determinar si cada píxel es transparente o no. Establece el `alphaTest` propiedad entre *0* y *1*. De forma predeterminada, su valor es *0.5*.

```ts
// Usando prueba alfa
Material.setPbrMaterial(meshEntity1, {
	texture: Material.Texture.Common({
		src: 'images/myTexture.png',
	}),
	transparencyMode: MaterialTransparencyMode.MTM_ALPHA_TEST,
	alphaTest: 1,
})
```

Al usar un [material unlit](#unlit-materials), puedes añadir un `alphaTexture` para hacer transparentes solo ciertas regiones del material, según una textura.


> [!WARNING]
> **📔 Nota**Pinta la máscara de blanco donde el material deba mostrarse y de negro donde deba desaparecer. El cliente lee el brillo de los canales de color de la máscara, no su canal alfa, así que cualquier canal con valor completo cuenta como opaco: una máscara roja pura también es opaca. No reutilices tu textura de color como su propia máscara, o cada píxel oscuro en ella se volverá transparente y se comerá tus contornos y sombreado.


![](/docs/_remote/9b242e1d8862ee66017fbceb.webp)

```ts
// Recorta una pantalla hasta convertirla en un círculo con una imagen de máscara
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

Otras dos cosas que debes saber sobre `alphaTexture` en el cliente de escritorio:

* **Solo funciona en materiales unlit.** `Material.setPbrMaterial` acepta un `alphaTexture` pero nunca lo usa. Para un material PBR, coloca la transparencia en el propio canal alfa de la textura de color y establece `transparencyMode` en su lugar.
* **Un material unlit sin un `alphaTexture` es completamente opaco**y `alphaTest` no tiene efecto en él. Un `texture` plus `alphaTest` por sí sola dibuja el color de los píxeles transparentes, que normalmente es negro.

Esto puede usarse de formas muy interesantes junto con videos. Consulta [video en reproducción](/docs/creator/content-creator-es/scenes-sdk7/media/video-playing/).

## Video en reproducción

Para transmitir un video desde una URL a un material, o reproducir un video desde un archivo almacenado en la escena, consulta [video en reproducción](/docs/creator/content-creator-es/scenes-sdk7/media/video-playing/).

El video se usa como una textura en un material; puedes establecer cualquiera de las otras propiedades de los materiales para alterar el aspecto de la pantalla de video.

## Sintaxis avanzada

La sintaxis completa para crear un `Materials` componente, sin ningún helper para simplificarlo, se ve así:

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

Así interpreta el protocolo base los componentes Materials. Las funciones helper abstraen esto y exponen una sintaxis más amigable, pero detrás de escena generan esta sintaxis.

El `$case` Los campos te permiten especificar uno de los tipos permitidos. Cada tipo admite un conjunto diferente de parámetros. Hay dos niveles de `$case` aquí:

* El `material` campo admite los valores `pbr` y `unlit`, para definir el tipo de material.
* El `tex` campo dentro de una textura admite los siguientes valores, para definir el tipo de textura:
  * `texture`
  * `avatarTexture`
  * `videoTexture`

Según el valor de `$case`, es válido definir el objeto para el tipo correspondiente, pasando cualquier propiedad relevante.

Para añadir un `Material` componente a una Entity que potencialmente ya tenga una instancia de este componente, usa `Material.createOrReplace()`. Las funciones helper como `Material.setPbrMaterial()` se encargan de sobrescribir las instancias existentes del componente, pero ejecutar `Material.create()` en una Entity que ya tiene este componente devuelve un error.

## Modificar materiales glTF

Usa la `GltfNodeModifiers` componente para modificar los materiales de un *glTF* modelo. Este componente te permite sobrescribir los materiales de un *glTF* modelo con tus propios materiales. Puedes usar cualquiera de las propiedades del `Material` componente, incluyendo texture, video texture, materiales unlit, etc.

Hay dos formas de usar el `GltfNodeModifiers` component:

* Modifica el material de todo el modelo dejando el `path` propiedad como una cadena vacía.
* Modifica el material de un nodo específico del modelo (o de varios nodos) estableciendo la `path` propiedad al path del nodo.

### Modificar el material de todo el modelo

El siguiente ejemplo muestra cómo modificar el material de un *glTF* modelo. En este caso, el material de todo el modelo se modifica para que sea rojo.

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

El `GltfNodeModifiers` componente tiene las siguientes propiedades:

* `modifiers`: Un array de modifiers. Cada modifier tiene las siguientes propiedades:
  * `path`: El path al nodo del modelo que se va a modificar.
  * `material`: El material a usar.

El `path` propiedad es una cadena que representa el path al nodo del *glTF* modelo que se va a modificar. Si quieres modificar el material de todo el modelo, puedes usar una cadena vacía. Si quieres modificar el material de un nodo específico, puedes usar el path al nodo. El path debe apuntar a un nodo mesh, no a un nodo vertex.


> [!NOTE]
> **💡 Consejo**: Puedes usar la [app Babylon Sandbox](https://sandbox.babylonjs.com/) para inspeccionar el *glTF* modelo y encontrar el path del nodo que quieres modificar.
> 
> Sin embargo, en algunos modelos, Babylon Sandbox puede listar paths que pertenecen a vertexes en lugar de meshes, lo que no funcionará. Si intentas usar un path que no sea válido, la consola de la escena mostrará un mensaje de error que incluye la lista completa de paths válidos de ese modelo.


El `material` propiedad es un objeto que representa el material a usar. Debe escribirse usando la [sintaxis avanzada](#advanced-syntax) para materials, como se muestra en el ejemplo anterior. Las funciones helper como `Material.setPbrMaterial()` no pueden usarse aquí.

### Modificar el material de un nodo específico del modelo

El siguiente ejemplo muestra cómo modificar el material de un nodo específico en el *glTF* modelo. En este caso, el material de la cabeza se modifica para usar una textura alternativa.

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

Un `GltfNodeModifiers` puede contener varios modifiers, cada uno modificando un nodo diferente del modelo. El siguiente ejemplo muestra cómo modificar el material de la cabeza y el cuerpo de un *glTF* modelo.

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

### Modificar campos de un material existente

El componente Material proporciona una interfaz simplificada para acceder y modificar las propiedades del componente Material. Elimina la necesidad de navegar por estructuras de unión profundamente anidadas (`PBR` vs `Unlit`, `texture` vs `avatarTexture` vs `videoTexture`), lo que hace que la manipulación de materials sea más intuitiva y menos propensa a errores.

Los siguientes métodos permiten manipular fácilmente el `Material` component:

* `Material.getFlat(entity: Entity): ReadonlyFlatMaterial`: Devuelve un **solo lectura** `FlatMaterial` objeto de acceso que proporciona acceso directo de lectura a las propiedades del material. Devolverá un error si la Entity no tiene un `Material` componente. Para evitar esto, se pueden usar los siguientes métodos como alternativa.

```ts
const src = Material.getFlat(entity).texture.src
```

* `Material.getFlatOrNull(entity: Entity): ReadonlyFlatMaterial | null`: Devuelve un **solo lectura o null**, dependiendo de si la Entity tiene o no un `Material` componente.

```ts
const src = Material.getFlatOrNull(entity)?.texture.src
```

* `Material.getFlatMutable(entity: Entity): FlatMaterial`: Devuelve un **lectura y escritura** `FlatMaterial`objeto que permite modificar sus propiedades. Lanzará un error si la Entity sobre la que se llama no tiene el `Material` componente.

```ts
const src = Material.getFlatMutable(entity).texture.src
```

* `Material.getFlatMutableOrNull(entity: Entity): FlatMaterial | null`: Devuelve un **lectura y escritura o null** `FlatMaterial` objeto que permite modificar sus propiedades. En caso de que la Entity no tenga un `componente Material`, devolverá `null`.

```ts
const flatMaterial = Material.getFlatMutableOrNull(entity)
if (flatMaterial) {
	flatMaterial.texture.src = myNewTextureFile
}
```

### Eliminar sombras de un modelo glTF

Para eliminar las sombras de un *glTF* modelo, puedes establecer el `castShadows` propiedad en `false` en el `GltfNodeModifiers` objeto. Esto conserva el material original del modelo, pero impide que proyecte sombras. Esto es útil para modelos que no están pensados para proyectar sombras, como haces de luz.

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
> **💡 Consejo**: Las texturas usadas en materiales también pueden aplicarse a efectos de partículas. Consulta [Sistema de partículas](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/particle-system/) para más detalles sobre el uso de texturas personalizadas y sprite sheets con partículas.



> [!NOTE]
> **💡 Consejo**: Para ver ejemplos prácticos de materials, consulta el [`52,-52-testing-gallery`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/52,-52-testing-gallery) escena de prueba, que muestra combinaciones PBR de metallic/roughness/emissive/normal-map una al lado de la otra, [`74,-8-gltfnodemodifier`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/74,-8-gltfnodemodifier), que sobrescribe materiales PBR y unlit en nodos GLTF individuales, y [`0,3-texture-movement`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,3-texture-movement), que anima el desplazamiento y el teselado de la textura.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
