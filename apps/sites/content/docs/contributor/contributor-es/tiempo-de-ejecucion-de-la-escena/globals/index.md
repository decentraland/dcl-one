# Globales

El runtime de la scene garantiza la existencia de ciertos objetos y funciones dentro del entorno JavaScript aislado, además de las utilidades estándar, como `Promise`, `Date` o `Math`.


> [!NOTE]
> Los globals descritos a continuación son conceptos típicos de JavaScript, pero están adaptados al runtime de la scene de Decentrland y pueden no comportarse de forma idéntica a sus equivalentes en entornos de navegador o Node.


### Globales

El runtime inyecta 6 definiciones en el ámbito global de la scene:

1. [`console`](#console): una versión simplificada de la típica `console` objeto.
2. [`exports`](#module): un objeto donde la scene puede añadir su [interfaz pública](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/execution/).
3. [`module`](#module): un contenedor para el `exports` objeto.
4. [`require`](#module): una función para cargar módulos proporcionados por el runtime por nombre.
5. [`fetch`](#http): una implementación restringida de la función del navegador `fetch` función.
6. [`WebSocket`](#http): una implementación restringida de la función del navegador `WebSocket` clase.

Todas estas se definen como propiedades de solo lectura, por lo que no pueden reasignarse. Algunas lanzarán excepciones al usarse a menos que ciertos [permisos](/docs/contributor/contributor-es/contenido/tipos-de-entidad/scenes/#permissions) se otorguen a la scene.

### Consola <a href="#console" id="console"></a>

Las scenes tienen acceso a un `console` objeto, muy similar al que proporciona un entorno de navegador o Node, aunque limitado solo a algunos de los métodos que normalmente encontrarías.

```ts
type Console = {
  log(...args: any): void
  info(...args: any): void
  debug(...args: any): void
  warning(...args: any): void
  error(...args: any): void
}
```

Al igual que sus equivalentes estándar, cada método toma argumentos variables de cualquier tipo y los renderiza como mensajes legibles para humanos. Por ejemplo, esto es válido:

```js
console.log("The thing just appeared", { thing: "foo" }, [1, 2, 3])
```

El comportamiento preciso de estos métodos depende del proveedor, pero los mensajes deben ser accesibles para los desarrolladores de la scene que depuran su código.


> [!NOTE]
> En el World Explorer basado en navegador de la Foundation, los mensajes de log aparecen en el panel de herramientas para desarrolladores.


### Importaciones y exportaciones <a href="#module" id="module"></a>

Las scenes pueden importar y exportar objetos usando la tradicional [CommonJS](https://wiki.commonjs.org/wiki/Modules/1.0) interfaz de módulo.

```ts
exports: Object
require(moduleName: string): Object
```

El `require` La función permite a las scenes acceder a módulos proporcionados por el runtime (como [EngineApi](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/modulos-de-tiempo-de-ejecucion/engine-api/) o [RestrictedActions](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/modulos-de-tiempo-de-ejecucion/restricted-actions/)), y nada más (no **no** accede a paquetes NPM ni a módulos por ruta).

Las propiedades agregadas al `exports` objeto son la interfaz pública de la scene y se expondrán al runtime. De hecho, las scenes *deben* exponer al menos un método para funcionar correctamente (ver [ejecución](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/execution/)).


> [!NOTE]
> Las scenes escritas en lenguajes como TypeScript usan la más moderna `import` y `export` sentencias, que pueden transpilarse a usos compatibles con CommonJS de `require` y `exports`.


### HTTP y WebSockets <a href="#http" id="http"></a>

El `fetch` y `WebSocket` globals funcionan exactamente igual que sus conocidas contrapartes (ver [fetch](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API) y [WebSocket](https://developer.mozilla.org/en-US/docs/Web/API/WebSockets_API) en MDN), pero con algunas restricciones impuestas por el runtime.

Al llamar a la `fetch` función:

* Se lanza un error si la URL no comienza con `https://`.
* Se lanza un error si la scene no tiene el [`USE_FETCH` permiso](/docs/contributor/contributor-es/contenido/tipos-de-entidad/scenes/#permissions).
* Un timeout definido por la implementación puede abortar la request.

Al usar la `WebSocket` clase:

* Se lanza un error si la URL no comienza con `wss:`
* Se lanza un error si la scene no tiene el [`USE_WEBSOCKET` permiso](/docs/contributor/contributor-es/contenido/tipos-de-entidad/scenes/#permissions).

Aparte de estas diferencias, ambos casos siguen el comportamiento estándar.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/globals.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
