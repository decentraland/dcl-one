> Crea tus propias librerías de Decentraland para compartirlas con otros

# Crear una librería

Las bibliotecas son una excelente forma de compartir soluciones a problemas comunes. Los desafíos complejos se pueden abordar una vez, encapsular las soluciones en una biblioteca y, cada vez que aparezcan, solo tendrás que escribir una línea de código. Al compartir bibliotecas con la comunidad, podemos hacer que la productividad de todos los creadores crezca exponencialmente.

Actualmente, estas bibliotecas en la [página de ejemplos](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library) están disponibles para que todos las usen. Te animamos a crear y compartir también las tuyas.

Siguiendo los pasos detallados aquí, puedes evitar la mayor parte de la complejidad que conlleva crear una biblioteca compatible con las scenes de Decentraland y fácil de compartir con otros.

## Crear una biblioteca

> **📔 Nota**: Asegúrate de estar usando Node versión 20 o posterior antes de compilar tu biblioteca.

Para crear tu propia biblioteca y compartirla a través de NPM, haz lo siguiente:

1. Crea una carpeta nueva y vacía, y abre allí una nueva ventana de Visual Studio Code.
2. Selecciona la pestaña de Decentraland en la barra lateral izquierda de Visual Studio Code
3. Haz clic **Crear proyecto**, luego selecciona **Biblioteca**

   Esto creará todos los archivos y dependencias predeterminados para una biblioteca de Decentraland.
