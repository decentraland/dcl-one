> Genera un árbol de entities y Components en tiempo de ejecución a partir de un archivo composite

# Composites

A *composite* es un archivo que describe un árbol de [entities and components](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/entities-components/). En lugar de escribir código para crear cada entity una por una, puedes almacenar esa estructura en un archivo composite y generar todo con una sola llamada.

Puede que ya trabajes con composites sin darte cuenta:

* Cada escena tiene un `main.composite` archivo. Contiene todo lo que añadiste y configuraste visualmente en el [Scene Editor](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/). Este archivo se carga automáticamente cuando tu escena empieza.
* [Custom Items](/docs/creator/content-creator-es/scene-editor/interactividad/custom-items/) se guardan como `composite.json` archivos. Cada Custom Item es un composite que puedes reutilizar en distintas escenas.

Esta página explica cómo generar un composite desde código en tiempo de ejecución, por ejemplo para generar un Custom Item dinámicamente como parte de la lógica de tu escena.

## Generar un composite

Generar un composite requiere dos pasos: **cargar** el archivo composite, luego **generar** lo. La carga es asíncrona y la generación es síncrona, así que el patrón siempre es cargar y luego generar:

```ts
import { engine, Composite, getCompositeProvider } from "@dcl/sdk/ecs";

export async function spawnBarrel() {
  const src = "barrel.composite";
  const provider = getCompositeProvider();
  if (!provider || !provider.loadComposite) return;

  // 1. Cargar el composite desde su archivo
  const resource = await provider.loadComposite(src);

  // 2. Generarlo: crea todas sus entities y components
  const barrel = Composite.instance(engine, resource, provider);
  return barrel;
}
```

`provider.loadComposite(src)` lee el archivo composite y lo carga en memoria. En una escena normal construida con `@dcl/sdk`, ya tienes configurado un proveedor de composite, y puedes acceder a él con la `getCompositeProvider()` función de `@dcl/sdk/ecs`.

`Composite.instance()` luego crea todas las entities descritas en el composite, con todos sus components, y devuelve el **root entity** del árbol generado. Usa esta entity devuelta para leer o cambiar components más tarde, por ejemplo para reposicionar o eliminar el item generado.


> [!NOTE]
> **💡 Consejo**: Para hacer lo mismo sin escribir código, usa la **Spawn Entity** en el Scene Editor. Consulta [Acerca de generar entities](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items-advanced/#about-spawning-entities).



> [!WARNING]
> **📔 Nota**: Debes cargar un composite antes de generarlo. `Composite.instance()` es síncrono y solo puede generar un composite que ya esté en memoria. Solo `main.composite` main.composite `loadComposite()` primero. Para comprobar de forma síncrona si un composite ya está disponible, usa `provider.getCompositeOrNull(src)`.
> 
> `loadComposite()` es idempotente: asigna a cada composite una clave según su `src` cadena, así que volver a llamarlo con la misma ruta no vuelve a cargar el archivo, sino que devuelve el composite ya cargado. Puedes llamarlo con seguridad antes de cada generación sin preocuparte por cargar el mismo archivo dos veces.


### Generar sobre una entity existente

De forma predeterminada `Composite.instance()` crea una nueva entity para contener el árbol generado. Pasa un `rootEntity` en las opciones para generar sobre una entity que ya tienes:

```ts
// Genera el composite directamente en la raíz de la escena, sin entity contenedora
const barrel = Composite.instance(engine, resource, provider, {
  rootEntity: engine.RootEntity,
})
```

## Posicionar un composite generado

Para colocar el composite generado en una posición, rotación o escala específicas, define un `Transform` component en la root entity devuelta por `Composite.instance()`.

```ts
import { engine, Composite, getCompositeProvider, Transform } from "@dcl/sdk/ecs";
import { Vector3 } from "@dcl/sdk/math";

export async function spawnBarrel() {
  const src = "barrel.composite";
  const provider = getCompositeProvider();
  if (!provider || !provider.loadComposite) return;

  const resource = await provider.loadComposite(src);

  // Genera el composite en una posición específica
  const barrel = Composite.instance(engine, resource, provider);
  Transform.createOrReplace(barrel, {
    position: Vector3.create(8, 0, 8),
  });
}
```


> [!WARNING]
> **📔 Nota**: `Transform.createOrReplace()` reemplaza el componente Transform existente de la root entity. Solo se ve afectada la root entity; las child entities mantienen sus posiciones relativas a la root.


## Limitación actual: composites anidados

La generación funciona para composites autocontenidos y Custom Items. Un composite puede referenciar otro composite desde una de sus entities, pero `loadComposite()` lo hace **no** recorre de forma recursiva esas referencias anidadas: solo carga el archivo que le pasas.

Al generar, `Composite.instance()` recorre de forma recursiva las referencias anidadas que ya están en memoria. Si un composite anidado no está cargado (cualquier cosa distinta de `main.composite`main.composite, el único composite incluido con tu escena), esa rama registra una advertencia y se omite. Para generar un composite con referencias anidadas, llama a `loadComposite()` cada archivo referenciado primero — o, más simple, mantén autocontenidos los composites que generas.

## Páginas relacionadas

* [Entities & Components](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/entities-components/) — los bloques de construcción que describe un composite.
* [Custom Items](/docs/creator/content-creator-es/scene-editor/interactividad/custom-items/) — elementos reutilizables almacenados como composites.
* [Smart Items - Advanced](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items-advanced/#about-spawning-entities) — genera composites sin código, usando la acción Spawn Entity.
* [Archivos de escena](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-files/) — donde `main.composite` reside en tu proyecto de escena.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/arquitectura/composites.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
