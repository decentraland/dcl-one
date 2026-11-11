> Obtén información de parcelas de LAND, estates o direcciones usando el CLI.

# Info de LAND

Puedes usar la CLI de Decentraland para consultar información directamente desde los tokens LAND en la blockchain y desde los archivos de la escena cargados en el content server.

Para ejecutar estos comandos, primero debes [Instalar la CLI](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/using-the-cli/).

## Obtener información sobre una escena

El `dcl info` comando devuelve el contenido del archivo de una escena, *scene.json* incluido el propietario, la información de contacto y las parcelas de la escena.

* `dcl info` desde la carpeta de la escena devuelve información sobre esa escena específica.
* `dcl info x,y`, donde *x* y *y* son coordenadas de parcela, devuelve información sobre la escena en esa ubicación.

  {% hint style="warning" %} **📔 Nota**: No agregues un espacio entre ambas coordenadas. {% endhint %}
* `dcl info id`, donde *id* es un ID de estate, devuelve información sobre la escena con ese ID de estate.

El `dcl status` El comando devuelve una lista con los archivos desplegados en una escena. Esto solo incluye nombres y tamaños de archivo. No puedes acceder al contenido de los archivos mediante la CLI.

* `dcl status` desde la carpeta de la escena devuelve información sobre esa escena específica.
* `dcl status x,y`, donde *x* y *y* son coordenadas de parcela (sin espacios entre ellas), devuelve información sobre la escena en esa ubicación.


> [!WARNING]
> **📔 Nota**: No agregues un espacio entre ambas coordenadas.


* `dcl status id`, donde *id* es un ID de estate, devuelve información sobre la escena con ese ID de estate.


> [!WARNING]
> **📔 Nota**: Todo lo que se sube a nuestro content server es público y accesible a través de esa red. Cuando despliegas una escena, de forma predeterminada no estás subiendo el código fuente original de la escena; en su lugar, subes una versión compilada a JavaScript minificado, que es mucho menos legible.


## Obtener información de una dirección de Wallet

El `dcl info` El comando también te permite ver información sobre una Wallet de Ethereum y los tokens LAND que posee.

* `dcl info xxxx`, donde *xxxx* es la dirección de Ethereum que te interesa, devuelve una lista de todas las parcelas y estates de LAND que posee esa dirección y sus detalles.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/marketplace/info-from-land.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
