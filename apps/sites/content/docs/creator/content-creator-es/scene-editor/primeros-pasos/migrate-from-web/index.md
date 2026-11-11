> Migra tu escena desde el Web Editor al Creator Hub.

# Migrar a Creator Hub

Si tienes una escena creada con otras herramientas que no sean Creator Hub, puedes migrarla fácilmente a Creator Hub.

Creator Hub es la herramienta recomendada para crear escenas de Decentraland. Tiene una interfaz mucho más pulida que el Web Editor y te permite combinar la facilidad de una interfaz de arrastrar y soltar con la posibilidad de personalizar más con código. También te permite ejecutar la vista previa de tu escena usando el cliente de escritorio más reciente.

## Migrar desde el Web Editor

Para editar el código en una escena creada con el Web Editor, debes exportar la escena a tu máquina y abrirla con Creator Hub.


> [!WARNING]
> **📔 Nota**: Si no tienes Creator Hub instalado, sigue los pasos de la siguiente página antes de empezar.
> 
> [Instalar Creator Hub](/docs/creator/content-creator-es/scene-editor/primeros-pasos/editor-installation/)


1. Haz clic en el **Icono de descarga** en el menú superior del Web Editor mientras editas la escena.

![](/docs/_remote/013ac41c9f6c08ef06e89d47.webp)

2. Esto descargará un *.zip* archivo; extráelo.
3. Abre la **Creator Hub**, ve al **escenas** sección.
4. Haz clic en el **Importar escena** botón y selecciona la ruta a la carpeta de tu proyecto exportado.

![](/docs/_remote/7cb4f59604e7cd5cbb0a15bd.webp)

Una vez que termines, puedes seguir trabajando en tu proyecto dentro de Creator Hub, con una interfaz visual que se parece mucho al Web Editor, pero mucho más pulida.

También puedes editar los archivos dentro de la `/src` carpeta para añadir comportamiento con código a tu escena. Consulta [Combinar con código](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/overview/) para saber cómo editar el código de tu escena.

## Migrar un proyecto solo de código

Puedes importar cualquier proyecto solo de código a Creator Hub. Para hacerlo,

1. Abre Creator Hub, ve a la **escenas** sección.
2. Haz clic en el **Importar escena** botón y selecciona la ruta a la carpeta de tu proyecto exportado.

![](/docs/_remote/7cb4f59604e7cd5cbb0a15bd.webp)

Una vez hecho esto, puedes empezar a trabajar en tu proyecto dentro de Creator Hub; esto no impide que sigas usando tu editor de código favorito para editar el código de tu escena, o usar la línea de comandos para ejecutar o desplegar tu escena.

Después de importar tu proyecto, cualquier contenido creado mediante código no será visible ni editable en el canvas de Creator Hub, lo que puede dificultar colocar y alinear nuevos elementos. Inicialmente verás tu escena como una cuadrícula vacía.

![](/docs/_remote/3b0a67be118d41cad070cf4b.webp)

En lugar de añadir manualmente tu contenido al canvas desde cero, puedes ejecutar un comando para añadirlo automáticamente por ti. Para ello, asegúrate de tener instalada la versión más reciente del SDK y ejecuta el siguiente comando en tu terminal:

```
npx sdk-commands code-to-composite
```


> [!CAUTION]
> **❗Advertencia**: Asegúrate de tener una copia de seguridad de tu proyecto antes de ejecutar este comando.
> 
> Este comando sobrescribirá el `main.composite` archivo con la nueva instantánea. También comentará todo el código en los `.ts` archivos de la `src` carpeta. Tendrás que quitar los comentarios del código para que vuelva a ejecutarse.


Este comando ejecuta tu escena y toma una instantánea del contenido creado mediante código en el primer frame. Esta instantánea se guarda en el `main.composite` archivo, que Creator Hub usa para mostrar el contenido de tu escena. El código de tu escena está comentado para evitar tener duplicados de todas las entities.

Ten en cuenta que este comando solo captura entities y los components que pueden representarse en la UI de Creator Hub. No replica components personalizados, ni reproduce código que realiza lógica, ni elementos de UI que se crean mediante código. Para volver a añadir cualquier comportamiento que se haya comentado, tendrás que editar el código de la `.ts` archivos de la `src` carpeta y quitar los comentarios de las líneas que necesites.

También puedes querer reescribir parte del código para que, en lugar de crear nuevas entities, haga referencia a entities existentes por nombre o por tags para darles comportamiento. Consulta [Combinar con código](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/overview/) para ver cómo obtener estas entities desde tu código.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/primeros-pasos/migrate-from-web.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
