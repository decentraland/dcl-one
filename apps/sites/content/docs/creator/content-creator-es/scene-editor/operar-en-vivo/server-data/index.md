> Consulta y edita los datos en vivo almacenados por el Multiplayer Server de tu escena.

# Datos del servidor

Las escenas que usan un [Servidor Multijugador](/docs/creator/content-creator-es/scenes-sdk7/networking/authoritative-servers/) pueden conservar datos en el servidor: tablas de clasificación, progreso del jugador, cambios en el entorno como puertas abiertas o elementos colocados, y más. Estos datos se actualizan **en vivo** a medida que los jugadores interactúan con tu escena publicada.

Puedes ver todos estos datos desde una UI web, y también puedes **cambiarlos directamente desde allí**. Cualquier edición que hagas surte efecto en la escena en vivo, sin necesidad de volver a publicarla. Esto incluye:

* **Datos de la escena**: valores compartidos por todos los jugadores, como una tabla de clasificación o el estado de una puerta.
* **Datos del jugador**: valores almacenados para un solo jugador, como su progreso o sus preferencias.
* **Variables de entorno**: valores de configuración y secretos que solo el servidor puede leer.

## Accede a la UI de almacenamiento

Puedes abrir la UI de almacenamiento directamente en [decentraland.org/storage](https://decentraland.org/storage).

También puedes acceder a ella desde Creator Hub:

1. Abre la **Manage** sección de Creator Hub.
2. Haz clic en el **tres puntos** junto a un lugar donde has publicado contenido.
3. Selecciona **Ver almacenamiento**.

<img src="/docs/_remote/512f80176ec141582be43e70.webp" alt="Opción Ver almacenamiento en Creator Hub" width="400">

La UI de almacenamiento muestra una lista de todas las ubicaciones de Worlds y LAND donde has publicado escenas, o tienes permisos de operador. Abre una escena para ver sus datos, organizados en tres pestañas: **Scene**, **Player**, y **Entorno**.


> [!NOTE]
> **💡 Consejo**: Estos datos solo existen para escenas que usan un Multiplayer Server y que almacenan datos a través de la `Almacenamiento` API. Consulta [Servidor Multijugador](/docs/creator/content-creator-es/scenes-sdk7/networking/authoritative-servers/) para aprender cómo configurarlo.


## Datos de la escena

El **Scene** la pestaña enumera todas las variables almacenadas que se comparten entre todos los jugadores, por ejemplo una tabla de clasificación o el estado actual del entorno.

![Scene data tab](/docs/_remote/76bca9abaef9d73bc207163d.webp)

Los valores de esta lista se actualizan en vivo a medida que los jugadores interactúan con tu escena. También puedes editar o eliminar cualquiera de estas variables haciendo clic en el icono del lápiz o de la papelera. Los cambios son tomados por la escena en ejecución, así que esta es una forma práctica de ajustar valores en vivo, como reiniciar una tabla de clasificación, sin volver a publicar.

## Datos del jugador

El **Player** la pestaña enumera todos los jugadores que tienen algún dato almacenado en tu servidor. Puedes buscar a un jugador por dirección de wallet o por nombre, y luego ver todos sus datos asociados.

Al igual que con los datos de la escena, puedes editar o eliminar cualquier valor haciendo clic en el icono del lápiz o de la papelera.

Esto es especialmente útil para soporte: si un jugador en particular informa de un problema en tu escena, puedes buscarlo e inspeccionar sus datos almacenados para entender su situación. Si terminó atascado en un estado incorrecto, por ejemplo con datos contradictorios de una versión anterior de tu escena, puedes editar o borrar sus registros para restaurarlo a un estado estable, sin redeplegar nada.

## Variables de entorno

El **Entorno** la pestaña enumera todas las variables de entorno definidas para tu escena. Son valores de configuración que solo el servidor puede leer, lo que las convierte en el lugar adecuado para **secretos** como claves privadas o códigos de reclamo de recompensas: los valores nunca viajan a través de la máquina de un jugador ni aparecen en el código publicado de tu escena.

![Environment variables tab](/docs/_remote/13189fe9bb9d6d0cc3454293.webp)

Desde esta pestaña puedes añadir nuevas variables, o sobrescribir y eliminar las existentes usando el icono del lápiz o de la papelera. Ten en cuenta que no puedes *leer* el valor actual de una variable; esto es intencional, para proteger datos sensibles. Solo puedes reemplazarla o eliminarla.

Las variables de entorno también son ideales para flags de funciones o parámetros del juego, como la duración de una partida o un máximo de jugadores, que quizá quieras ajustar en la escena en vivo sin volver a publicar.

## Más información

Para aprender a leer y escribir estos datos desde el código de tu escena, incluidas las prácticas recomendadas para cambiar la estructura de los datos almacenados con el tiempo, consulta [Servidor Multijugador](/docs/creator/content-creator-es/scenes-sdk7/networking/authoritative-servers/).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/operar-en-vivo/server-data.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
