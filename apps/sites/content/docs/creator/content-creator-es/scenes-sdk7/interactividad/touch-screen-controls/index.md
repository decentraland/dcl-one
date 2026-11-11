> Configura los controles táctiles nativos en pantalla para tu escena.

# Controles en pantalla

En el cliente móvil, los jugadores interactúan con tu escena a través de un conjunto de controles nativos en pantalla: un joystick virtual, una mira y un gamepad de botones. El `TouchScreenControls` componente permite que tu escena reorganice ese HUD: lo simplifique, oculte el joystick o la mira, cambie lo que hace el gran botón central, reemplace el glifo de un botón por tu propio icono, u oculte por completo los botones y los sustituya por tu propia UI.

<figure><img src="/docs/_remote/8414c098957fe0e8df7af46b.webp" alt="The default mobile HUD: virtual joystick on the left, gamepad buttons on the right"><figcaption><p>Los controles predeterminados en pantalla, antes de cualquier personalización</p></figcaption></figure>


> [!NOTE]
> El componente se aplica automáticamente mientras el jugador esté dentro de tu escena y vuelve a los valores predeterminados en el momento en que se va, así que las escenas que no lo usan no se ven afectadas. Solo afecta a plataformas con controles nativos en pantalla: no hace nada en escritorio y no tiene efecto en VR.


## Lo que se muestra por defecto

Desde SDK `7.32.0`, cada escena comienza con tres botones del gamepad visibles:

* `IA_JUMP`, el gran botón central
* `IA_POINTER`, el botón de interacción
* `IA_PRIMARY`, el botón E

`IA_SECONDARY` y `IA_ACTION_3` para `IA_ACTION_6` empiezan ocultos. Solo necesitas `TouchScreenControls` cuando quieras algo distinto a esto.


> [!WARNING]
> **📔 Nota**: El valor predeterminado de tres botones y el `showDefault()` asistente necesitan SDK `7.32.0` o posterior. En versiones anteriores, se muestran los ocho botones a menos que tu escena oculte algunos, y `showDefault()` no existe, así que `showAll()` es la única forma de volver a mostrar los botones.


SDK escribe ese conjunto predeterminado una vez al inicio de la escena, justo antes del primer frame, y lo omite cuando tu escena ya eligió sus propios botones. Tu escena cuenta como que ya eligió si ocurre cualquiera de estas dos cosas:

* Ya se ejecutó una de las funciones auxiliares de visibilidad de botones: `hideAll()`, `showAll()`, `showDefault()` o `hide()`.
* El componente en RootEntity ya enumera al menos un botón, por ejemplo porque tu escena lo escribió con `createOrReplace` en el nivel superior de `index.ts`, fuera de `main()`.

`hideJoystick()`, `showJoystick()`, `hideCrosshair()`, `showCrosshair()` y `setMainAction()` no cuentan: el conjunto de botones predeterminado sigue aplicándose junto con ellos.

Tu `main()` función se ejecuta después de esa escritura, así que cualquier cosa que hagas allí se aplica encima del conjunto predeterminado. Un asistente como `hide()` se fusiona con él, mientras que `createOrReplace` reemplaza toda la lista por la tuya.

Para volver a mostrar los botones ocultos, llama a `showAll()` para los ocho. `showDefault()` restaura el conjunto predeterminado y deja intactos tus iconos, joystick, crosshair y main action.

```ts
import { TouchScreenControls } from '@dcl/sdk/ecs'

// Muestra todos los botones en pantalla, incluidos los botones 1 / 2 / 3 / 4
TouchScreenControls.showAll()
```

## Cómo funciona el diseño de los botones

Los botones del gamepad forman una sola **pila de prioridad**. El orden es fijo:

1. `IA_JUMP`
2. `IA_POINTER`
3. `IA_PRIMARY` (E)
4. `IA_SECONDARY` (F)
5. `IA_ACTION_3` (1)
6. `IA_ACTION_4` (2)
7. `IA_ACTION_5` (3)
8. `IA_ACTION_6` (4)

Las posiciones en pantalla también son fijas. Los **visibles** botones llenan esas posiciones desde la parte superior de la pila hacia abajo — así que lo que cambias es *qué* botones son visibles y *cuál va primero*, no su orden.

| Cuando tú…                                               | Los controles…                                                                                                                                                |
| -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Ocultar un botón** (cualquier botón, incluido jump)    | Cada botón de menor prioridad sube para llenar el hueco. Oculta jump y `IA_POINTER` ocupa el lugar central.                                                   |
| **No cambiar nada**                                      | Se aplica el conjunto predeterminado: `IA_JUMP` es el gran botón central, con `IA_POINTER` y `IA_PRIMARY` en las ranuras a su alrededor.                      |
| **Establecer un botón principal** con `mainAction`       | Esa acción salta al frente y se convierte en el botón central; todos los demás botones conservan su orden normal.                                             |
| **Establecer un botón principal que además esté oculto** | Ocultar prevalece — el botón sigue oculto.                                                                                                                    |
| **Dejar visibles 5 botones o menos**                     | Todos se muestran directamente (el botón central más hasta cuatro alrededor); no hay menú "+".                                                                |
| **Dejar visibles más de 5 botones**                      | El "+" ocupa la última ranura, así que cuatro se muestran directamente (el botón central más tres) y el resto queda detrás del control de desbordamiento "+". |

