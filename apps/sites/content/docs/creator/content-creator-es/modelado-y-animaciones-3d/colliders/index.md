# Colliders

Aprende cómo añadir colliders a los modelos 3D importados a Decentraland.

Para habilitar collisions entre un modelo 3D y los usuarios de tu escena, debes darle colliders al modelo. Sin un collider, los players pueden caminar a través de los modelos como si no estuvieran ahí. Los colliders también son necesarios para que un objeto responda a los pointer events del player o los bloquee, o para que sea intersectado por raycasts o los bloquee. Por razones de rendimiento, los colliders suelen modelarse por separado como una geometría invisible más simple que tiene menos vertices que la geometría visible.


> [!NOTE]
> **💡 Consejo**: Instala el [plugin Decentraland Tools Blender](https://extensions.blender.org/add-ons/decentraland-tools/). Incluye varias functions útiles para ayudarte a editar y exportar modelos 3D, wearables y emotes.


El modelo puede usar una geometría invisible como colliders, o (a partir de SDK7) puedes asignar la geometría visible para que se comporte como un collider. Ver [**Colliders en modelos 3D**](/docs/creator/sdk7/3d-essentials/colliders/#colliders-on-3d-models) . También puedes asignar diferentes layers de colisiones a las distintas geometries, para controlar por separado la física del player, los pointer events y otras layers personalizadas, consulta [**Colliders en modelos 3D**](/docs/creator/sdk7/3d-essentials/colliders/#collision-layers) .

Los colliders no afectan cómo los models y las Entities interactúan entre sí; siempre pueden solaparse. Los colliders solo afectan cómo el model interactúa con el avatar del player.

Para que una scene de Decentraland reconozca un objeto como un collider, solo necesita tener un nombre de cierta manera. El nombre del objeto debe incluir el sufijo “\_collider” al final.

Por ejemplo, para crear un collider para un poste de luz puedes crear un objeto de caja simple que rodee su tronco. Los players en la scene no verán esta caja, pero bloqueará su camino.

![](/docs/_remote/872cedc3afd092432696701c.webp)

En este caso, podemos llamar a la caja “LampPost*collider” y exportar tanto el árbol como la caja como un único modelo \_\_gltf. La etiqueta \*\_\_collider*\* avisa al World Engine de Decentraland que el objeto caja pertenece a la colección de colliders, haciendo invisible el mesh del collider.

Cada vez que un player vea el modelo del poste de luz en tu scene, verá el modelo complejo de tu lámpara. Sin embargo, cuando camine dentro de él, colisionará con la caja, no con el árbol.

## Restricciones espaciales del player

Cuando diseñes models que están pensados para que los players caminen alrededor de ellos, debes tener en cuenta las siguientes medidas de referencia:

* Los players pueden subir un escalón de hasta unos 40 centímetros sin saltar. Un escalón de 45 centímetros o más requiere un salto. Para unas escaleras que resulten cómodas, mantén cada escalón a 25 centímetros de alto o menos, y con al menos 30 centímetros de profundidad.
* Los players pueden subir pendientes de hasta 46 grados sin saltar. En superficies más pronunciadas, el avatar se resbala. Para caminar fácilmente, mantén las rampas a 30 grados o menos. Un collider de rampa sobre escaleras es la excepción: sigue la pendiente de las escaleras, unos 40 grados para escalones de 25 centímetros de alto y 30 centímetros de profundidad (ver [Agregar un collider a una escalera](#add-a-collider-to-a-staircase)).
* Los players pueden saltar hasta 1,8 metros de altura
* Los avatars miden alrededor de 1,9 metros de altura, pero considera que pueden llevar hats u otros wearables en la cabeza.
* El collider del avatar es una cápsula de 0,6 metros de ancho (un radio de 0,3 metros). Una viga o techo de menos de unos 1,85 metros detiene al avatar. Deja al menos 2 metros de altura libre en cualquier lugar por donde caminen los players. En interiores, deja mucho más espacio para la cámara, ver [Construye interiores más grandes que en la vida real](/docs/creator/content-creator-es/scenes-sdk7/disenar-la-experiencia/design-games/#build-interiors-larger-than-real-life).

Cada vez que un player vea el modelo del árbol en tu scene, verá el modelo complejo de tu árbol. Sin embargo, cuando camine dentro de tu árbol, colisionará con la caja, no con el árbol.

### Agregar un Collider a una escalera

Las escaleras son un caso de uso muy común para los objetos collider. Para que los players puedan subir escaleras, debe existir un objeto \_collider correspondiente sobre el que los players puedan pisar.

Recomendamos usar un objeto rampa para tus colliders de escaleras; esto ofrece una experiencia mucho mejor al subir o bajar. Cuando suban tus escaleras, parecerá una ascensión o descenso suave, en lugar de obligarlos a “saltar” cada escalón individual.

Usar un objeto rampa también evita crear geometría innecesaria, dejando espacio para otros models más complicados. Ten en cuenta que la geometría del collider también se toma en cuenta al calcular la [**limitaciones de la escena**](/docs/creator/sdk7/optimizing/scene-limitations/)

1. Crea un nuevo objeto con forma de rampa que se parezca al tamaño y las proporciones de las escaleras originales.

![](/docs/_remote/8162fc04d84389328f79ddb3.webp)

2. Nombra el objeto de la rampa algo similar a S*tair\_collider*. Debe terminar en \_*collider*.
3. Superpone el objeto rampa sobre las escaleras para que ocupen el mismo espacio.

![](/docs/_remote/fab0dd4c3439215495a72aa2.webp)

4. Exporta ambos objetos juntos como un único *glTF* model.

Ahora, cuando los players vean las escaleras en tu scene, verán el modelo más elaborado de las escaleras, pero cuando las suban, colisionarán con la rampa.

## Optimizando colliders

* Prefiere usar **Box**, **Sphere** y **Capsule** collision meshes, porque tendrán mejor rendimiento al cargar y en tiempo de ejecución.
* Considera aproximar una collision mesh compleja con varias Primitive collision meshes (**Box**/**Sphere**/**Capsule**)

![](/docs/_remote/195d7d652ad646f33e815b49.webp)

*Ejemplo: El collider para Ring puede componerse de varios colliders Capsule o Box*

* Coloca la collision mesh dentro de la Hierarchy del objeto al que está relacionada. Por ejemplo, Spaceship\_collider puede ser un hijo dentro del padre Spaceship (desde el punto de vista de la Hierarchy)

### Colliders convexos vs cóncavos

Si usar un collider de mesh arbitraria es la única posibilidad, entonces haz la collision mesh Convex (no Concave). Esto evitará situaciones físicas extrañas, como tartamudeo, quedar atascado dentro del collider o atravesarlo. Ten en cuenta que a menudo es posible imitar una mesh cóncava con varias meshes convexas.

![](/docs/_remote/f40bda0817152df8a3cd5005.webp)

## Nomenclatura de colisiones

**Usa nombres significativos. El nombre debe dar contexto de dónde se usa el asset o a qué parte del objeto se relaciona.**

Para la nomenclatura de assets usamos una mezcla de `PascalCase` y `snake_case`, a la que podemos llamar `PascalSnake_case`. Las reglas básicas ahí: una palabra nueva o una palabra después de un separador (`_`) comienza con **letra mayúscula.**

Collision Mesh es la mesh que se usa como referencia para crear un collider en el Explorer

* Sigue los mismos principios que las Meshes anteriores, pero tiene `_collider` sufijo al final **Ejemplos:** 🟢 **Prefiere** nombres - `Tram_Wheel_LeftFront_collider` 🔴 **Evita** nombres - `Primitive.001_Collider`, `collider (1)`, `Cylinder.007_collider`,


> [!WARNING]
> **🔥Consejo de optimización🔥**
> 
> Si un collider puede aproximarse con una forma Box, Sphere o Capsule, entonces usa un sufijo adicional con una de estas 3 posibles variaciones \_box, \_sphere o \_capsule. Usar estas 3 formas en lugar de una mesh arbitraria para la detección de colisiones aumentará el rendimiento de tu scene y el tiempo de carga 💪🚀
> 
> Ejemplos: 🟢 Teater\_MainWall\_North\_collider\_box, Fountain\_Statue\_collider\_sphere


## Mejores prácticas para colliders

* **Usa siempre el menor número posible de triangles al crear colliders.** Evita hacer una copia de un objeto complejo para usarla como collider. Los colliders simples garantizan una buena experiencia de usuario y mantienen tu scene dentro de las limitaciones de triangles.
* **Los objetos collider no deberían tener ningún material**, ya que los players de tu scene nunca lo verán. Los colliders son invisibles para los players.
* **Todos los nombres de objetos collider deben terminar en \_*****collider*****.** Por ejemplo, T*ree\_collider*.
* **Si usas un \_plane**\_\*\* como collider, solo bloqueará en una dirección.\*\* Si quieres que los colliders bloqueen desde ambos lados, por ejemplo para una pared, necesitas crear dos planes con sus normales orientadas en direcciones opuestas.
* Al duplicar objetos collider, presta atención a sus nombres. Algunos programas añaden un \_*1* al final del nombre del archivo para evitar duplicados, por ejemplo *tree\_collider\_1*. Los objetos que se nombren así serán interpretados por el World Engine de Decentraland como objetos normales, no como colliders.
* Puedes evitar agregar una mesh collider si añades un `MeshCollider` componente invisible a la Entity, que aproxima la forma del modelo 3D.
* También puedes evitar agregar una mesh collider si configuras el `GLTFContainer` component para usar la geometría visible como un collider. Ver [**Colliders en modelos 3D**](/docs/creator/sdk7/3d-essentials/colliders/#colliders-on-3d-models) .
* Si estás creando un modelo 3D con muchas formas de mesh que cada una necesita su collider, hay algunos addons útiles que pueden ayudar. Para Blender puedes usar la opción de renombrado por lotes dentro de Blender \*\*\*\*para agregar automáticamente un `_collider` sufijo a todos los nombres de mesh de un grupo. Más sobre esta herramienta en la sección “**sección “Usar Blender para renombrar assets”**


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/modelado-y-animaciones-3d/colliders.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
