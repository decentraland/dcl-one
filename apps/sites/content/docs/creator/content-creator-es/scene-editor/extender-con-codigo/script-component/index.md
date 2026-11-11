> Usa el Script Component para darle funcionalidad al código, sin necesidad de profundizar en toda la estructura del proyecto.

# Usando el Script Component

Con el nuevo Script Component, es posible crear Entities que ejecutan código personalizado desde dentro de la propia entidad.

Los Script Components permiten la ejecución del comportamiento personalizado de un Entity sin necesidad de trabajar directamente sobre el `index.ts` y potencialmente otros archivos.

## Configurar el Script Component

1. Añade el Script Component a un Entity haciendo clic en el `+` botón y selecciónalo. Crea un nuevo Script haciendo clic en **+ Añadir nuevo módulo de Script** y elige un nombre, o usando la Ruta del archivo (explora o arrastra y suelta un archivo existente).

![](/docs/_remote/2c5b0bcd10d2297a8c766d9b.webp)

2. Haz clic en el botón CODE del componente para abrir el editor de código predeterminado. Revisemos su estructura. Para más detalles sobre cómo seleccionar y gestionar tu editor predeterminado, por favor ve a [Combinar con código](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/overview/).

## Entender la estructura del Script

Cuando se abre el Script por primera vez, tiene el siguiente código:

```ts
import { engine, Entity } from '@dcl/sdk/ecs'
import {} from '@dcl/sdk/math'

export class BuildingScript {
  /**
   * Propiedades
   * Define los campos de la clase que quieras reutilizar entre métodos.
   * Ejemplo de uso: this.myVariable
   */
   // variable privada myVariable: boolean = true

  /**
   * Constructor / Entradas
   * Los parámetros declarados aquí aparecen en la UI del Script Component en Creator Hub.
   * Tipos compatibles: Entity, String, Number, Boolean, ActionCallback y Slider
   * para un número editado con un slider, por ejemplo public speed: Slider<0, 10, 0.5> = 1
   *
   * Nota: después de editar este archivo, haz clic en el icono de actualizar en la UI del Script component
   * para ver las entradas actualizadas.
   *
   * Los campos `src` y `entity` en el constructor son requeridos por referencias internas.
   */
  constructor(
    public src: string,     // NO ELIMINAR
    public entity: Entity,   // NO ELIMINAR
    // Añade tus entradas personalizadas abajo
  ) {}

  /**
   * start()
   * Se llama una vez cuando el script se inicializa.
   */
  start() {
    // Inicialización del Script
    console.log("BuildingScript inicializado para la entidad:", this.entity);
  }

  /**
   * update(dt)
   * Se llama en cada frame.
   * @param dt - (opcional) Tiempo delta desde el último frame (en segundos)
   */
  update(dt: number) {
    // Se llama en cada frame
  }
}
```

La clase se compone de tres partes principales:

* El **constructor**,
* el **start()** method
* el **update()** método.

## Constructor

El constructor contiene los parámetros que quieres exponer y modificar dinámicamente desde tu escena en Creator Hub.

```ts
export class BuildingScript {
  constructor(
    public src: string,
    public entity: Entity,
    public numericVariable: number, 
  ) {}
...
}
```

Una vez guardado el archivo, el **Refresh** botón del Script Component actualiza todos los cambios realizados.

<img src="/docs/_remote/f1c901d70e456b61b41d362b.webp" alt="Botón Refresh" width="360">

Una vez actualizado, el Script Component ahora muestra el `numericVariable` añadido en el código.

![](/docs/_remote/7bf7d0505e8aa1c4870ff35a.webp)

## Parámetros

Si diferentes Entities usan el mismo archivo en el Script component, cada una sigue teniendo parámetros independientes: si la escena tiene dos edificios, `building1` y `building2`, ambos con un Script Component apuntando a `BuildingScript.ts` archivo, cada edificio tiene sus propios `numericVariable` parámetro que puede modificarse independientemente.


