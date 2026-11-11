> Lo que se debe y no se debe hacer al diseñar UI de escena que funcionen bien en móviles.

# Mejores prácticas de UI

Todavía no existe una única receta probada para la UI móvil de Decentraland: la plataforma es nueva y seguimos iterando. Las recomendaciones de esta página son las mejores prácticas actuales, destiladas del trabajo en escenas reales. Tómalas como un punto de partida y pruébalas en un dispositivo real.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/5OmTTzpPdDc" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=5OmTTzpPdDc>">Watch on YouTube</a></p>



## Qué hacer

* **Diseña UI específicas para móviles**, o varía tu UI según el tamaño de pantalla y la plataforma. Usa [`isMobile()`](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/detect-platform/) para ramificar.
* **Mantén la UI crítica dentro de la** [**área segura**](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/safe-area/)**.** Los márgenes de hardware del dispositivo (notch, barra de estado, indicador de inicio) se despejan automáticamente por defecto — solo no desactives esto con `screenInset: 'none'`. Los controles propios del cliente son un área separada, y `screenInset: 'interactable'` cumple parte de esa función: posiciona tu UI en el rectángulo que el Explorer designa para la UI de la escena. En mobile eso despeja los controles del lado izquierdo; los botones de acción en la esquina inferior derecha se dibujan sobre el área por diseño, así que ten [esas esquinas](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/safe-area/#where-the-client-controls-live) en mente. `'interactable'` requiere cliente móvil `1.12.1` o más reciente — los clientes más antiguos no reportan ningún margen y la UI cubre toda la pantalla.
* **Minimiza las opciones.** Muestra solo lo que el jugador necesita ahora mismo y revela progresivamente el resto.
* **Coloca los diálogos accionables en el centro de la pantalla** — en cualquier lugar donde el jugador necesite leer y responder.
* **Coloca los mensajes no accionables en el centro superior** — estado, notificaciones e información ambiental.
* **Oculta los controles táctiles detrás de una UI a pantalla completa.** Mientras una tabla de puntuaciones, una tienda o un panel de resultados esté abierto, el jugador no necesita saltar ni presionar E. Consulta [UI a pantalla completa sobre los botones de acción](#full-screen-ui-over-the-action-buttons).

## Qué no hacer

* **No dimensiones los elementos de UI solo en píxeles sin tener en mente una pantalla virtual.** Los valores en píxeles se escalan con respecto a la `virtualWidth` / `virtualHeight` resolución de referencia descrita en [UI en pantalla](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/#screen-virtual-scale) — `1600x720` por defecto en móvil, `1920x1080` en desktop y web. Sabe contra qué resolución de referencia se crean tus valores en píxeles, pásala explícitamente si no es la predeterminada y combínala con un tamaño adaptado a la plataforma. Desactiva la pantalla virtual solo (pasando un tamaño de `0`) si realmente quieres píxeles sin procesar del canvas.
* **No coloques elementos fuera del área segura.** Entrarán en conflicto con los controles del cliente.
* **No ancles la UI a la esquina superior izquierda ni al borde izquierdo.** Ahí es donde el cliente dibuja el chat, el perfil y el joystick virtual, todo encima de la UI de tu escena. Ancla a la derecha o al centro, o usa `screenInset: 'interactable'`. Consulta [Nunca ancles tu UI en la esquina superior izquierda](/docs/creator/content-creator-es/scenes-sdk7/disenar-la-experiencia/ux-ui-guide/#never-anchor-your-ui-to-the-top-left).
* **No pongas elementos interactivos en la esquina inferior derecha.** Los botones de acción se dibujan sobre esa región, y los toques allí van a los botones del cliente, no a tu escena.
* **No dependas de botones pequeños.** Los objetivos pequeños son poco fiables para tocar en una pantalla táctil.
* **No vincules acciones de teclas a `IA_SECONDARY` o `IA_ACTION_3`–`IA_ACTION_6`** (la `F` y `1`/`2`/`3`/`4` teclas en un teclado) a menos que tu escena muestre esos botones. Están ocultos por defecto en móvil (SDK 7.32.0+). Consulta [Entrada en móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/input-on-mobile/).

## UI a pantalla completa sobre los botones de acción

`screenInset: 'interactable'` mantiene tu UI libre de los controles del lado izquierdo, pero los botones de acción en la esquina inferior derecha se dibujan sobre esa área por diseño. Un panel que llena el área sigue cubriéndose allí, y los toques en esa región activan los botones del cliente en lugar de tu UI.

Para un **HUD de gameplay** que debe seguir siendo usable mientras el jugador se mueve, mantenlo fuera de la esquina inferior derecha: ancla lo arriba a la derecha o al centro superior.

Para un **UI que interrumpe el gameplay**, como una tabla de puntuaciones, una tienda o una pantalla de resultados, oculta los controles táctiles mientras esté abierta y restáuralos al cerrar:

```ts
import { TouchScreenControls } from '@dcl/sdk/ecs'

function openScoreboard() {
  TouchScreenControls.hideAll()
  TouchScreenControls.hideJoystick()
  // ... muestra tu UI
}

function closeScoreboard() {
  TouchScreenControls.showDefault()
  TouchScreenControls.showJoystick()
  // ... oculta tu UI
}
```

Usa `showDefault()` en lugar de `showAll()` para restaurar los botones: las escenas comienzan con solo jump, pointer y primary visibles, así que `showAll()` traería de vuelta cinco botones que el jugador nunca tuvo. Tanto el valor predeterminado de tres botones como `showDefault()` requieren SDK 7.32.0 o posterior; en SDKs más antiguos se muestran los ocho botones y `showAll()` es la única forma de restaurarlos. `showDefault()` solo restaura los botones del gamepad, así que combínalo con `showJoystick()`. Ambos helpers no hacen nada en desktop, así que no hace falta ramificar por plataforma. Consulta [Controles en pantalla](/docs/creator/content-creator-es/scenes-sdk7/interactividad/touch-screen-controls/) para ver el componente completo.

Si el avatar también debe dejar de moverse mientras el panel esté abierto, añade un [InputModifier](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/locomotion/#freeze-the-player) encima.

## Dimensionamiento

La vieja regla general — *diseñar en desktop y luego escalar los tamaños de la UI 3× para móvil* — es anterior a dos cambios del SDK y ya no debería aplicarse a ciegas. La mayor parte de ese 3× ahora se aplica por ti:

* **`devicePixelRatio` no interviene en la distribución de la UI.** La UI con tamaño en píxeles ahora se renderiza `devicePixelRatio` veces más grande que antes — normalmente **2–3× en un teléfono**, y también en laptops retina. No hay forma de desactivarlo.
* **La pantalla virtual tiene valores predeterminados por plataforma**, `1600x720` en móvil contra `1920x1080` en desktop y web, así que el mismo valor en píxeles ya cubre aproximadamente 1.2× más de la pantalla en un teléfono.

Parte de tus tamaños de desktop sin cambios, mide en un dispositivo real — consulta [Vista previa en móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/preview-on-mobile/) — y escala solo donde los objetivos táctiles o el texto realmente se queden cortos. Ramifica con [`isMobile()`](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/detect-platform/) cuando los dos realmente necesiten diferir.

## Relacionado

* [Funciones que faltan](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/missing-features/)
* [área segura móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/safe-area/)
* [Detectar la plataforma desde el código](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/detect-platform/)
* [UI en pantalla](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/)
* [Guía de UX & UI](/docs/creator/content-creator-es/scenes-sdk7/disenar-la-experiencia/ux-ui-guide/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/desarrollar-para-movil/desarrollar/ui-best-practices.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
