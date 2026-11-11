> Oculta Avatares, oculta los nametags o desactiva los pasaportes para los jugadores dentro de una región de tu escena

# Áreas modificadoras de Avatar

Los avatars se comportan y se ven de forma consistente en todo Decentraland mientras recorren scenes. Sin embargo, puedes añadir un `AvatarModifierArea` a una región de tu scene para afectar cómo se comportan los avatars de los jugadores cuando entran en esa área.


> [!CAUTION]
> **❗Advertencia**\
> Por favor, limita la cantidad de `AvatarModifierAreas` que usas en tu scene a solo unas pocas. Si usas demasiadas, puede tener un impacto significativo en el rendimiento.


### Colocación de Avatar Modifier Areas

Añade una entity con un `AvatarModifierArea` component y posiciona esta entity usando un `Transform` componente.

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```

Al crear un `AvatarModifierArea` component, debes proporcionar lo siguiente:

* `area`: Tamaño del área del modificador
* `modifiers`: Un array que enumera los modifiers a implementar en el área. Esta propiedad usa valores de `AvatarModifierType` enum.

Los modifiers compatibles son:

* `AvatarModifierType.AMT_HIDE_AVATARS`
* `AvatarModifierType.AMT_DISABLE_PASSPORTS`
* `AvatarModifierType.AMT_HIDE_NAMETAGS`

Todos los efectos de un `AvatarModifierArea` solo tienen lugar dentro de la región de su área. Los jugadores vuelven a la normalidad cuando salen del área.

Una `AvatarModifierArea` afecta solo a los jugadores que están dentro del área. Entrar en el área no afecta a cómo se percibe a los otros jugadores fuera del área.

Los efectos de un `AvatarModifierArea` se calculan localmente para cada jugador. Puedes tener un `AvatarModifierArea` que solo está presente en la scene para algunos jugadores y no para otros. Por ejemplo, podrías hacer un juego de "marco polo", donde solo un jugador en la scene tenga un área de modificación que oculte a todos los demás jugadores. Todos los otros jugadores que no tengan esta área de modificación en su versión local de la scene pueden verse entre sí con normalidad.

Si el área oculta avatars, entonces los jugadores que no tengan el área en su versión local de la scene verán todos los avatars con normalidad, incluso aquellos que se experimentan a sí mismos como ocultos. Los jugadores que sí tengan el área se experimentarán a sí mismos y a todos los demás avatars como afectados por el área cuando entren en ella.


> [!WARNING]
> **📔 Nota**: Las áreas de modificación de avatar se ven afectadas por el *posición* y *rotación* del component Transform de su entity anfitriona, pero no se ven afectadas por el *escala*.



> [!WARNING]
> **📔 Nota**: Los `AvatarModifierArea`component debe importarse mediante
> 
> > `import { AvatarModifierArea } from "@dcl/sdk/ecs"`
> 
> Consulta [Importaciones](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes/#imports) para ver cómo manejarlos fácilmente.


### Ocultar avatars

Cuando un jugador entra en un `AvatarModifierArea` que tiene el `AvatarModifierType.AMT_HIDE_AVATARS` modificador, el avatar del jugador deja de renderizarse. Esto se aplica tanto al jugador en vista en tercera persona como cuando otros jugadores entran en el área.

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```