> [!WARNING]
> **Nota importante**: No modifiques/elimines `public src: string` y `public entity: Entity`. Puedes añadir nuevas entradas siguiendo estas.


Los tipos permitidos para los parámetros del constructor son:

* `Entity`
* `cadena`
* `number`
* `booleano`
* `ActionCallback`
* `Slider<Min, Max, Step>`

### Parámetros numéricos con un slider

Escribe un parámetro numérico como `Slider<Min, Max, Step>` para editarlo con un slider en lugar de un simple cuadro numérico. Creator Hub muestra tanto un slider como un cuadro numérico.

```ts
constructor(
  public src: string,
  public entity: Entity,
  // Un slider de 0 a 10, moviéndose en pasos de 0,5
  public speed: Slider<0, 10, 0.5> = 1,
) {}
```

* `Paso` es opcional y por defecto es `1`.
* Se permiten límites negativos, por ejemplo `Slider<-90, 90>`.
* En tiempo de ejecución, el valor es un simple `number`cero, así que `this.speed` se comporta como cualquier otro parámetro numérico.


> [!NOTE]
> **📔 Nota**: Ambos `públicas` y `private` parámetros del constructor se exponen a Creator Hub. El `private` keyword solo restringe el acceso dentro de la `BuildingScript` clase. Para más detalles, consulta la documentación oficial de TypeScript sobre\
> [Propiedades de parámetros](https://www.typescriptlang.org/docs/handbook/2/classes.html#parameter-properties).


### Acceder a los parámetros dentro del Script

Para acceder al valor de un parámetro desde tu código, usa la notación `this.definedParameter`. Por ejemplo, `this.numericVariable` o `this.entity`.

La plantilla predeterminada de Script incluye esta línea en el método start():

`console.log("BuildingScript inicializado para la entidad:", this.entity);`.

Cámbiala así para registrar el valor de un valor que definiste en el constructor:

`console.log("BuildingScript inicializado con numericVariable:`, `this.numericVariable);`

Ten en cuenta que cuando cambias el valor del parámetro en la UI de Creator Hub, también deberías ver que este valor registrado se actualiza en consecuencia.

### Parámetros predeterminados

El constructor, por defecto, contiene un `src` y un `entity` parámetro; este es muy útil para el código de tu script:

* `this.entity` siempre se refiere al Entity que contiene el `Script` componente; úsalo para acceder a información sobre el Entity o añadirle componentes.
* `this.src` es la ruta donde se almacena el script. Esto es especialmente útil al crear Smart Items que están pensados para que otros los usen. Usa este campo para construir la ruta a los archivos que se empaquetan con tu smart item, incluso si la ruta del smart item cambia o se renombra.

```ts
export class BuildingScript {
  constructor(
    public src: string,
    public entity: Entity,
  ) {}

  start() {
    Material.setPbrMaterial(this.entity, {
      texture: Material.Texture.Common({
        src: this.src + '/images/myImage.png',
      })
    });
  }
}
```

El script anterior obtiene el Entity que posee el script y le aplica una textura. Obtiene la textura de un `.png` archivo que está empaquetado en la carpeta del smart item, en una subcarpeta llamada `/images`. Al usar `this.src`, nos aseguramos de que la ruta del archivo se conozca siempre, sin importar si el smart item se importa a la escena bajo `/assets/custom/itemName` o `/assets/asset-packs/itemName`

### Tooltips en los parámetros

Añade tooltips a tus parámetros de entrada, para que los usuarios sepan para qué se usan estos campos o qué valores se aceptan. Los usuarios verán un icono de tooltip junto a cada campo en la UI del Script component, y podrán leer texto personalizado al pasar el cursor sobre el icono.

Para añadir tooltips a tu constructor, añade un bloque comentado justo antes del constructor y escribe una línea con `@param` más el nombre del campo, seguido de una descripción, para cada tooltip.

```ts
  /**
   * @param startDate - La fecha de inicio del evento en formato YYYY-MM-DD
   * @param yOffset - Cuántos metros por encima del suelo mostrar el elemento
   */
  constructor(
    public src: string,
    public entity: Entity,
    public startDate?: string,
    public yOffset: number = 0.5,
  ) {
  }
```

Puede que necesites hacer clic en el icono de actualizar en la UI del Script component para ver los cambios en tus tooltips.

<img src="/docs/_remote/f1c901d70e456b61b41d362b.webp" alt="Botón Refresh" width="360">

## Método start() & update()

El **start()** El método contiene código que se ejecuta solo una vez, cuando se crea el Entity (en este caso, cuando la escena carga por primera vez).

Previsualiza la escena y revisa los logs (**Consejo**: puedes usar el `` ` `` atajo): Muestra el nuevo mensaje incluyendo el `numericVariable` parámetro.

![](/docs/_remote/3391665f06d28b07d1c783f1.webp)

El **update()** método, por otro lado, ejecuta su código en cada frame del juego (como hacen los Systems). Por ejemplo, comprobar valores del `PlayerEntity` para activar comportamientos en el script.

El siguiente código imprime Logs en cada frame del juego en el que el `PlayerEntity` es mayor que el previamente definido `numericVariable`, que es proporcionado dinámicamente por el creador desde la UI del Script Component.

```ts
update(dt: number) {
    if (Transform.get(engine.PlayerEntity).position.y > this.numericVariable ) {
      console.log("La altura del jugador supera ", this.numericVariable);
    }}
```

<img src="/docs/_remote/cff6c35c55f5fdd39baaefae.webp" alt="Update Method" data-size="line">

El primer log pertenece al método start(), indicando que establecemos numericVariable. El segundo pertenece al método update(), cuando el jugador está por encima de ese valor.

## Exponer acciones a Creator Hub

Es posible definir un `Acción` dentro de un Script Component script y que esté accesible en la UI de Creator Hub. Esto habilita la posibilidad de activar este `Acción` con otro Entity.

```ts
  /**
   * Expón esta acción para activarla
   * @action
   */
  exposedAction(creatorHubParameter: number) {
    console.log("Activado desde otro Entity usando el parámetro: ", this.creatorHubParameter);
  }
```

`creatorHubParameter` se expondrá como un `Acción` parámetro para darle un valor personalizado. Tras actualizar el Script Component, la nueva acción estará disponible como una opción en el desplegable de Actions.

![](/docs/_remote/eb92dee98d90a44d44de70b3.webp)

Después de añadir la Action, cualquier Entity en Creator Hub puede activarla usando `Triggers`

![](/docs/_remote/e8bcd93f2829f1275473cb37.webp)


> [!NOTE]
> **📔 Nota**: Puedes añadir tantas Actions como necesites dentro del Script. Todas ellas serán accesibles de forma independiente desde el `Acción` desplegable.


## Llamar métodos de Script desde fuera

Para llamar a un método de Script desde otro Script o desde `src/index.ts`, deben seguirse los siguientes pasos:

1. Crea una `públicas` método dentro de la clase Script.
2. Ejecuta `npm run build` desde el directorio raíz de la escena.
3. Desde el archivo donde quieras usar el método público, añade `import { callScriptMethod } from '~sdk/script-utils'`.
4. Llama a `callScriptMethod` con los parámetros necesarios (en este caso, `someParamter`).

Aquí hay un ejemplo con un `públicas` método expuesto

```ts
export class BuildingScript {
  constructor(
    public src: string,
    public entity: Entity,
    ...,
  ) {}

  public publicMethod(boolParameter: boolean, someNumberParameter: number) {
    if (boolParameter) {
      console.log("Método público llamado con el parámetro true!: ", someNumberParameter);
    } else {
      console.log("Método público llamado con el parámetro: false!", someNumberParameter);
    }
  }
...
}
```

Para llamarlo desde `src/index.ts`, usa:

```ts
import { callScriptMethod } from '~sdk/script-utils'


export function main() {
    const buildingEntity = engine.getEntityOrNullByName("building")
    if (buildingEntity) {
        const scriptMethod = callScriptMethod(
            buildingEntity,
            "assets/scene/Scripts/BuildingScript.tsx",
            "publicMethod",
            false,
            3,
        )

        scriptMethod
    }
}
```

Primero, la `main` función busca el `Entity` que tiene el Script component. Segundo, si el `Entity` existe, `callScriptMethod` se llama con los siguientes parámetros:

1. `entity`: `Entity` que tiene el `públicas` método.
2. `scriptPath`: `path` donde vive la `Script` clase.
3. `methodName`: nombre del `públicas` método a llamar.
4. `...args`: Argumentos del método. En este caso, hay dos. Deben añadirse en orden, uno tras otro.

Tercero, llamamos al `callScriptMethod`, en este caso, `scriptMethod`.

Con los valores de los parámetros dados, la salida es:

![](/docs/_remote/732f383d31c15cab43ea285e.webp)


> [!NOTE]
> **📔 Nota**: Puedes seguir la misma lógica para llamar a un `públicas` método de Script desde otro script o archivo. Puedes usarlo para obtener o cambiar valores de `públicas` variables de la clase Script.


## Activar Actions de otros Entities desde un Script

Es posible usar un parámetro de tipo `ActionCallback` en el constructor de la clase Script. Esto permite activar el `Entity`de `Acción` definida a través de la UI de Creator Hub desde los métodos del Script.

En este ejemplo, `anotherEntityAction` se añade como un `públicas` parámetro.

```ts
export class BuildingScript {
  constructor(
    public src: string,
    public entity: Entity,
    public anotherEntityAction: ActionCallback,
    ...,
  ) {}
  ...
}
```

Una acción seleccionable `Entity` y `Acción` ya están disponibles cuando se actualiza el Script Component en la UI de Creator Hub. `Sphere` es un Entity ya existente en la escena que tiene una acción llamada `Escala`.

![](/docs/_remote/b12b866e818c2c2562ab037c.webp)

La acción del otro Entity ahora es accesible en la clase Script. Podría usarse de muchas maneras diferentes. En el siguiente ejemplo, al pulsar E se activará `this.anotherEntityAction` definiendo un `pointerEventsSystem` en el `inicio` método.

```ts
  start() {
    pointerEventsSystem.onPointerDown(
      {
        entity: this.entity,
        opts: {
          button: InputAction.IA_PRIMARY,
          hoverText: "Pulsa E para activar una Action desde otro Entity.",
        },
      },
      () => {
        this.anotherEntityAction();
      }
    );
  }
```

### Callbacks de acción opcionales

Un `ActionCallback` parámetro que el creador deja sin conectar es `undefined`, y su tipo es `ActionCallback | undefined`. Compruébalo antes de llamarlo, para que un script funcione tanto si la acción está conectada como si no:

```ts
if (this.anotherEntityAction) {
  this.anotherEntityAction();
}
```


> [!NOTE]
> **📔 Nota**: Combinar exponer y activar `Actions` es una herramienta muy poderosa. Puedes definir un Script Component en un Entity, exponer una acción usando un `públicas` método, y luego activarla desde el Script Component de otro Entity usando un `ActionCallback` parámetro.


## Ver también

* [Smart items - Conceptos básicos](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/)
* [Smart items - Advanced](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items-advanced/)
* [Estados y condiciones](/docs/creator/content-creator-es/scene-editor/interactividad/states-and-conditions/)
* [Hacer que cualquier objeto sea smart](/docs/creator/content-creator-es/scene-editor/interactividad/make-any-item-smart/)
* [Inicio rápido de SDK](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/sdk-101/): sigue este mini tutorial para una introducción rápida.
* [Flujo de trabajo de desarrollo](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/dev-workflow/): léelo para entender la creación de escenas de principio a fin.
* [Ejemplos](/docs/creator/content-creator-es/tutoriales-y-ejemplos/examples/): lánzate directamente a escenas de ejemplo funcionales.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/extender-con-codigo/script-component.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
