> Contenido de la escena impulsado por una Hoja de cálculo de Google, actualizado sin volver a desplegar.

# Datos desde una hoja de cálculo

Impulsa el contenido de la escena desde una hoja de cálculo de Google. Edita la hoja para cambiar el nombre del artista, la transmisión de video y el wearable destacado sin volver a desplegar la escena.

## Información del proyecto

**Mantenido por:** Decentraland

[Ver fuente](https://github.com/decentraland/Read_Spreadsheet_Scene)

**Licencia:** [Apache-2.0](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/main/LICENSE). Consulta los términos upstream antes de reutilizarlo; el código y los assets pueden tener términos distintos.

## Explora el código

Estos enlaces apuntan a una instantánea específica de la fuente para que las explicaciones sigan siendo útiles a medida que continúa el desarrollo upstream. Son referencias de aprendizaje, no una evaluación de seguridad.

### Lee una hoja de cálculo de Google publicada como JSON

`getSpreadsheetData` solicita la hoja a través del endpoint de Google Visualization (`gviz/tq?tqx=out:json`), elimina el `setResponse(...)` wrapper y convierte cada fila en un objeto indexado por los encabezados de columna.

**Contexto del proyecto:** La hoja debe estar compartida para que cualquiera con el enlace pueda verla. `gid` selecciona la pestaña y `headers=1` trata la primera fila como nombres de columna.

**Referencias de la fuente:**

* [`getSpreadsheetData`](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/57bb4086aed3054d69a74f33f20f2d86c4eb9bb6/src/scheduler.ts#L1-L47)

### Aplicar una fila a la escena

`updateFromSpreadsheet` lee una fila y actualiza un banner `TextShape`, reemplaza el `VideoPlayer` source, intercambia el wearable mostrado en el dispensador y dirige la acción de clic del dispensador al enlace de compra de la fila.

**Contexto del proyecto:** Los nombres de columna como `NowPlaying`, `VideoURL`, `WearableURN` y `BuyURL` deben coincidir con los encabezados de la hoja. Las Entities se buscan por los nombres dados en la escena de Creator Hub.

**Referencias de la fuente:**

* [`updateFromSpreadsheet`](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/57bb4086aed3054d69a74f33f20f2d86c4eb9bb6/src/index.ts#L10-L64)
* [`createWearable`](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/57bb4086aed3054d69a74f33f20f2d86c4eb9bb6/src/index.ts#L100-L130)

### Recorre las filas desde un botón de Creator Hub

`main` escucha las `Next` y `Previous` acciones en la `Switch Artist` entity y reinicia el índice de fila sobre los datos descargados.

**Contexto del proyecto:** Usa `getActionEvents` de `@dcl/asset-packs`, así que las acciones se configuran en la entity en Creator Hub.

**Referencias de la fuente:**

* [`main`](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/57bb4086aed3054d69a74f33f20f2d86c4eb9bb6/src/index.ts#L69-L97)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/external-services/spreadsheet-data.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
