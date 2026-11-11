> Usa modifier areas en el Scene Editor para cambiar el comportamiento del avatar o forzar un modo de cámara dentro de una región de tu escena.

# Áreas modificadoras

Las áreas modificadoras son regiones invisibles que cambian cómo los jugadores experimentan una parte de tu escena. Hay dos tipos:

* **Avatar Modifier Area**: Cambia cómo se comportan o se ven los avatares para los jugadores dentro de la región, por ejemplo ocultando avatares o desactivando las ventanas emergentes del passport.
* **Camera Modifier Area**: Fuerza la cámara del jugador a primera persona o tercera persona mientras está dentro de la región.

Ambos tipos vuelven al comportamiento normal cuando el jugador sale.

## Añadir un área modificadora

Puedes añadir un área modificadora de dos maneras:

**Desde el catálogo de assets:** Abre el catálogo de assets y busca **Avatar Modifier Area** o **Camera Modifier Area** en la **Utils** categoría. Arrástralo a la escena. El elemento aparece como un cubo de marcador de posición translúcido que solo es visible en el editor, no en la escena en ejecución.

**Desde el menú de componentes:** Selecciona cualquier entidad existente, haz clic en el **+** botón en la parte superior del panel de propiedades y elige **Avatar Modifier Area** o **Camera Modifier Area** de la lista. Consulta [Agregar Components](/docs/creator/content-creator-es/scene-editor/construir/components/#add-components) para más detalles.

![El menú de añadir componente abierto en una entidad llamada Stage area, mostrando los componentes de los grupos Contenido 3D e Interacción, incluidos Avatar Modifier Area y Camera Modifier Area.](/docs/_remote/e0e52c0c060b63e97e2466dd.webp)

## Dimensionar el área

El tamaño de un área modificadora está controlado por la **escala de Transform**. Si escalas la entidad a 4 x 3 x 4, la región modificadora se convierte en una caja de 4 x 3 x 4 metros centrada en la posición de la entidad. Usa el gizmo de escala o escribe valores exactos en el componente Transform para establecer el tamaño que necesitas.


> [!WARNING]
> **Nota:** No hay un campo separado "Área" en el panel del editor. El editor mantiene la propiedad interna de área sincronizada automáticamente con la escala de la entidad, así que todo lo que necesitas hacer es redimensionar la entidad.


## Avatar Modifier Area

El panel de Avatar Modifier Area tiene dos ajustes:

![El panel del componente Avatar Modifier Area, con un menú desplegable de Modifiers que muestra Ocultar avatares seleccionado, una sección Exclude Player IDs y un botón Add Player ID.](/docs/_remote/d49ca42bf27a1995edcc536d.webp)

### Modificadores

Un menú desplegable de selección múltiple con las siguientes opciones:

* **Ocultar avatares**: Los avatares dentro del área se vuelven invisibles. Los jugadores dentro no pueden verse entre sí, pero los jugadores fuera del área todavía los ven con normalidad.
* **Desactivar passports**: Hacer clic en el avatar de un jugador dentro del área ya no abre su ventana emergente de perfil.

Puedes seleccionar uno o ambos modificadores en la misma área. Cuando agregas el componente, **Ocultar avatares** está seleccionado de forma predeterminada. Haz clic en el **x** en un modificador para quitarlo.

### Excluir IDs de jugador

Una lista de direcciones de wallet que no se ven afectadas por los modificadores. Los jugadores cuya dirección esté en esta lista se comportan con normalidad incluso dentro del área.

Haz clic **Agregar ID de jugador** para agregar una entrada y luego pega la dirección de wallet. Para eliminar una, haz clic en el menú de tres puntos que está junto a ella y selecciona **Eliminar ID de jugador**.

Esto es útil para eventos en los que quieres ocultar los avatares del público en un escenario pero mantener visibles a los artistas.


> [!WARNING]
> **Nota:** Asegúrate de que las direcciones de wallet estén en minúsculas. El área modificadora usa una comparación exacta de cadenas.


### Casos de uso

* **Zonas de privacidad**: Oculta los avatares en un backstage o vestuario.
* **Escenarios limpios**: Oculta los avatares del público en un escenario para que solo los artistas (enumerados en la lista de IDs excluidos) sean visibles.
* **Arenas de juego**: Desactiva los passports en un área de combate para que los jugadores no abran perfiles accidentalmente durante el juego.

## Camera Modifier Area

El panel de Camera Modifier Area tiene un ajuste:

![El panel del componente Camera Modifier Area, con un menú desplegable de Modo de cámara configurado en Primera persona.](/docs/_remote/199bb09636ac6f76d722633e.webp)

### Modo de cámara

Un menú desplegable con dos opciones:

* **Primera persona**: Fuerza al jugador a la vista en primera persona mientras está dentro del área.
* **Tercera persona**: Fuerza al jugador a la vista en tercera persona mientras está dentro del área.

El valor predeterminado es **Primera persona**. Cuando un jugador sale del área, se restaura su modo de cámara anterior.


> [!NOTE]
> **Consejo:** El modo de cámara cinematográfica no está disponible aquí. Para controlar la cámara más allá del cambio entre primera y tercera persona, usa un [Virtual Camera](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/camera/#using-virtual-cameras) con código.


### Casos de uso

* **Pasillos estrechos o salas de acertijos**: Fuerza la primera persona para hacer clic con precisión en objetos pequeños.
* **Miradores panorámicos**: Fuerza la tercera persona para que los jugadores puedan ver su avatar contra el fondo.
* **Zonas de tutorial**: Asegura que todos los jugadores usen el mismo modo de cámara durante una experiencia guiada.

## Combinación de áreas modificadoras

Puedes colocar ambos tipos de área modificadora en la misma entidad o superponer áreas modificadoras separadas en el mismo espacio. Cada área aplica sus efectos de forma independiente.


> [!WARNING]
> **Nota:** Mantén bajo el número de áreas modificadoras en tu escena. Demasiadas pueden afectar al rendimiento. Unas pocas están bien para la mayoría de las escenas.


## Usar áreas modificadoras con código

Los componentes de Scene Editor corresponden directamente a los componentes de SDK7 `AvatarModifierArea` y `CameraModeArea` componentes. Para obtener todos los detalles sobre los modificadores disponibles, el comportamiento en runtime y opciones avanzadas como cambiar dinámicamente la lista de exclusión, consulta:

* [Áreas modificadoras de Avatar](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/modifier-areas/) (SDK7)
* [Áreas de modo de cámara](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/camera/#1st-and-3rd-person-camera-modes) (SDK7)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/interactividad/modifier-areas.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