<figure><img src="/docs/_remote/ac9947bc2b985676d54e81a8.webp" alt="Three HUDs showing the gamepad reflowing as the number of visible buttons changes"><figcaption><p>Cómo se reorganizan los botones según la cantidad visible. <strong>Izquierda (7 botones):</strong> el botón principal, un arco de 1–3 y el "+" que contiene el desbordamiento (4–7) en una columna que asciende. <strong>Centro (5 botones):</strong> el mismo arco, con una columna de desbordamiento más corta (4–5) detrás del "+". <strong>Derecha (4 botones):</strong> los cuatro se muestran directamente y el "+" desaparece.</p></figcaption></figure>


> [!NOTE]
> El `1`/`2`/`3`/`4` los botones empiezan ocultos. `showAll()` los vuelve a mostrar, pero eso deja ocho botones visibles, así que los de menor prioridad terminan detrás del "+". Para poner un botón numerado directamente en pantalla, mantén el recuento visible en cinco o menos: muestra solo los botones que tu escena realmente usa.


## Tareas comunes

`TouchScreenControls` incluye un conjunto de funciones auxiliares de conveniencia. Cada una escribe el componente en la `RootEntity` (donde el cliente lo lee) y se fusiona con el valor actual, así que puedes llamarlas desde cualquier lugar.

**Cambiar el botón principal** — haz que el gran botón central dispare una acción diferente:

```ts
import { TouchScreenControls, InputAction } from '@dcl/sdk/ecs'

export function main() {
	TouchScreenControls.setMainAction(InputAction.IA_PRIMARY)
}
```

**Ocultar el joystick o la mira** — elimina el stick de movimiento y/o la retícula de apuntado, y vuelve a mostrarlos con sus `show` contrapartes:

```ts
TouchScreenControls.hideJoystick()
TouchScreenControls.hideCrosshair()

// y para volver a mostrarlos:
TouchScreenControls.showJoystick()
TouchScreenControls.showCrosshair()
```

**Ocultar botones específicos** — pasa las acciones que quieres quitar (el resto sube en cascada):

```ts
TouchScreenControls.hide([InputAction.IA_SECONDARY, InputAction.IA_JUMP])
```

**Ocultar todos los botones, mostrar todos los botones o volver al conjunto predeterminado:**

```ts
TouchScreenControls.hideAll()     // no hay botones del gamepad en absoluto
TouchScreenControls.showAll()     // los ocho botones
TouchScreenControls.showDefault() // solo jump, pointer y primary
```

Estos tres solo afectan a los botones del gamepad. No restauran un joystick o una mira ocultos, y no cambian tus iconos personalizados ni tu main action. Usa `showJoystick()` / `showCrosshair()` para eso.

**Reemplazar el icono de un botón** — para tener control total (iconos personalizados, varios cambios a la vez), escribe el componente sin procesar en `engine.RootEntity`:

```ts
import { engine, TouchScreenControls, InputAction } from '@dcl/sdk/ecs'

export function main() {
	TouchScreenControls.createOrReplace(engine.RootEntity, {
		hideCrosshair: true,
		mainAction: InputAction.IA_PRIMARY,
		touchInputs: [
			{
				inputAction: InputAction.IA_PRIMARY,
				icon: { tex: { $case: 'texture', texture: { src: 'images/grab.png' } } },
			},
		],
	})
}
```

Las funciones auxiliares de un vistazo:

| Asistente               | Qué hace                                                                                                                                                                                      |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `setMainAction(action)` | Establece qué acción dispara el gran botón central.                                                                                                                                           |
| `hideJoystick()`        | Oculta el joystick virtual nativo.                                                                                                                                                            |
| `showJoystick()`        | Muestra de nuevo el joystick virtual nativo.                                                                                                                                                  |
| `hideCrosshair()`       | Oculta la mira / retícula en pantalla.                                                                                                                                                        |
| `showCrosshair()`       | Muestra de nuevo la mira / retícula en pantalla.                                                                                                                                              |
| `hide(actions)`         | Oculta los botones del gamepad indicados (fusionados con la configuración actual).                                                                                                            |
| `hideAll()`             | Oculta todos los botones del gamepad.                                                                                                                                                         |
| `showAll()`             | Muestra los ocho botones del gamepad (borra la lista de botones ocultos). No **no** afecta al joystick/crosshair.                                                                             |
| `showDefault()`         | Restaura el conjunto predeterminado: jump, pointer y primary visibles, secondary y las actions 3 a 6 ocultas. Mantiene los iconos, el joystick, la crosshair y el main action tal como están. |

## Propiedades

Escríbelas directamente cuando uses `createOrReplace`:

