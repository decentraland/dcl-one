> Detecta si tu escena se está ejecutando en móvil, escritorio o web.

# Detectar la plataforma

Usa la `isMobile()` función para adaptar tu UI, controles y gameplay específicamente para mobile. Esta es la forma recomendada de ofrecer una gran experiencia en todos los clientes sin bifurcar por completo la lógica de tu scene.

## Funciones disponibles

```ts
import { getPlatform, isMobile, isDesktop, isWeb } from '@dcl/sdk/platform'
```

* **`getPlatform()`** — devuelve la plataforma actual como `'mobile' | 'desktop' | 'web' | null`. Devuelve `null` hasta que el Explorer haya informado su plataforma de vuelta a la scene (esto ocurre poco después de que la scene comience).
* **`isMobile()`** — devuelve `true` si el jugador está en el cliente mobile.
* **`isDesktop()`** — devuelve `true` si el jugador está en el cliente desktop.
* **`isWeb()`** — devuelve `true` si el jugador está en el cliente web.


> [!WARNING]
> **📔 Nota**: Estas funciones leen un valor que se completa de forma asíncrona cuando la scene comienza, y nada garantiza que ya esté disponible cuando `main()` se ejecute. Si las llamas demasiado pronto, todavía podrían devolver `null` / `false`. Para estar seguro, pospone cualquier configuración dependiente de la plataforma hasta que `getPlatform()` devuelva algo distinto de `null`, por ejemplo comprobándolo dentro de un system:
> 
> ```ts
> import { engine } from '@dcl/sdk/ecs'
> import { getPlatform } from '@dcl/sdk/platform'
> 
> function platformCheckSystem() {
>   if (getPlatform() === null) return
>   engine.removeSystem(platformCheckSystem)
>   setupUI()
> }
> 
> engine.addSystem(platformCheckSystem)
> ```
> 


## Divide tu UI por plataforma

Un patrón común es configurar una UI diferente para jugadores mobile y desktop:

```ts
import { isMobile, isDesktop } from '@dcl/sdk/platform'

function setupUI() {
  if (isMobile()) {
    // Botones más grandes, diseño más simple para touch
    createMobileUI()
  } else if (isDesktop()) {
    // Diseño más compacto ajustado para teclado y mouse
    createDesktopUI()
  }
}
```

También puedes usar el mismo patrón para:

* Mostrar u ocultar instrucciones en pantalla adaptadas a cada método de input.
* Reemplazar elementos pequeños clicables con objetivos táctiles más grandes en mobile.
* Deshabilitar asignaciones de input que no están fácilmente disponibles en mobile (ver [Input on mobile](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/input-on-mobile/)).

## Comprobación del valor bruto de la plataforma

Si necesitas manejar múltiples plataformas en un solo switch, usa `getPlatform()`:

```ts
import { getPlatform } from '@dcl/sdk/platform'

const platform = getPlatform()

switch (platform) {
  case 'mobile':
    // comportamiento específico de mobile
    break
  case 'desktop':
    // comportamiento específico de desktop
    break
  case 'web':
    // comportamiento específico de web
    break
  case null:
    // la plataforma aún no se conoce
    break
}
```

## Relacionado

* [Área segura para móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/safe-area/)
* [Buenas prácticas de UI para móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/ui-best-practices/)
* [UI en pantalla](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/desarrollar-para-movil/desarrollar/detect-platform.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
