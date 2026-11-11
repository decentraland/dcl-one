> Lugares

# Lugares

<details>

<summary>¿Qué son los Places?</summary>

[Places](https://decentraland.org/places/) muestra ciertos puntos de interés en Genesis City y Worlds.

</details>

<details>

<summary>¿Cómo edito el título, la descripción o la imagen de mi Place?</summary>

El título, la descripción y la imagen de tu Place se toman de los metadatos de la escena. Obtén más información sobre cómo editar los metadatos de la escena en la documentación de Creator.

</details>

<details>

<summary>¿Cómo se actualiza mi Place?</summary>

Cuando vuelvas a subir tu escena, el sitio web de Places actualizará automáticamente cualquier metadato.

</details>

<details>

<summary>¿Qué pasa si cambio mi Place de forma significativa?</summary>

A cada Place se le asigna un ID único. Cuando se cumplen ciertas condiciones, tu Place puede considerarse "nuevo", lo que da lugar a la asignación de un nuevo ID. Este cambio hará que tu Place pierda todos los favoritos y likes acumulados. Para obtener más detalles sobre estas condiciones, consulta [**ADR: Place Identifiers**](https://adr.decentraland.org/adr/ADR-186).

</details>

<details>

<summary>¿Cómo se calcula el porcentaje de likes?</summary>

Para evitar abusos, el porcentaje de likes se calcula a partir de likes y dislikes de usuarios que tengan al menos 100 de poder de voto. Cualquiera con una cuenta puede dar like o dislike, pero solo los votos que cumplen los requisitos se utilizan para calcular el porcentaje de likes.

Aumenta tus posibilidades de obtener una buena valoración asegurándote de que tu Place tenga un título, una descripción y una miniatura.

</details>

<details>

<summary>¿Cómo puede mi Place convertirse en un Point of Interest?</summary>

Los puntos de interés son ubicaciones destacadas en Decentraland. Estos "POIs" se promocionan en varias áreas de la UI del mundo virtual y se presentan como buenos lugares para que los usuarios exploren, especialmente quienes son nuevos en Decentraland.

Puedes crear una propuesta en la [**DAO**](https://decentraland.org/dao/en/) para nominar tu Place como Point of Interest.

</details>

<details>

<summary>¿Qué Worlds aparecen en la pestaña Worlds?</summary>

Según lo definido en *Worlds 1.0 - Plan a corto plazo* [**propuesta de DAO**](https://decentraland.org/governance/proposal/?id=e712bb50-e822-11ed-b8f1-75dbe089d333/), permitimos el descubrimiento de Worlds desplegados en el World Content Server de Foundation solo para los propietarios de NAME que también posean LAND o un contrato de alquiler de LAND activo.

</details>


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-es/preguntas-frecuentes/places.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
