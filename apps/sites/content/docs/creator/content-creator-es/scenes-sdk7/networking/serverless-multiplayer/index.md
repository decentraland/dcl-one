> Sincroniza el estado de la escena entre jugadores.

# Multiplayer sin servidor

Decentraland ejecuta las escenas localmente en la instancia del explorer de un jugador. De forma predeterminada, los jugadores pueden verse entre sí e interactuar directamente, pero cada jugador interactúa con el entorno de forma independiente. Los cambios en el entorno no se comparten entre jugadores por defecto.

Ver el mismo contenido en el mismo estado es extremadamente importante para que los jugadores interactúen de formas más significativas.

Hay tres formas de sincronizar el estado de la escena, para que todos los jugadores vean lo mismo:

* **Marcar una entidad como sincronizada**: La opción más fácil. Ver [Marcó una entidad como sincronizada](#mark-an-entity-as-synced)
* **Enviar mensajes explícitos de MessageBus**: Envía y escucha manualmente mensajes específicos. Ver [Enviar mensajes explícitos de MessageBus](#send-explicit-messagebus-messages)
* **Usar un servidor multijugador**: Ver [servidor multijugador](/docs/creator/content-creator-es/scenes-sdk7/networking/authoritative-servers/). El servidor valida todos los cambios de estado y es la única fuente de verdad. Requiere más configuración, pero se recomienda encarecidamente cuando los jugadores tienen incentivos para explotar tu escena.

Las dos primeras opciones se tratan en este documento. Son más sencillas, ya que no requieren servidor. La desventaja es que dependes más de la velocidad de conexión de los jugadores, y el estado de la escena no se conserva cuando todos los jugadores abandonan la escena.

## Marcar una Entity como sincronizada

En el [Creator Hub](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/), marca una entidad como sincronizada añadiéndole un **componente Multiplayer** a ella. Incluye una casilla de verificación para cada uno de los demás componentes de la entidad, lo que te permite seleccionar cuáles actualizar.

![](/docs/_remote/710c6ed40b2f7a539e355f1b.webp)

Para marcar una entidad como sincronizada mediante código, usa la `syncEntity` función:

```ts
import { syncEntity } from "@dcl/sdk/network";

export function main() {
  const doorEntity = engine.addEntity();

  syncEntity(doorEntity, [Transform.componentId, Animator.componentId], 1);
}
```


> [!WARNING]
> **📔 Nota**: Llama siempre a `syncEntity()` dentro de la `main()` función, o en una función que se ejecute después de `main()` (como un callback o un system). Llamarla en el nivel superior de un archivo genera un error, porque en ese punto el perfil del jugador todavía no se ha inicializado.



> [!WARNING]
> **📔 Nota**: En el multijugador serverless, cada cliente llama a `syncEntity` por su cuenta. Si actualizas a un [servidor multijugador](/docs/creator/content-creator-es/scenes-sdk7/networking/authoritative-servers/), el patrón cambia: solo el servidor debería llamar a `syncEntity`, protegido por `isServer()`. Los clientes deberían evitar declarar la sincronización de entidades compartidas, a menos que sean creadas por el cliente.


La sección `syncEntity` La función toma las siguientes entradas:

* **entityId**: Una referencia a la entidad que se va a sincronizar
* **componentIds**: Una lista de los componentes que necesitan sincronizarse desde esa entidad. Es un array que puede contener tantos componentes como sea necesario. Todos los valores deben ser `componentId` propiedades.
* **entityEnumId**: (opcional) Un id único que se usa de forma consistente por todos los jugadores, ver [Acerca del id enum](#about-the-enum-id).

No todas las entidades o componentes necesitan sincronizarse. Los elementos estáticos, como un árbol que permanece en el mismo lugar, no requieren sincronización. En las entidades que sí sincronizas, solo deben sincronizarse los componentes que cambian con el tiempo. Por ejemplo, si un cubo cambia de color cuando se hace clic en él, solo deberías sincronizar el componente Material, no el MeshRenderer ni el Transform, ya que esos nunca cambiarán.


> [!NOTE]
> **💡 Consejo**: Si los datos que quieres compartir no existen como componente, define un [componente personalizado](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/custom-components/) que contenga esos datos.


### Sobre el id enum

La sección **entityEnumId** de una entidad debe ser único. No está relacionado con el entityId local asignado por `engine.addEntity()`, que se genera automáticamente y puede variar entre jugadores que ejecutan la misma escena. El entityEnumId de una entidad debe definirse explícitamente en el código y ser único.

Establecer este ID explícitamente es importante para evitar inconsistencias si una condición de carrera hace que una parte de la escena se cargue antes que otra. Quizá para el jugador A la puerta de la escena sea la entidad *512*, pero para el jugador B esa misma puerta sea la entidad *513*. En ese caso, si el jugador A abre la puerta, el jugador B ve en cambio que todo el edificio se mueve.


> [!NOTE]
> **💡 Consejo**: Crea un enum en tu escena para mantener referencias claras a cada id sincronizable en tu escena.
> 
> ```ts
> import { syncEntity } from "@dcl/sdk/network";
> 
> enum EntityEnumId {
>   DOOR = 1,
>   DRAW_BRIDGE = 2,
>   ELEVATOR = 3,
> }
> 
> export function main() {
>   syncEntity(
>     doorEntity,
>     [Transform.componentId, Animator.componentId],
>     EntityEnumId.DOOR
>   );
> }
> ```
> 
> Aquí el enum EntityEnumId se usa para etiquetar entidades con un identificador único, asegurando que cada cliente reconozca la entidad modificada, independientemente del orden de creación.



> [!WARNING]
> **📔 Nota**: Evita usar números superiores a **8001** Si tu escena también incluye Smart Items, [Creator Hub](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/) los items que se crean con un componente Multiplayer usarán IDs asignados automáticamente a partir de 8001. Cualquier ID inferior a 8001 es seguro para asignarlo a tus entidades sincronizadas.


**Entidades creadas por un jugador**

Si una entidad se crea como resultado de una interacción de un jugador, y esta entidad debe sincronizarse con otros jugadores, la entidad no necesita un entityEnumId. Puedes usar `syncEntity()` pasando solo la entidad y la lista de componentes. Se asigna automáticamente un valor único para entityEnumId detrás de escena.

Todas las entidades instanciadas al iniciar la escena necesitan tener un ID asignado manualmente. Eso garantiza que todos los jugadores usen el mismo ID en cada una. Cuando un solo jugador se encarga de instanciar una entidad, no hacen falta IDs explícitos. Los demás jugadores reciben actualizaciones sobre esta nueva entidad con un ID ya asignado, así que no hay riesgo de desajustes de ID.

Por ejemplo, en una escena de pelea de bolas de nieve, cada vez que un jugador lanza una bola de nieve, está instanciando una nueva entidad que se sincroniza con otros jugadores. La bola de nieve no necesita un entityEnumId único.

```ts
import { syncEntity } from "@dcl/sdk/network";

function onThrow() {
  const ball = engine.addEntity();
  Transform.create(ball, {});
  GltfContainer.create(ball, { src: "assets/snowBall.glb" });
  syncEntity(ball, [Transform.componentId, GltfContainer.componentId]);
}
```

### Entidades con parent <a href="#parented-entities" id="parented-entities"></a>

El parent de una entidad normalmente se define mediante `parent` propiedad en el `Transform` componente. Sin embargo, esta propiedad apunta al id local de la entidad padre, que podría variar; ver [Acerca del id enum](#about-the-enum-id). Para parentar entidades que necesiten sincronizarse, o que tengan hijos que necesiten sincronizarse, usa la `parentEntity()` función en lugar de la `Transform`.

```ts
import { syncEntity, parentEntity } from "@dcl/sdk/network";

export function main() {
  const parent = engine.addEntity();
  Transform.create(parent, { position: somePosition });
  syncEntity(parent, []);

  const child: Entity = engine.addEntity();
  syncEntity(child, [Transform.componentId]);

  parentEntity(child, parent);
}
```

Ten en cuenta que tanto el padre como el hijo se sincronizan con `syncEntity`, así que todos los jugadores tienen una comprensión común de qué ids usan ambas entidades. Esto es necesario incluso si los componentes del padre quizá nunca necesiten cambiar. En este ejemplo, la `syncEntity` incluye una matriz vacía de componentes, para evitar sincronizar componentes innecesarios.


> [!WARNING]
> **📔 Nota**: Si una entidad es parentada tanto por la `parentEntity()` y también por el `parent` propiedad en el `Transform` componente, la propiedad en el `Transform` componente se ignora.


Cuando las entidades se parentan mediante la `parentEntity()` función, también puedes usar las siguientes funciones auxiliares:

* **removeParent()**: Deshace los efectos de `parentEntity()`. Requiere que pases solo la entidad hija. El nuevo parent de la entidad pasa a ser la entidad raíz de la escena. La entidad padre original no se elimina de la escena.
* **getParent()**: Devuelve la entidad padre de una entidad que pasaste.
* **getChildren()**: Devuelve la lista de hijos de la entidad que pasaste, como un iterable.
* **getFirstChild()**: Devuelve el primer hijo de la lista para la entidad que pasaste.

```ts
import {
  syncEntity,
  parentEntity,
  getParent,
  getFirstChild,
  getChildren,
  removeParent,
} de "@dcl/sdk/network";

export function main() {
  const parent = engine.addEntity();
  Transform.create(parent, { position: somePosition });
  syncEntity(parent, []);

  const child: Entity = engine.addEntity();
  syncEntity(child, [Transform.componentId]);

  // establece parent como parent
  parentEntity(child, parent);

  // getParent
  const getParentResult = getParent(child);
  // devuelve parent

  // getFirstChild
  const getFirstChildResult = getFirstChild(parent);
  // devuelve child

  // getChildren
  const getChildrenResult = Array.from(getChildren(parent));
  // devuelve [child]

  // elimina parent del child
  removeParent(child);
}
```

## Comprobar el estado de sincronización

Cuando un jugador acaba de cargar una escena, es posible que todavía no esté sincronizado con los demás jugadores que lo rodean. Si el jugador empieza a alterar el estado del juego antes de estar sincronizado, esto podría causar problemas en tu juego. Recomendamos comprobar siempre que un jugador esté sincronizado antes de permitirle editar cualquier cosa de la escena.

Si un jugador sale de las parcelas de la escena, también estará desincronizado con la escena mientras permanezca fuera. Así que también es importante que los systems de la escena manejen ese escenario, ya que la escena sigue ejecutándose mientras el jugador esté cerca. Una vez que el jugador vuelve a entrar, se le actualiza automáticamente con cualquier cambio del estado de la escena.

Puedes comprobar si el estado de la escena está actualmente sincronizado para un jugador mediante la `isStateSyncronized()` función. Esta función devuelve un booleano, que es true si el jugador ya está sincronizado con la escena.

```ts
import { isStateSyncronized } from "@dcl/sdk/network";

const isConnected = isStateSyncronized();
```

Por ejemplo, podrías incluir esta comprobación en un system y bloquear cualquier interacción si esta función devuelve false.

```ts
import { isStateSyncronized } from "@dcl/sdk/network";

engine.addSystem(() => {
  if (isStateSyncronized() && !button.enabled) {
    console.log("Activar Start Game");
    button.enable();
  }

  if (!isStateSyncronized() && button.enabled) {
    console.log(`Desactivar Start Game.`);
    button.disable();
  }
});
```

## Enviar mensajes explícitos de MessageBus


> [!WARNING]
> **📔 Nota**: El `MessageBus` API está marcada como obsoleta en el SDK y podría eliminarse en una versión futura. Para la mayoría de los casos de uso, es preferible [marcar las entidades como sincronizadas](#mark-an-entity-as-synced).


**Iniciar un bus de mensajes**

Crea un objeto bus de mensajes para gestionar los métodos necesarios para enviar y recibir mensajes entre jugadores.

```ts
import { MessageBus } from "@dcl/sdk/message-bus";

const sceneMessageBus = new MessageBus();
```

**Enviar mensajes**

Usa el `.emit` comando del bus de mensajes para enviar un mensaje a todos los demás jugadores de la escena.

```ts
import { MessageBus } from "@dcl/sdk/message-bus";

const sceneMessageBus = new MessageBus();

const myEntity = engine.addEntity();
MeshRenderer.setBox(myEntity);
MeshCollider.setBox(myEntity);

pointerEventsSystem.onPointerDown(
  {
    entity: myEntity,
    opts: { button: InputAction.IA_PRIMARY, hoverText: "Clic" },
  },
  function () {
    sceneMessageBus.emit("box1Clicked", {});
  }
);
```

Cada mensaje puede contener un payload como segundo argumento. El payload es de tipo `Object`, y puede contener cualquier dato relevante que quieras enviar.

```ts
import { MessageBus } from "@dcl/sdk/message-bus";

const sceneMessageBus = new MessageBus();

sceneMessageBus.emit("spawn", { position: { x: 10, y: 2, z: 10 } });
```


> [!NOTE]
> **💡 Consejo**: Si necesitas que un solo mensaje incluya datos de más de una variable, crea un tipo personalizado para guardar todos estos datos en un solo objeto.


**Recibir mensajes**

Para gestionar mensajes de todos los demás jugadores de esa escena, usa `.on`. Al usar esta función, proporcionas una cadena de mensaje y defines una función a ejecutar. Cada vez que llega un mensaje con una cadena coincidente, la función dada se ejecuta una vez.

```ts
import { MessageBus } from "@dcl/sdk/message-bus";

const sceneMessageBus = new MessageBus();

type NewBoxPosition = {
  position: { x: number; y: number; z: number };
};

sceneMessageBus.on("spawn", (info: NewBoxPosition) => {
  const myEntity = engine.addEntity();
  Transform.create(myEntity, {
    position: { x: info.position.x, y: info.position.y, z: info.position.z },
  });
  MeshRenderer.setBox(myEntity);
  MeshCollider.setBox(myEntity);
});
```


> [!WARNING]
> **📔 Nota**: Los mensajes que envía un jugador también son captados por ese mismo jugador. El `.on` método no puede distinguir entre un mensaje emitido por ese mismo jugador y un mensaje emitido por otros jugadores.


**Ejemplo completo de MessageBus**

Este ejemplo usa un bus de mensajes para enviar un nuevo mensaje cada vez que se hace clic en el cubo principal, generando un nuevo cubo en una posición aleatoria. El mensaje incluye la posición del nuevo cubo, para que todos los jugadores vean estos nuevos cubos en las mismas posiciones.

```ts
import { MessageBus } from "@dcl/sdk/message-bus";

/// --- Crear bus de mensajes ---
const sceneMessageBus = new MessageBus();

// Fábrica de cubos
function createCube(x: number, y: number, z: number): Entity {
  const meshEntity = engine.addEntity();
  Transform.create(meshEntity, { position: { x, y, z } });
  MeshRenderer.setBox(meshEntity);
  MeshCollider.setBox(meshEntity);

  // Cuando se hace clic en un cubo, enviar mensaje para generar otro
  pointerEventsSystem.onPointerDown(
    {
      entity: meshEntity,
      opts: { button: InputAction.IA_PRIMARY, hoverText: "Pulsa E para generar" },
    },
    function () {
      sceneMessageBus.emit("spawn", {
        position: {
          x: 1 + Math.random() * 8,
          y: Math.random() * 8,
          z: 1 + Math.random() * 8,
        },
      });
    }
  );

  return meshEntity;
}

// Inicialización
createCube(8, 1, 8);

// definir el tipo de datos
type NewBoxPosition = {
  position: { x: number; y: number; z: number };
};

// al recibir el mensaje spawn, crear un nuevo cubo
sceneMessageBus.on("spawn", (info: NewBoxPosition) => {
  createCube(info.position.x, info.position.y, info.position.z);
});
```

## Probar localmente una escena multijugador

Si inicias una vista previa de la escena y la abres en dos (o más) ventanas diferentes de Explorer, cada ventana abierta se interpretará como un jugador separado, y un servidor simulado de comunicaciones mantendrá a estos jugadores sincronizados.

Interactúa con la escena en una ventana y luego cambia a la otra para ver que los efectos de esa interacción también son visibles allí.

Usando Creator Hub, activa **Vista previa multiinstancia** en el **Cliente de escritorio** submenú de **Opciones de reproducción**, luego haz clic en el **Reproducir** botón una segunda vez para abrir una segunda ventana de Explorer de Decentraland. Debes conectarte en ambas ventanas con direcciones diferentes. Las mismas sesiones permanecerán abiertas mientras la escena se recarga.

![](/docs/_remote/ca7e6550cdb1bb947ce74b28.webp)

Como alternativa, puedes abrir una segunda ventana de Explorer de Decentraland escribiendo lo siguiente en una URL del navegador:

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

## Escenas de un solo jugador

Si tu escena se despliega en un [Decentraland World](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/#decentraland-worlds), puedes convertirla en una escena de un solo jugador. Los jugadores no se verán entre sí, no podrán chatear ni ver los efectos de las acciones de los demás.

Para hacer esto, configura el `scene.json` archivo de la escena para establecer el **fixedAdapter** en `offline:offline`. La escena no tendrá ningún Communication Service en absoluto y cada usuario que se una a ese world siempre estará solo.

**Ejemplo:**

```json
{
  "worldConfiguration": {
    "name": "my-name.dcl.eth",
    "fixedAdapter": "offline:offline"
  }
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/networking/serverless-multiplayer.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