Esto te permite reemplazar el avatar predeterminado de Decentraland por cualquier avatar personalizado que quieras mostrar en tu scene, consulta [Reemplazar el avatar del jugador](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/npc-avatars/#replace-the-players-avatar). Ten en cuenta que si quieres ver a otros jugadores con avatars personalizados, debes encargarte tú mismo de la sincronización de las posiciones de los jugadores.

### Desactivar la ventana emergente de Passport

Cuando un jugador entra en un `AvatarModifierArea` que tiene el `AvatarModifierType.AMT_DISABLE_PASSPORTS` modificador, al hacer clic sobre ellos ya no se abre la UI de Passport que muestra la bio del jugador, el inventario, etc.

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_DISABLE_PASSPORTS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```

Esto es especialmente útil en juegos donde abrir accidentalmente esta UI podría interrumpir el flujo de una partida, por ejemplo, en un juego de disparos multijugador.

### Ocultar nametags

Cuando un jugador entra en un `AvatarModifierArea` que tiene el `AvatarModifierType.AMT_HIDE_NAMETAGS` modificador, el nametag del jugador se oculta mientras el avatar en sí permanece visible.

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_NAMETAGS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```

Esto es útil para stages, presentaciones o scenes con guion en las que quieras suprimir los nametags de los jugadores sin ocultar los avatars en sí. Por ejemplo, podrías querer una experiencia visual limpia durante una actuación, donde los avatars sean visibles pero los nombres flotantes no distraigan a los espectadores.

`AMT_HIDE_NAMETAGS` también oculta cualquier placa personalizada añadida con el [`AvatarNametag`](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/avatar-nametags/) componente.


> [!NOTE]
> **💡 Consejo**: `AMT_HIDE_AVATARS` ya oculta los nametags junto con el avatar, así que no necesitas añadir `AMT_HIDE_NAMETAGS` al usar `AMT_HIDE_AVATARS`. Usa `AMT_HIDE_NAMETAGS` solo cuando quieras ocultar los nametags mientras mantienes los avatars visibles.


También puedes combinar `AMT_HIDE_NAMETAGS` con otros modifiers como `AMT_DISABLE_PASSPORTS` en la misma área:

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_NAMETAGS, AvatarModifierType.AMT_DISABLE_PASSPORTS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```


> [!NOTE]
> **💡 Consejo**: El nametag solo se oculta mientras la cabeza o el torso del jugador estén dentro del área. Si el área es demasiado baja y el jugador hace doble salto por encima, el nametag reaparecerá brevemente. Haz que el área sea lo bastante alta para cubrir el rango de movimiento esperado.


### Excluir avatars

Puedes excluir una lista de jugadores para que no se vean afectados por una modifier area añadiendo sus player IDs a un array en la `excludeIds` propiedad de la área de modificación.

Este ejemplo oculta todos los avatars en un área, excepto los de jugadores con IDs específicas. Podrías usarlo, por ejemplo, en un evento en vivo, para mostrar solo a los hosts del evento en el escenario y ocultar a cualquier otro jugador que salte al escenario.

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
	excludeIds: ['0xx1...', '0xx2...'],
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```


> [!WARNING]
> **📔 Nota**: Asegúrate de que todos los player IDs estén escritos con letras minúsculas. Usa `.toLowerCase()` si es necesario.


Las modifier areas se ejecutan localmente en la instancia de cada jugador. La lista de IDs excluidos puede ser diferente para cada jugador. En el ejemplo siguiente, cada jugador excluye su propio ID de un modificador que oculta avatars, de modo que cada uno vea su propio avatar y ningún otro.

```ts
import { getPlayer } from '@dcl/sdk/src/players'

export function main() {
	let userData = getPlayer()
	if (!userData) return

	const entity = engine.addEntity()

	AvatarModifierArea.create(entity, {
		area: Vector3.create(16, 5, 16),
		modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
		excludeIds: [userData.userId],
	})

	Transform.create(entity, {
		position: Vector3.create(8, 0, 8),
	})
}
```


> [!CAUTION]
> **❗Advertencia**\
> Si la lista de IDs excluidos va a cambiar periódicamente (por ejemplo, según los jugadores entren o salgan de un área), asegúrate de que la lista se mantenga ordenada. Realiza un `.sort()` sobre el array, para que la lista permanezca en el mismo orden cada vez que se pase. Así, solo se calculan los cambios de la lista. De lo contrario, esto puede tener un impacto significativo en el rendimiento de la scene.
> 
> ```ts
> AvatarModifierArea.create(entity, {
> 	area: Vector3.create(16, 5, 16),
> 	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
> 	excludeIds: myAvatarList.sort(),
> })
> ```
> 


### Áreas superpuestas y cambios en runtime

Cada avatar lleva un indicador de activación/desactivación por efecto, no un conteo de las áreas en las que se encuentra. Cada área escribe ese mismo indicador y la última escritura prevalece. Dos consecuencias suelen sorprender a la gente:

* **Salir de una área vuelve a mostrar el avatar, incluso aunque siga dentro de otra.** Salir caminando de una área pequeña que oculta avatars vuelve a hacer visible el avatar, aunque un área de ocultación que cubre toda la scene siga cubriendo ese punto.
* **Una área cuyas `excludeIds` listas de jugadores escriben "visible" para ellos al entrar**, para cada efecto que esa área enumere. Eso anula un área de ocultación total en la que también se encuentren.

Prefiere una sola área a la vez por avatar. Si las áreas se superponen, asegúrate de que coincidan en cada efecto.

**Eliminar un modificador in situ no lo revierte.** Cuando cambias los fields de un área, el client vuelve a aplicar los modifiers que figuran ahora. Nada restaura un efecto cuyo modificador quitaste de la lista, así que vaciar `modifiers`, o intercambiar `AMT_HIDE_AVATARS` por `AMT_HIDE_NAMETAGS`, deja los avatars ocultos. Añadir un modificador in situ sí funciona.

Para cambiar una área a otro conjunto de modifiers, elimina la entity y crea la reemplazante en el mismo frame:

```ts
import { AvatarModifierArea, AvatarModifierType, engine, Entity, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// Reemplaza el área en lugar de editar sus modifiers in situ
function setModifiers(oldArea: Entity, modifiers: AvatarModifierType[]): Entity {
	engine.removeEntity(oldArea) // esto también vuelve a mostrar todos los avatars dentro de ella

	const area = engine.addEntity()
	Transform.create(area, { position: Vector3.create(8, 1, 8) })
	AvatarModifierArea.create(area, {
		area: Vector3.create(16, 5, 16),
		modifiers,
		excludeIds: [],
	})
	return area
}
```

### Depurar áreas de modificación

Puede ser difícil saber exactamente qué partes de la scene cubren tus áreas de modificación basándote solo en el código. La retroalimentación visual ayuda mucho a confirmar que están bien colocadas.

Para verificar las posiciones de una `AvatarModifierArea` o un `CameraModeArea`, dale a la entity que la contiene un `MeshRenderer` component con una `box` shape, y establece la escala al mismo tamaño que el `area` de la área de modificación.


> [!WARNING]
> **📔 Nota**: Las áreas de modificación no se ven afectadas por la `escala` propiedad del Transform, su tamaño se basa en su `area` propiedad.


```ts
const entity = engine.addEntity()
const areaSize = Vector3.create(8, 3, 8)

AvatarModifierArea.create(entity, {
	area: areaSize,
	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
  	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
	scale: areaSize,
})

MeshRenderer.setBox(entity)
Material.setPbrMaterial(entity, {
	albedoColor: Color4.create(0.5, 0.5, 0.5, 0.5),
})
```

Para activar los efectos del área de modificación, la cabeza o el torso del jugador deben entrar en el área. No surtirá efecto si solo los pies del jugador están en el área. Asegúrate de que el jugador no pueda evadir fácilmente el área saltando.


> [!WARNING]
> **📔 Nota**: El área completa debe caber dentro de los límites de tu scene.



> [!NOTE]
> **💡 Consejo**: Para ver ejemplos en funcionamiento, consulta el [`9,99-modifier-areas`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/9,99-modifier-areas) test scene, que combina `AvatarModifierArea` con runtime-mutated `excludeIds`y [`10,99-avatar-modifier-hide-nametags`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/10,99-avatar-modifier-hide-nametags), que aísla el `AMT_HIDE_NAMETAGS` modificador.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/avatares/modifier-areas.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