| Propiedad       | Tipo                                                                                                                      | Descripción                                                                                                                                                                                                                                                                                                                                   |
| --------------- | ------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `hideJoystick`  | *boolean*                                                                                                                 | Oculta el joystick virtual de movimiento nativo.                                                                                                                                                                                                                                                                                              |
| `hideCrosshair` | *boolean*                                                                                                                 | Oculta la mira / retícula en pantalla.                                                                                                                                                                                                                                                                                                        |
| `mainAction`    | [*InputAction*](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#pointer-buttons) | Mueve esta acción al frente de la pila, convirtiéndola en el gran botón central; los demás botones conservan su orden. Solo son válidas las acciones del gamepad (ver abajo). Cuando no se establece, el primer botón visible (`IA_JUMP` por defecto) va primero. Ver [Cómo funciona el diseño de los botones](#how-the-button-layout-works). |
| `touchInputs`   | *array*                                                                                                                   | Anulaciones por botón. Escribir este campo reemplaza toda la lista, así que cualquier botón que omitas se muestra con su glifo predeterminado, incluidos los que empiezan ocultos.                                                                                                                                                            |

Cada `touchInputs` la entrada tiene:

| Campo         | Tipo                                                                                                                      | Descripción                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ------------- | ------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `inputAction` | [*InputAction*](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#pointer-buttons) | Qué botón en pantalla configura esta entrada.                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `hide`        | *boolean*                                                                                                                 | Oculta este botón. El valor predeterminado es `false` (mostrado). Cualquier botón puede ocultarse, **incluyendo `IA_JUMP`** — el resto sube en cascada para ocupar su lugar.                                                                                                                                                                                                                                                                                               |
| `icon`        | [*TextureUnion*](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/ui_background/#background) (opcional)                    | Reemplaza el glifo del botón con una imagen de la escena. Usa la `texture` variante con un `src` (una imagen incluida en tu escena) — `{ tex: { $case: 'texture', texture: { src: 'images/grab.png' } } }`. Solo se admiten rutas de contenido de la escena (no URLs externas, ni texturas de avatar o de vídeo). Para el botón jump, esto reemplaza todos sus estados dinámicos (jump / double-jump / glide). Si no se puede resolver la ruta, se usa el glifo integrado. |

## Qué acciones se asignan a qué botones

El [`InputAction`](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#pointer-buttons) los valores aquí son los mismos que se usan en toda [Entrada en móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/input-on-mobile/) y [Eventos de clic](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/). Estas son las acciones que se asignan a botones en pantalla:

| InputAction                                                   | Botón en pantalla                      |
| ------------------------------------------------------------- | -------------------------------------- |
| `IA_JUMP`                                                     | El gran botón central (predeterminado) |
| `IA_POINTER`                                                  | El botón de interacción                |
| `IA_PRIMARY`                                                  | El botón E                             |
| `IA_SECONDARY`                                                | El botón F                             |
| `IA_ACTION_3` / `IA_ACTION_4` / `IA_ACTION_5` / `IA_ACTION_6` | Los botones 1 / 2 / 3 / 4              |


> [!WARNING]
> `IA_ANY` y `IA_MODIFIER` son valores meta — no se asignan a un botón y no pueden usarse aquí.


## Ejemplo

Para ocultar el joystick, mantener ocultos los botones F y numerados, y darle al botón central jump un icono personalizado incluido en tu escena:

```ts
import { engine, TouchScreenControls, InputAction } from '@dcl/sdk/ecs'

export function main() {
	TouchScreenControls.createOrReplace(engine.RootEntity, {
		hideJoystick: true,
		touchInputs: [
			// Esta lista reemplaza a la predeterminada, así que los botones que normalmente empiezan
			// ocultos también deben incluirse aquí
			{ inputAction: InputAction.IA_SECONDARY, hide: true },
			{ inputAction: InputAction.IA_ACTION_3, hide: true },
			{ inputAction: InputAction.IA_ACTION_4, hide: true },
			{ inputAction: InputAction.IA_ACTION_5, hide: true },
			{ inputAction: InputAction.IA_ACTION_6, hide: true },
			{
				inputAction: InputAction.IA_JUMP,
				icon: { tex: { $case: 'texture', texture: { src: 'images/banana.png' } } },
			},
		],
	})
}
```

<figure><img src="/docs/_remote/c911e3ac3460b2381dd7c1b5.webp" alt="A mobile HUD with the joystick hidden and the central jump button showing a custom banana icon"><figcaption><p>El resultado: joystick eliminado, botones numerados ocultos (así que el "+" desaparece) y el botón jump con un nuevo icono</p></figcaption></figure>

Para reemplazar por completo los controles nativos, ocúltalos aquí y construye tus propios botones táctiles con [Vinculación de entrada de UI](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/ui_input_binding/).

## Relacionado

* [Vinculación de entrada de UI](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/ui_input_binding/)
* [Entrada en móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/input-on-mobile/)
* [Eventos de clic](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/)
* [Detectar la plataforma desde el código](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/detect-platform/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/touch-screen-controls.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
