> Conectando escenas con API externas, fuentes de datos y servidores de terceros.

# Servicios externos

Conectando escenas con API externas, fuentes de datos y servidores de terceros.

| Ejemplo                                                                                                                                          | Lo que demuestra                                                                                        | Mantenido por | Etiquetas                                      | Fuente                                                                                                   |
| ------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------- | ------------- | ---------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| [Datos desde una hoja de cálculo](/docs/creator/content-creator-es/tutoriales-y-ejemplos/external-services/spreadsheet-data/)                       | Contenido de la escena impulsado por una Hoja de cálculo de Google, actualizado sin volver a desplegar. | Decentraland  | `API`, `hoja de cálculo`, `video`, `wearables` | [Abrir repositorio](https://github.com/decentraland/Read_Spreadsheet_Scene)                              |
| [Libro de visitas de terceros](/docs/creator/content-creator-es/tutoriales-y-ejemplos/external-services/third-party-guest-book/)                    | Un libro de visitas respaldado por un servicio HTTP externo.                                            | Decentraland  | `API`, `almacenamiento`                        | [Abrir escena](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/guest-book-api)               |
| [Tabla de posiciones de terceros](/docs/creator/content-creator-es/tutoriales-y-ejemplos/external-services/third-party-leaderboard/)                | Una tabla de posiciones persistente respaldada por un servicio HTTP externo.                            | Decentraland  | `API`, `almacenamiento`, `tabla de posiciones` | [Abrir escena](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/leader-board)                 |
| [Autenticación de jugador de terceros](/docs/creator/content-creator-es/tutoriales-y-ejemplos/external-services/third-party-player-authentication/) | Verificando la identidad del jugador para solicitudes enviadas a un servicio externo.                   | Decentraland  | `API`, `autenticación`, `jugador`              | [Abrir escena](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/validate-player-authenticity) |

[Explorar todas las escenas de ejemplo](/docs/creator/content-creator-es/tutoriales-y-ejemplos/examples/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/external-services.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
