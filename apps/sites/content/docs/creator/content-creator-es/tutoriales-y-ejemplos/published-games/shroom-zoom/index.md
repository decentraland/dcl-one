> Plataformeo con raycasts de rebote activados por disparadores, impulsos del jugador y sprites coleccionables que siguen o orbitan a su dueño.

# Shroom Zoom

![Vista previa de Shroom Zoom](/docs/_remote/abaf092d80ad05ebe9c8fc1b.webp)

Un juego de plataformas y contrarreloj con doble salto, planeo, coleccionables y exploración.

* [Jugar en Decentraland](https://decentraland.org/jump?realm=shroomzoom.dcl.eth)

## Información del proyecto

**Mantenido por:** Comunidad · CanessaDCL

[Ver la fuente upstream](https://github.com/CanessaDCL/dcl-shroomzoom)

**Licencia:** No identificado en este catálogo. Consulta el repositorio upstream y al mantenedor para conocer los términos aplicables antes de reutilizarlo.


> [!WARNING]
> **Código mantenido por la comunidad — revisa antes de usarlo**
> 
> Este proyecto comunitario se mantiene fuera de las organizaciones de GitHub de Decentraland Foundation y DAO. Su código fuente y su implementación en vivo pueden cambiar de forma independiente.
> 
> La inclusión en este catálogo no implica una auditoría de seguridad ni garantiza la seguridad. Revisa el código, las dependencias y los scripts de instalación antes de ejecutarlo o reutilizarlo.
> 
> [Informar de un problema del catálogo](https://github.com/decentraland/docs/issues/new). Para posibles vulnerabilidades, sigue la política de seguridad del proyecto upstream. No incluyas detalles del exploit ni credenciales en issues públicos.


## Características clave

* Servidor multijugador
* Impulsos de física para hongos reboteadores

## Explora el código

Estos enlaces apuntan a una instantánea específica del código fuente, de modo que las explicaciones sigan siendo útiles a medida que continúa el desarrollo upstream. Son referencias de aprendizaje, no una evaluación de seguridad.

### Combina un trigger cercano con una comprobación precisa de contacto

Los callbacks de BounceTrigger permiten realizar raycasting mientras el jugador está dentro de un trigger y eliminan ese sistema al salir. La lógica de raycast y rebote comprueba los impactos hacia abajo contra la entity de hongo configurada y aplica un impulso al jugador cerca de su superficie.

**Contexto del proyecto:** Léelo junto con BounceZoom o BounceRegular para ver las dimensiones del trigger de la escena y los ajustes de impulso.

**Referencias de la fuente:**

* [callbacks de BounceTrigger](https://github.com/CanessaDCL/dcl-shroomzoom/blob/042968e3c13b5d1093d2922d5713570ad4cb0284/src/bounce/class.bounce.ts#L75-L101)
* [La lógica de raycast y rebote](https://github.com/CanessaDCL/dcl-shroomzoom/blob/042968e3c13b5d1093d2922d5713570ad4cb0284/src/bounce/class.bounce.ts#L103-L164)

### Convierte los objetos recogidos en un séquito animado

Las funciones auxiliares de destino de follower calculan desplazamientos alrededor del jugador y posiciones en una órbita. followSpritesSystem interpola cada sprite hacia su objetivo, cambiando a órbita cuando la colección está completa y girando los sprites hacia su propietario.

**Contexto del proyecto:** Los conjuntos de colección de la escena, las entities follower y las marcas de tiempo de órbita por jugador proporcionan el estado de este comportamiento visual.

**Referencias de la fuente:**

* [Funciones auxiliares de destino de follower](https://github.com/CanessaDCL/dcl-shroomzoom/blob/042968e3c13b5d1093d2922d5713570ad4cb0284/src/spriteFollowers.ts#L400-L437)
* [followSpritesSystem](https://github.com/CanessaDCL/dcl-shroomzoom/blob/042968e3c13b5d1093d2922d5713570ad4cb0284/src/spriteFollowers.ts#L678-L706)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/published-games/shroom-zoom.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