4. Establece un nombre único `name` en `package.json`. Este es el nombre que se usará al publicar en NPM; asegúrate de que no exista ya otro proyecto que use ese nombre en npm. También completa la descripción y cualquier etiqueta para ayudar a otros a encontrar tu biblioteca.
5. Crea un nuevo *públicas* repositorio de GitHub para tu proyecto.

   El proyecto está configurado para usar GitHub Actions y publicar una nueva versión del paquete con cada push a `main`.

   En la `package.json` archivo, asegúrate de que en `repositorio` configures el `campo URL` a la URL de tu repositorio. Así será fácil de encontrar para los usuarios que naveguen [npmjs.com](https://www.npmjs.com).
6. Obtén un token de NPM:
   1. Crea una cuenta o inicia sesión en <https://www.npmjs.com/>.
   2. Ve a **Cuenta > Tokens de acceso > Generar nuevo token** Crea un nuevo token y asígnale **Publicar** permisos.

      El mensaje de éxito incluye una cadena del token recién generado. Copia esta cadena y guárdala en un lugar seguro.
7. En tu repositorio de github, ve a **Configuración > Secretos > Acciones**, luego haz clic en **Nuevo secreto del repositorio**.

   Nombra tu secreto **NPM\_TOKEN**, y pega la cadena del token de NPM como su valor.
8. Haz push de cualquier cambio (cualquier cambio) a la rama principal de tu repositorio de GitHub y el paquete se publicará.

   Eso es todo; ahora cualquiera puede instalar el paquete usando `npm i <package-name>`! Ahora puedes encontrar tu biblioteca si la buscas por nombre en [npmjs.com](https://www.npmjs.com).
9. Completa la `/src` carpeta de tu proyecto con toda la funcionalidad que quieras exponer y sube los cambios a GitHub.

## Desarrolla

Añade cualquier función, componente, etc. que quieras exponer en la `index.ts` archivo de la biblioteca, así será fácil importarlos.

Por ejemplo, si añades un `RotatorComponent` componente a `index.ts`, entonces puedes importar este componente en una escena escribiendo lo siguiente:

`import { RotatorComponent } from 'my-library'`

Prueba tu biblioteca usándola en una nueva escena de Decentraland mientras la desarrollas. Instala tu paquete npm en una escena ejecutando `npm i <package-name>`. Luego crea una escena que sirva para probar la funcionalidad de la biblioteca. Intenta cubrir distintos casos de prueba con tu escena para asegurarte de que la biblioteca funcione como se espera en cada caso.

### Iteraciones rápidas

Si necesitas hacer pequeños ajustes a tu biblioteca de forma continua y probarlos, puede resultar agotador tener que hacer commit de los cambios y luego esperar a que se complete la publicación en npm antes de poder probar la nueva versión en una escena. Por suerte, hay una alternativa mucho más rápida para ejecutar pruebas con tu biblioteca.

En un **proyecto de una escena de prueba**:

1. Añade este script a la `lista de scripts` en package.json: `"link-sdk": "cd node_modules/@dcl/sdk && npm link && cd ../js-runtime && npm link"`
2. Ejecuta `npm install`
3. Ejecuta `npm install YOUR_LIBRARY_PATH`, por ejemplo, `npm install /User/projects/dcl-sdk7-library-test`
4. Ejecuta `npm run link-sdk`

En la **proyecto de biblioteca**:

1. Ejecuta `npm install`
2. Ejecuta `npm run build`
3. Ejecuta: `npm link @dcl/sdk @dcl/js-runtime`

> **❗Advertencia**: El orden de estos pasos es importante. Puede que no funcione en otro orden.

Esto mantendrá tu escena sincronizada con la versión de la biblioteca que está directamente en tu unidad local. Para cualquier cambio en la biblioteca que quieras probar, simplemente ejecuta `npm run build` en la carpeta de la biblioteca; no hace falta publicar los cambios en GitHub o NPM.

> **💡 Consejo**: Para verificar que el enlace se realizó correctamente, ejecuta `npm ls --link`. Deberías ver el nombre de la biblioteca apuntando a la carpeta en tus archivos locales.

Si haces cambios en la biblioteca, debes ejecutar `npm run build` para actualizarlos. Para evitar tener que hacerlo cada vez:

1. Añade el script "start": "tsc -p tsconfig.json --watch" a la biblioteca
2. Ejecuta `npm start` en la biblioteca

Cuando termines de probar, recuerda desvincular la biblioteca.

1. En la carpeta de la escena ejecuta `npm install <library name>`
2. Luego, en la biblioteca, ejecuta `rm -rf node_modules && npm install`

## Control de versiones

Las versiones de tu biblioteca se publican automáticamente en `npm` con un `@latest` y una `@next` etiqueta.

El `@next` La etiqueta siempre apunta al último commit en la `main` rama. Esta versión puede ser inestable, ya que es posible que los últimos cambios no estén probados. Los usuarios de tu biblioteca pueden instalarla (bajo su propia responsabilidad) haciendo `npm i <library name>@next`.

El `@latest` La etiqueta apunta a la última versión estable de la biblioteca. Esta es la que los usuarios de tu biblioteca deberían instalar normalmente. Es la versión que npm obtiene cuando ejecutan `npm i <library name>`.

Para hacer que la `@latest` etiqueta apunte a tus commits más recientes, tendrás que crear un **lanzamiento** de tu biblioteca en GitHub.

1. Abre la página de GitHub de tu proyecto. Abre el **Lanzamientos** enlace, en el margen derecho de la página.
2. Haz clic **Borrador de nueva versión**.
3. On **Elige la etiqueta** escribe un nombre para tu nueva versión, por ejemplo "1.1.0". También escribe un nombre en **Título del lanzamiento**. A menudo es el mismo nombre, "1.1.0".

   > Importante: la etiqueta debe ser un número, sin letras ni símbolos adicionales. El número debe ser mayor que el de las versiones publicadas anteriormente.
4. Describe tu lanzamiento para que los usuarios sepan qué hay de nuevo.

   > Consejo: haz clic en el botón **Generar notas del lanzamiento automáticamente** para mostrar todos los commits desde el último lanzamiento.
5. Haz clic en **Publicar lanzamiento**. Esta acción activa una nueva publicación automática en npm con la `@latest` etiqueta. Los usuarios de tu biblioteca ahora descargarán esta versión.

## Notas sobre usabilidad

Haz todo lo posible para que tu biblioteca sea fácil de usar para otros creadores. Nuestras suposiciones siempre ayudan a dar forma a las herramientas que creamos. A menudo estas suposiciones nos parecen obvias, pero otras personas con un contexto distinto pueden tener suposiciones completamente diferentes.

* Elige cuidadosamente los nombres de todo en tu biblioteca, incluidas las funciones, componentes y parámetros. Es mejor tener un nombre largo y autoexplicativo que uno corto que sea un completo misterio.
* Piensa ampliamente en los distintos casos de uso posibles para tu biblioteca. Quizá alguien use tu biblioteca en un smart wearable, quizá alguien necesite encender o apagar tu sistema en ciertos momentos, quizá alguien necesite añadir varias instancias de tu sistema en la misma scene. No todos se enfrentarán a los mismos desafíos que tú.
* Dicho esto, no necesitas soportar todos los casos de uso posibles. De hecho, siempre hay que hacer concesiones entre flexibilidad y simplicidad de uso; crear buenas herramientas consiste en encontrar el equilibrio adecuado. Sé estratégico al definir hasta dónde llega lo que decidiste no soportar. Aun así, es importante documentar claramente las limitaciones que elegiste.
* Esto nos lleva a la documentación. Documenta tu biblioteca con claridad, con ejemplos, descripciones de lo que hace cada parámetro, comentarios sobre qué escenarios no funcionarían y qué suposiciones estás haciendo sobre el contexto en el que se usa. El README.md de la biblioteca suele ser el mejor lugar para mostrar este contenido.
* Otra gran herramienta es añadir comentarios de metadatos directamente en tu código. Estos comentarios se muestran en el IDE (como Visual Studio Code) mientras los usuarios de tu biblioteca escriben. Puedes describir para qué sirve cada función/componente, describir qué espera recibir cada parámetro y qué devuelve una función. Gracias a esto, los usuarios de tu biblioteca no necesitan alternar entre el código y la documentación; ¡todo está ahí en un solo lugar! Por ejemplo, mira esta función de la `@dcl/ecs-scene-utils` biblioteca:

  ```ts
  /**
   * Limita un valor para que no supere un valor mínimo ni un valor máximo.
   *
   * @param value - número de entrada
   * @param min - valor mínimo de salida.
   * @param max - valor máximo de salida.
   * @returns El valor resultante mapeado entre el mínimo y el máximo
   * @public
   */
  export function clamp(value: number, min: number, max: number) {
  	let result = value

  	if (value > max) {
  		result = max
  	} else if (value < min) {
  		result = min
  	}
  	return result
  }
  ```

  Los usuarios de esta biblioteca ven estas sugerencias mostradas mientras escriben la `clamp()` función:
* Declara siempre los tipos. No dejes que la gente adivine qué estructura de objeto tendrá que consumir.
* Mantén tu biblioteca ligera y enfocada en una sola funcionalidad. Cuando se compila una escena de Decentraland, todo el código de todas sus bibliotecas se empaqueta con ella, incluso el código que la escena nunca llama. Por esta razón, es mejor evitar crear bibliotecas grandes y voluminosas. Es mejor tener bibliotecas ligeras que permitan a los creadores usar solo lo que realmente necesitan.
* Mantén un ojo en el repo de tu biblioteca; la gente podría hacer pull requests o reportar issues.
* Incluye la información de la licencia en tu repo, así otros sabrán si pueden usar tu biblioteca libremente. La biblioteca predeterminada incluye una licencia Apache 2 abierta; si quieres, puedes cambiarla.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/libraries/create-libraries.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
