> Ejecuta varios proyectos de DCL al mismo tiempo

# Espacios de trabajo

Ejecuta varios proyectos de Decentraland en vista previa agrupándolos en un espacio de trabajo. Ejecuta varias escenas adyacentes para ver cómo encajan, o también ejecuta varias [wearables inteligentes](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/smart-wearables/) juntas para ver cómo interactúan entre sí y con diferentes escenas.

Ejecutar varios proyectos en un espacio de trabajo proporciona una alternativa de prueba mucho más completa para asegurarse de que el contenido diferente funcione bien en conjunto. Un espacio de trabajo es una función de depuración; no afecta la experiencia en la escena publicada.


> [!WARNING]
> **📔 Nota**: El **Creator Hub** no admite actualmente el manejo de espacios de trabajo.


## Crear un espacio de trabajo

1. Descarga el [Goerli Plaza](https://github.com/decentraland/sdk7-goerli-plaza) repositorio.
2. Crea una carpeta independiente de nivel superior para alojar el espacio de trabajo.
3. Desde el repositorio de Goerli Plaza, copia los siguientes archivos a tu espacio de trabajo:
   * `dcl-workspace.json`
   * `package.json`
   * `.gitignore`
4. Dentro de esta carpeta, añade una carpeta en la raíz para cada proyecto con el que quieras trabajar. Puedes arrastrar carpetas existentes con escenas o smart wearables. Para carpetas nuevas, ejecuta `npx @dcl/sdk-commands init` dentro de cada una, para crear un proyecto de Decentraland.

   > Nota: Asegúrate de que los parcels de cada una de las escenas no se superpongan.
5. Poniéndote sobre la carpeta del espacio de trabajo, ejecuta lo siguiente para crear los archivos necesarios:

   `npm run update-parcels && npm run sync && npm run test && npm run format`

Puedes confirmar qué proyectos forman parte del espacio de trabajo abriendo el `dcl-workspace.json` archivo y comprobando las rutas enumeradas en sus `carpetas` array.

## Ejecutar un espacio de trabajo

Ejecuta `npm run start` en la carpeta raíz del espacio de trabajo. Esto ejecuta todos los proyectos al mismo tiempo, visibles en una sola ventana de vista previa. Esta vista previa se comporta igual que cuando se previsualiza una sola escena.

Cualquier smart wearable del espacio de trabajo está disponible para probarse buscándolo en la mochila.

## Añadir proyectos

Una vez creado un espacio de trabajo, puedes añadir proyectos adicionales editando el `dcl-workspace.json` archivo manualmente. Añade una entrada al `carpetas` array con la ruta relativa a la carpeta que quieres añadir. Por ejemplo `my-other-example`.

También puedes añadir un proyecto que no esté dentro de la carpeta del espacio de trabajo, usando la ruta absoluta.


> [!WARNING]
> **📔 Nota**: La carpeta ya debe contener un proyecto de Decentraland iniciado con `npx @dcl/sdk-commands init`. No puede ser una carpeta vacía.


Para añadir o eliminar proyectos, edita el `dcl-workspace.json` archivo para incluir las rutas relativas a cada uno de los proyectos del espacio de trabajo en el `carpetas` array.

```json
{
	"folders": [
		{
			"path": "example-scene"
		},
		{
			"path": "example-scene2"
		}
	]
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/workspaces.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
