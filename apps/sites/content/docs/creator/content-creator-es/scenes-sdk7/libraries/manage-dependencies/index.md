> Cómo añadir librerías a una escena

# Gestionar dependencias

Muchas veces, tu escena puede necesitar una funcionalidad que ya está encapsulada en una librería reutilizable. Usar librerías externas puede facilitar mucho tu trabajo.

Puedes importar librerías externas en un proyecto de Decentraland y luego hacer referencia a los systems, components o functions descritas en estas librerías.

Consulta la [página de ejemplos](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library) para encontrar una serie de librerías, creadas tanto por la Decentraland Foundation como por miembros de la comunidad, que resuelven problemas comunes.

## Mediante la CLI

### Instalar

Para instalar una librería en la carpeta de tu proyecto, ejecuta `npm i` y el nombre de la librería en la ruta raíz de tu proyecto. Por ejemplo:

`npm i @dcl-sdk/utils`

> **📔 Nota**: Es posible que necesites ejecutar `npm run start` o `npm run build` después de instalar la librería para que se instalen las dependencias de la librería.

### Actualizar

Si tu `archivo package.json` archivo enumera la versión de la librería como `@latest`, entonces ejecuta `npm i` para actualizar todas las librerías a sus versiones más recientes correspondientes.

Si tu `archivo package.json` hace referencia a un número de versión específico, puedes actualizarla ejecutando el comando para instalar la librería, añadiendo `@` y el número de versión al final. Por ejemplo:

`npm i @dcl-sdk/utils@1.7.5`

### Desinstalar

Para eliminar una librería de las dependencias de tu escena, ejecuta `npm rm` y el nombre de la librería. Por ejemplo:

`npm rm @dcl-sdk/utils`

Es una buena práctica eliminar cualquier librería que no estés usando. Las librerías sin usar siguen ocupando espacio en la escena, lo que afecta la experiencia del jugador al descargar y ejecutar tu escena.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/libraries/manage-dependencies.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
