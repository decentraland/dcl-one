> Emparejamiento de memoria seguido de persecuciones de monstruos activadas por disparadores, con progresión coleccionable, tablas de clasificación y texto bitmap personalizado.

# Monster Recon

![Vista previa de Monster Recon](/docs/_remote/276e8747b4e8b433f9cfa9f6.webp)

Un juego de memoria en el que limpias tableros cronometrados y luego persigues al monstruo que revelaste antes de que escape. Recoge hallazgos raros en tu Codex y escala en las tablas de clasificación semanales y de todos los tiempos.

* [Juega en Decentraland](https://decentraland.org/jump?realm=monsterrecon.dcl.eth)

## Información del proyecto

**Mantenido por:** Comunidad · carlosmu

[Ver la fuente upstream](https://github.com/carlosmu/dcl-monster-recon)

**Licencia:** No identificado en este catálogo. Consulta el repositorio upstream y al mantenedor para conocer los términos aplicables antes de reutilizarlo.


> [!WARNING]
> **Código mantenido por la comunidad — revísalo antes de usarlo**
> 
> Este proyecto de la comunidad se mantiene fuera de las organizaciones de GitHub de Decentraland Foundation y DAO. Su código fuente y su despliegue en vivo pueden cambiar de forma independiente.
> 
> La inclusión en este catálogo no implica una auditoría de seguridad ni garantiza que sea seguro. Revisa el código, las dependencias y los scripts de instalación antes de ejecutarlo o reutilizarlo.
> 
> [Reportar un problema del catálogo](https://github.com/decentraland/docs/issues/new). Para posibles vulnerabilidades, sigue la política de seguridad del proyecto upstream. No incluyas detalles del exploit ni credenciales en issues públicas.


## Características clave

* Servidor multijugador
* Almacenamiento para progreso, mejores tiempos, tablas de clasificación y recompensas semanales.
* Cámaras virtuales
* Emotes de escena
* Áreas de Trigger
* Partículas
* Tweens
* TextShapes
* Leaderboard 3D (con fotos de perfil de los jugadores)
* Fuente bitmap personalizada para la UI
* Feed en vivo sincronizado de las capturas de otros jugadores

## Explora el código

Estos enlaces apuntan a una instantánea específica de la fuente para que las explicaciones sigan siendo útiles mientras continúa el desarrollo upstream. Son referencias de aprendizaje, no una evaluación de seguridad.

### Construye una persecución a partir de marcadores de escena y áreas de Trigger

`spawnHop` combina un tween de balanceo, un trigger esférico, sonido y un sprite Billboard en un marcador seleccionado. El manejador de captura verifica que la entidad que entra sea el jugador local antes de limpiar y llamar al callback de recompensa.

**Contexto del proyecto:** Lee esto junto con los nombres de los marcadores, el atlas de sprites y los callbacks proporcionados a `startPrizeChase`.

**Referencias de la fuente:**

* [`spawnHop`](https://github.com/carlosmu/dcl-monster-recon/blob/eb0bc6fa87b9ebf98c8c068ebb9bba686a9467cb/src/prizeChase.ts#L216-L271)
* [manejador de captura](https://github.com/carlosmu/dcl-monster-recon/blob/eb0bc6fa87b9ebf98c8c068ebb9bba686a9467cb/src/prizeChase.ts#L183-L214)

### Renderiza texto bitmap personalizado en ReactEcs

`BitmapTextLine` dispone las imágenes de glifos individuales usando UVs del atlas, desplazamientos de glifos y kerning. Los helpers anteriores de medición y ajuste dividen el texto en líneas según el ancho medido.

**Contexto del proyecto:** El renderer necesita métricas de fuente bitmap y assets de textura coincidentes.

**Referencias de la fuente:**

* [`BitmapTextLine`](https://github.com/carlosmu/dcl-monster-recon/blob/eb0bc6fa87b9ebf98c8c068ebb9bba686a9467cb/src/bitmapFont.tsx#L268-L327)
* [helpers de medición y ajuste](https://github.com/carlosmu/dcl-monster-recon/blob/eb0bc6fa87b9ebf98c8c068ebb9bba686a9467cb/src/bitmapFont.tsx#L229-L266)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/published-games/monster-recon.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
