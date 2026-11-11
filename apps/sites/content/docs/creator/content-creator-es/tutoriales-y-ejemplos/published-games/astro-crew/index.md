> Combate cooperativo de naves espaciales con estado del juego propiedad del servidor, progresión persistente, cámaras de torreta y marcadores de objetivo en el borde de la pantalla.

# Astro Crew

![Vista previa de Astro Crew](/docs/_remote/31ce14ffd2ff2d42c05fb6b7.webp)

Una escena cooperativa donde la tripulación comparte una sola nave. En cada parada disparan a los asteroides y platillos volantes entrantes, reparan las brechas del casco y suben los niveles de artillero e ingeniería. La pelea se ejecuta en un servidor multijugador autoritativo.

* [Juega en Decentraland](https://decentraland.org/play/?realm=astrocrew.dcl.eth)

## Información del proyecto

**Mantenido por:** Comunidad · DansStuff

[Ver la fuente upstream](https://github.com/DansStuff/AstroCrew)

**Licencia:** No identificado en este catálogo. Consulta el repositorio upstream y al mantenedor para conocer los términos aplicables antes de reutilizarlo.


> [!WARNING]
> **Código mantenido por la comunidad — revísalo antes de usarlo**
> 
> Este proyecto comunitario se mantiene fuera de las organizaciones de GitHub de Decentraland Foundation y DAO. Su código fuente y su despliegue en vivo pueden cambiar de forma independiente.
> 
> La inclusión en este catálogo no implica una auditoría de seguridad ni garantiza la seguridad. Revisa el código, las dependencias y los scripts de instalación antes de ejecutarlo o reutilizarlo.
> 
> [Reportar un problema del catálogo](https://github.com/decentraland/docs/issues/new). Para posibles vulnerabilidades, sigue la política de seguridad del proyecto upstream. No incluyas detalles de explotación ni credenciales en incidencias públicas.


## Características clave

* **Multijugador autoritativo** durante toda la pelea. Los clientes envían solicitudes para apuntar, reparar y sobrecargar. El servidor controla el casco, las brechas y los spawns, y sincronizados `GameState` y `PlayerStats` solo aceptan escrituras del servidor multijugador.
* **Almacenamiento** para los niveles de artillero e ingeniería de cada jugador, y una tabla de misiones semanal.
* **Cámaras virtuales** en las tres torretas, mezcladas con una transición temporizada. Los impactos sacuden esas cámaras. Tomar un arma congela al avatar con **InputModifier** y oculta el joystick y la mira del móvil con **TouchScreenControls**.
* **PrimaryPointerInfo** apunta cada disparo desde el rayo del mundo del Pointer. **PointerLock** permanece liberado para que **PointerEvents** todavía puedan controlar las consolas, mantener pulsadas las brechas para repararlas y la mesa de la misión.
* **GltfNodeModifiers** recolorean las Meshes emisivas de la nave: verde mientras navega, rojo durante un encuentro y un segundo conjunto mientras las armas están sobrecargadas.
* **ReactEcs** HUD para el casco, los niveles de habilidad y la tabla del final de la ronda. Los iconos de brecha fuera de pantalla se sitúan en el borde de la pantalla, colocados con **UiCanvasInformation**.
* **Billboards**, **TextShape** y **avatar textures** en cada objetivo: una mira orientada hacia la cámara, una etiqueta "Objetivo bloqueado" y retratos de quien le esté disparando.
* **Animator** repite el clip de giro del platillo volante. **AudioSource** reproduce stingers y loops, con un pool de disparos únicos de láser para que un disparo pueda reiniciarse mientras el último clip aún se está reproduciendo. Los clips se precargan con **AssetLoad**.
* In-world **TextShape** filas para la tabla semanal. Al hacer clic en una fila se abren los resultados de esa misión.
* **AvatarModifierArea** desactiva el aviso del passport en toda la escena.

## Explora el código

Estos enlaces apuntan a una instantánea específica de la fuente para que las explicaciones sigan siendo útiles a medida que continúa el desarrollo upstream. Son referencias de aprendizaje, no una evaluación de seguridad.

### Orientar a los jugadores hacia objetivos fuera de pantalla

edgePositionForBreach proyecta una dirección de brecha sobre los ejes horizontales de la cámara, encuentra un borde de un rectángulo y limita el icono dentro de la pantalla usando la relación de aspecto del canvas. BreachIcon conecta esa posición con un marcador de ReactEcs pulsante.

**Contexto del proyecto:** Léelo con las posiciones de brecha de la escena, el estado de brecha activa y las constantes de UI.

**Referencias de la fuente:**

* [edgePositionForBreach](https://github.com/DansStuff/AstroCrew/blob/dbe11b05be2a753b27b6d5dd866d0554ff73344f/src/ui/breachIcons.tsx#L27-L76)
* [BreachIcon](https://github.com/DansStuff/AstroCrew/blob/dbe11b05be2a753b27b6d5dd866d0554ff73344f/src/ui/breachIcons.tsx#L78-L107)

### Añadir sacudida de cámara que vuelve a la pose original

shakeEntity registra un Transform base y combina las sacudidas superpuestas usando la intensidad y la duración restante. CameraShakeSystem aplica desplazamientos decrecientes de posición y rotación, luego restaura la pose base y elimina el Component.

**Contexto del proyecto:** El efecto usa las constantes de amplitud del proyecto y debe registrarse como un sistema ECS.

**Referencias de la fuente:**

* [shakeEntity](https://github.com/DansStuff/AstroCrew/blob/dbe11b05be2a753b27b6d5dd866d0554ff73344f/src/effects/cameraShake.ts#L13-L47)
* [CameraShakeSystem](https://github.com/DansStuff/AstroCrew/blob/dbe11b05be2a753b27b6d5dd866d0554ff73344f/src/effects/cameraShake.ts#L87-L121)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/published-games/astro-crew.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
