> Crea tu primera escena de Decentraland con Creator Hub, el SDK y vibe coding asistido por IA usando las SDK Skills oficiales.

# Inicio rápido de SDK

Este tutorial te guía paso a paso para crear tu primera escena, combinando las distintas herramientas que tienes disponibles: el Scene Editor visual de Creator Hub, código escrito a mano usando el Decentraland SDK y la asistencia de IA **vibe coding**. Usarás las tres juntas y aprenderás cuándo brilla cada una.

## ¿Cómo creo una escena de Decentraland?

1. **Instala el** [**Creator Hub**](https://decentraland.org/download/creator-hub), la aplicación oficial de escritorio para crear, previsualizar y publicar escenas. Es la herramienta recomendada para creadores de todos los niveles de habilidad.
2. **Si usas un asistente de código con IA** (como Claude Code, Cursor o Copilot), instala el oficial [Decentraland SDK Skills](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/) para que conozca patrones verificados de SDK7: `npx skills add decentraland/sdk-skills`
3. **El** [**CLI**](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/using-the-cli/) es una alternativa para usuarios avanzados y flujos de trabajo automatizados.

El resto de este tutorial recorre ese camino recomendado paso a paso.

## Instala Creator Hub

Creator Hub te permite crear, previsualizar y desplegar escenas de Decentraland. Descarga Creator Hub [aquí](https://decentraland.org/download/creator-hub).

Para editar el código de tu escena, también necesitas un editor de código. [Visual Studio Code](https://code.visualstudio.com/) y [Cursor](https://www.cursor.com/) son ambas excelentes opciones, pero cualquier editor de código funciona.

Lee la [guía de instalación](/docs/creator/content-creator-es/scene-editor/primeros-pasos/editor-installation/) para más detalles.

## Crear tu primera escena

1. Abre Creator Hub.
2. Selecciona la **escenas** pestaña y haz clic en **Nueva escena**.

   ![](/docs/_remote/efc0e8d68c33532f7a840f5e.webp)
3. Elige una plantilla inicial. Para este ejercicio, elige **Empty Scene**.

Este paso puede tardar un par de minutos. Rellena tu carpeta con el conjunto predeterminado de archivos para una escena básica. Cuando termine, verás la cuadrícula vacía de tu escena.

## Añade elementos de los asset packs

Explora los **Asset packs** en la sección inferior del Scene Editor, y arrastra un par de elementos a tu escena. Por ahora sirve cualquier elemento.

![](/docs/_remote/c5121b56db02ac73b992d99e.webp)

Los elementos que ya colocaste se pueden hacer clic y arrastrar para reubicarlos. Ver [Aspectos esenciales del Scene Editor](/docs/creator/content-creator-es/scene-editor/primeros-pasos/scene-editor-essentials/#position-items) para más detalles.


> [!NOTE]
> **💡 Consejo**: Cubre toda la escena con un elemento Ground. Los elementos de tipo **Ground** tienen un icono de cubeta de pintura. Si arrastras uno de estos a tu scene, cubrirá todo el Ground de tu scene con copias de este item.
> 
> <img src="/docs/_remote/e120bb99677fe9cb301458fb.webp" alt="Ground" data-size="original">


## Ejecuta una previsualización

Haz clic en el **Reproducir** botón en el menú superior para cargar tu escena dentro de Decentraland. Ahora puedes explorar la escena como un avatar de Decentraland.

![](/docs/_remote/77e30200d3905dd6bed10959.webp)

Puedes mantener abierta la ventana de previsualización mientras trabajas: se actualiza cada vez que haces un cambio. Lee más en [previsualizar una escena](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/preview-scene/).

## Assets 3D personalizados

Descarga este modelo 3D de un aguacate en *glb* formato desde el siguiente [enlace](https://github.com/decentraland-scenes/avocado/raw/main/avocado-glb.zip) y descomprímelo.

![](/docs/_remote/54ed4744f3c2c873fc597b60.webp)

Arrastra el **avocado.glb** archivo desde tu explorador de archivos al panel inferior del Scene Editor (el mismo panel que contiene las **Asset Packs** y **Local Assets** pestañas) y haz clic en **Import**.

![](/docs/_remote/63d04fd8ef301b90c747fa37.webp)

Ahora puedes encontrar el **avocado.glb** modelo en la **Local Assets** pestaña, dentro de la carpeta **Scene** Arrastra el archivo a tu escena, como cualquier elemento de los Asset Packs.

![](/docs/_remote/23bd93a0c282b7207519dc58.webp)

## Editar el código de la escena

Haz clic en el **<> Code** botón en el menú superior para abrir el proyecto de tu escena en tu editor de código.

![](/docs/_remote/b1e812c7bf7175e15d141046.webp)


> [!WARNING]
> **📔 Nota**: Si no se abre nada, asegúrate de tener un editor de código como [Visual Studio Code](https://code.visualstudio.com/) o [Cursor](https://www.cursor.com/) instalado.


En el margen izquierdo de tu editor de código puedes navegar por los archivos y la estructura de carpetas de tu proyecto. Abre el `index.ts` archivo dentro de la `src` carpeta. Su contenido debería verse así:

```ts
import {} from '@dcl/sdk/math'
import { engine } from '@dcl/sdk/ecs'

export function main() {}
```

Las escenas de Decentraland están escritas en [TypeScript](https://www.typescriptlang.org/), usando el Decentraland SDK: una biblioteca con todo lo que necesitas para posicionar contenido 3D, añadir interactividad y controlar lo que ocurre en tu escena.

Este archivo define una función llamada `main()`, que es el punto de entrada de la escena: cualquier código que pongas ahí se ejecuta cuando la escena se carga por primera vez.

Ya arrastraste un aguacate a la escena visualmente. Ahora añadamos un segundo, esta vez escribiendo código. Sustituye el contenido completo de tu `index.ts` archivo por lo siguiente:

```ts
import { Vector3 } from '@dcl/sdk/math'
import { engine, Transform, GltfContainer } from '@dcl/sdk/ecs'

export function main() {
	// crear una Entity nueva
	let avocado2 = engine.addEntity()

	// darle un Transform
	Transform.create(avocado2, {
		position: Vector3.create(8, 0, 8),
	})

	// darle un GLTF
	GltfContainer.create(avocado2, {
		src: 'assets/scene/avocado.glb',
	})
}
```

Estas líneas crean una nueva [entity](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/entities-components/), le dan un [shape](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/shape-components/) basado en el modelo 3D que descargaste, y [configuran su posición](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/entity-positioning/) a través de la **Transform** component.

Como regla general, el código que escribes en `index.ts` debería estar todo dentro de `main()` (o en otras funciones que están referenciadas indirectamente por main). Ver [Ciclo de vida de la scene](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes/#scene-lifecycle) para más detalles.

Ejecuta la previsualización de la escena: ahora deberías ver dos aguacates, el que añadiste en el Scene Editor y el que añadiste mediante código.

![](/docs/_remote/9e7996b651530bdb72380bb3.webp)


> [!WARNING]
> **📔 Nota**: El segundo aguacate existe **solo en tu código**. Aparece cuando ejecutas la escena, pero el lienzo del Scene Editor y el árbol de entidades no pueden mostrar las entidades creadas en `index.ts`. No te alarmes si no lo ves en el editor. Esto es algo clave que debes tener en cuenta cuando combinas edición visual con código.


Ambos aguacates están construidos con las mismas piezas. Selecciona el **primero** aguacate en el Scene Editor: el panel de propiedades muestra un **Transform** y una **GltfContainer** componente con los mismos tipos de campos que acabas de escribir en código. Son dos vistas de la misma cosa.

![](/docs/_remote/90ea2c66a117f593fd4e77cc.webp)

## Añade interactividad con un Script

Vamos a hacer que un aguacate responda al jugador. Podríamos hacerlo añadiendo código en el `index.ts` que ya estábamos editando, pero en su lugar usemos el **Script component**: una forma de adjuntar código directamente a un item en el Scene Editor. Mantiene el comportamiento de cada item autocontenido, e incluso puedes reutilizar el mismo script en varios items.

1. En el Scene Editor, selecciona el primer aguacate (el que arrastraste; recuerda, el aguacate solo por código no es visible aquí).
2. Haz clic en el **+** botón en la parte superior del panel de propiedades y selecciona **Script** para añadir un componente Script.
3. Haz clic **+ Crear nuevo Script** y llámalo `AvocadoScript`.

![](/docs/_remote/73d08b2cfd32963d68a7790e.webp)

4. Haz clic en el **<> Code** botón del componente Script para abrir el nuevo archivo en tu editor de código.

El script es una clase con tres partes principales (comentarios recortados por brevedad):

```ts
import { engine, Entity } from '@dcl/sdk/ecs'
import {} from '@dcl/sdk/math'

export class AvocadoScript {
	constructor(
		public src: string, // NO ELIMINAR
		public entity: Entity, // NO ELIMINAR
		// Agrega tus inputs personalizados abajo
	) {}

	start() {
		// Se llama una vez, cuando la escena se carga
	}

	update(dt: number) {
		// Se llama en cada frame
	}
}
```

* El **constructor** define parámetros que aparecen como campos editables en el componente Script en Creator Hub. No elimines `src` o `entity`.
* **`start()`** se ejecuta **una vez**, cuando la escena se carga. Úsalo para la configuración inicial: crear Components, registrar manejadores de clic, etc.
* **`update(dt)`** se ejecuta en **cada frame** del juego, aproximadamente 30 veces por segundo. Úsalo para comportamiento continuo, como el movimiento. `dt` te indica cuántos segundos han pasado desde el último frame.

Dentro de la clase, `this.entity` siempre se refiere a la entity que contiene el componente Script: en este caso, tu aguacate. Esto es lo que hace que los scripts sean reutilizables: adjunta el mismo script a diez items, y cada uno actúa sobre sí mismo.

### Crea tu primera interacción con vibe coding

Puedes escribir el siguiente código a mano, pero este es un gran momento para probar **vibe coding**: describirle a un asistente de IA lo que quieres y dejar que escriba el código. La mayoría de los editores de código tienen uno integrado, como el chat de Cursor, GitHub Copilot en VS Code, o [Claude Code](https://claude.com/product/claude-code). También puedes escribir prompts desde fuera de tu editor de código, como con Claude Desktop o Claude Code en la línea de comandos.

Antes de tu primer prompt, instala las skills del Decentraland SDK, para que la IA conozca los patrones del SDK y cometa muchos menos errores:

```bash
npx skills add decentraland/sdk-skills
```

Estas skills son documentos de referencia que la IA consulta cuando no sabe bien cómo hacer algo con el SDK, así escribe código correcto en vez de adivinar. Consulta [Vibe Coding con IA](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/) las opciones de configuración y consejos de prompting.

Ahora prueba con un prompt como este:

> En AvocadoScript.ts, haz que el aguacate registre un mensaje en la consola cuando el jugador haga clic en él.

Deberías acabar con algo como esto (o pegarlo tú mismo):

```ts
import { engine, Entity, pointerEventsSystem, InputAction } from '@dcl/sdk/ecs'
import {} from '@dcl/sdk/math'

export class AvocadoScript {
	constructor(
		public src: string,
		public entity: Entity,
	) {}

	start() {
		pointerEventsSystem.onPointerDown(
			{
				entity: this.entity,
				opts: { button: InputAction.IA_POINTER },
			},
			() => {
				console.log('CLICKED AVOCADO')
			}
		)
	}

	update(dt: number) {}
}
```

Añadió algo de comportamiento de clic dentro de la `start()` función. Solo necesitamos definir el comportamiento de clic una vez, y reaccionará cada vez que el jugador haga clic en el item. La `pointerEventsSystem.onPointerDown()` declaración define tres cosas:

* Qué `entity` los eventos de clic funcionan sobre: aquí `this.entity`, el aguacate que contiene el script.
* Un `opts` objeto: qué botón usar y otros argumentos opcionales que no estamos usando ahora.
* Una función que se ejecuta cada vez que se hace clic en la entity.

Para ver el mensaje registrado, ejecuta la previsualización y abre la consola haciendo clic en el ![](/docs/_remote/92e8eb247fcf289c2feff8f8.webp) icono en la esquina superior derecha. También puedes alternarlo presionando la tecla **\`** . Cada vez que hagas clic en el aguacate, verás aparecer una nueva línea:

![](/docs/_remote/609636cba47e8f304bc52855.webp)


> [!WARNING]
> **📔 Nota**: Para que una entity sea clicable, debe tener una geometría collider. El modelo usado aquí ya incluye una. Ver [Colliders](/docs/creator/content-creator-es/modelado-y-animaciones-3d/colliders/) para ver soluciones alternativas para modelos que no la tienen.


### Haz desaparecer el aguacate

¡Vamos a hacer que el clic haga algo más emocionante ahora! Pídele a tu asistente de IA:

> Cuando se haga clic en el aguacate, haz que desaparezca. Quiero que desaparezca con una animación rebotante de encogimiento. También quiero que el hint que veo al apuntar al aguacate diga "Collect".

Deberías acabar con algo como esto:

```ts
import { engine, Entity, pointerEventsSystem, InputAction, Tween, EasingFunction } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

export class AvocadoScript {
	constructor(
		public src: string,
		public entity: Entity,
	) {}

	start() {
		pointerEventsSystem.onPointerDown(
			{
				entity: this.entity,
				opts: { button: InputAction.IA_POINTER, hoverText: 'Collect' },
			},
			() => {
				this.collect()
			}
		)
	}

	collect() {
		Tween.setScale(
			this.entity,
			Vector3.One(),
			Vector3.Zero(),
			500,
			EasingFunction.EF_EASEINBOUNCE
		)
	}

	update(dt: number) {}
}
```

Desglosemos lo que hizo la IA.

* Para hacer la animación rebotante, usó un **Tween**. Un tween describe una transición gradual de la posición, rotación o escala de una entity a lo largo del tiempo. Aquí `Tween.setScale()` encoge el aguacate desde su tamaño completo (`Vector3.One()`) hasta nada (`Vector3.Zero()`) en 500 milisegundos, usando una curva de easing rebotante. Aprende más sobre tweens en [mover entities](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/move-entities/).
* En lugar de escribir el tween directamente dentro de la función de clic, creó un `método` separado. Esto no es obligatorio (si tu IA puso el tween dentro de la función de clic, también funciona), pero es una buena práctica: los métodos mantienen el código legible y te permiten reutilizar la misma lógica desde distintos lugares.
* Para mostrar el hint "Collect" en el aguacate, lo configuró como el `hoverText` valor.

Ejecuta la previsualización y haz clic en el aguacate: debería desaparecer con estilo.


> [!WARNING]
> **📔 Nota**: El aguacate se encoge hasta un tamaño de 0, pero la entity sigue existiendo. Idealmente deberías eliminar la entity después de que termine el tween, para mantener tu escena más ligera. Ver [Al finalizar el tween](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/move-entities/#on-tween-finished).


### Ejecuta código en cada frame

Hasta ahora todo nuestro código se ejecutaba en `start()`. Vamos a usar `update()` para hacer que el aguacate gire continuamente. Sustituye el constructor y `update()` por:

```ts
	constructor(
		public src: string,
		public entity: Entity,
		public speed: number = 45,
	) {}

	update(dt: number) {
		const transform = Transform.getMutable(this.entity)
		transform.rotation = Quaternion.multiply(
			transform.rotation,
			Quaternion.fromAngleAxis(this.speed * dt, Vector3.Up())
		)
	}
```

También necesitarás añadir `Transform` y `Quaternion` a los imports en la parte superior del archivo, para que se vean así:

```ts
import { engine, Entity, pointerEventsSystem, InputAction, Tween, EasingFunction, Transform } from '@dcl/sdk/ecs'
import { Vector3, Quaternion } from '@dcl/sdk/math'
```


> [!NOTE]
> **💡 Consejo**: En lugar de editar los imports a mano, puedes hacer clic en los errores marcados por tu editor de código y dejar que los añada automáticamente.


En cada frame, esto rota el aguacate un poco más. Multiplicar por `dt` hace que el movimiento sea suave e independiente del frame rate: el aguacate gira a 45 grados por segundo, sin importar lo rápido que corra la máquina del player.

Como `speed` es un parámetro del constructor, también aparece como un campo en el componente Script en Creator Hub. Haz clic en el icono de refrescar en la parte superior derecha del componente Script para verlo, luego ajusta el valor sin tocar ningún código.

<img src="/docs/_remote/f1c901d70e456b61b41d362b.webp" alt="Botón Refresh" width="360">

Cada entity almacena su propio valor para el campo **Speed** , así que varios items pueden compartir el mismo script pero comportarse de forma distinta. Pruébalo: copia el aguacate con Ctrl + C y Ctrl + V, mueve la copia para que los dos no se superpongan, y asigna una velocidad distinta a cada uno. Ahora cada aguacate gira a su propio ritmo.

Los scripts pueden hacer mucho más, como exponer acciones que otros smart items puedan activar. Ver [Script component](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/script-component/) para el panorama completo.

## Referenciar un item desde el Scene Editor

El componente Script es la forma más fácil de dar comportamiento a un solo item, pero existe una alternativa: tu código en `index.ts` puede obtener cualquier item que hayas añadido visualmente, por nombre, usando `engine.getEntityOrNullByName()`. Esto es útil cuando una pieza de lógica involucra varios items, o cuando quieres tenerlo todo en un solo lugar. Usa el nombre que aparece en el [entity tree](/docs/creator/content-creator-es/scene-editor/primeros-pasos/scene-editor-essentials/#the-entity-tree).

En este ejemplo, usamos una entity llamada **Yellow Crate**. Puedes usar cualquier item, solo escribe su nombre exactamente como aparece en el entity tree.


> [!NOTE]
> **💡 Consejo**: Puedes renombrar entities haciendo clic derecho y seleccionando **Renombrar** en el entity tree.


```ts
export function main() {
	// código de avocado2 de antes
	// (...)

	const crate = engine.getEntityOrNullByName('Yellow Crate')

	if (crate) {
		pointerEventsSystem.onPointerDown(
			{
				entity: crate,
				opts: { button: InputAction.IA_POINTER, hoverText: 'Open' },
			},
			function () {
				console.log('CLICKED CRATE')
			}
		)
	}
}
```

Aquí `engine.getEntityOrNullByName()` obtiene una referencia a la entity llamada *Yellow Crate*. El `if (crate)` la comprobación se asegura de que la entity realmente exista en la escena; si no hay una entity con ese nombre, `crate` es `null`. Consulta [Elementos de referencia](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/reference-items/) para más información.


> [!NOTE]
> **💡 Consejo**: Todas las entities añadidas mediante el Scene Editor ya están cargadas cuando `main()` se ejecuta, así que es seguro referenciarlas ahí o en funciones llamadas indirectamente por `main()`.


## Más tutoriales

Lee [Codificación de escenas](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes/) para tener una comprensión general de cómo funcionan las escenas de Decentraland.

Para ver ejemplos creados con SDK7, consulta la [página de ejemplos](/docs/creator/content-creator-es/tutoriales-y-ejemplos/examples/), que contiene escenas y repositorios reutilizables.

Consulta el **Guía de desarrollo** sección para más instrucciones sobre cómo añadir contenido a tu escena.

## Conéctate con otros desarrolladores

Visita el [Discord de Decentraland](https://dcl.gg/discord) el [Discord de Decentraland DAO](https://discord.gg/bxHtcMxUs4) para unirte a una animada discusión sobre lo que es posible y cómo hacerlo en la **sección Creators** sección.

Para depurar cualquier problema, revisa las [Solución de problemas](/docs/creator/content-creator-es/scenes-sdk7/depuracion/troubleshooting/) y [debug](/docs/creator/content-creator-es/scenes-sdk7/depuracion/debug-in-preview/) secciones. Un asistente de IA con las [skills del SDK](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/) instaladas también es un gran compañero de depuración: pega el mensaje de error de la consola, o describe qué no se está comportando como esperabas, y normalmente puede encontrar el problema en tu código. Si no encuentras una solución, puedes publicar en la [categoría de soporte de SDK](https://forum.decentraland.org/c/support-sdk/11) en el Foro de Decentraland.

## Assets de arte 3D

Una buena experiencia tendrá un gran arte 3D que la acompañe. Si te interesa crear esos modelos 3D tú mismo, consulta la [sección de Modelado 3D](/docs/creator/content-creator-es/modelado-y-animaciones-3d/3d-models/). Pero si prefieres centrarte en la parte de código o diseño del juego, ¡no necesitas crear tus propios assets! Ver [Recursos útiles](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/useful-resources/) para bibliotecas de assets y herramientas de IA que puedes usar para obtener modelos 3D para tu escena.

## Publica tu scene

Si posees un NAME de Decentraland, un nombre ENS de ETH o LAND, o tienes permisos dados por alguien que sí los tiene, puedes subir tu escena a Decentraland. Ver [publicación](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing/).

## Otra información útil

* [Vibe Coding con IA](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/)
* [Flujo de trabajo de desarrollo](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/dev-workflow/)
* [Editar escenas](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/)
* [Aspectos esenciales del Scene Editor](/docs/creator/content-creator-es/scene-editor/primeros-pasos/scene-editor-essentials/)
* [Restricciones de diseño para juegos](/docs/creator/content-creator-es/scenes-sdk7/disenar-la-experiencia/design-games/)
* [modelado 3D](/docs/creator/content-creator-es/modelado-y-animaciones-3d/3d-models/)
* [Limitaciones de la escena](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/primeros-pasos/sdk-101.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
