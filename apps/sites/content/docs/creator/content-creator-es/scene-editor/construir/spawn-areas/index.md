> Usa las Áreas de Spawn para establecer visualmente la posición de Spawn y la posición de Spawn Camera.

# Áreas de Spawn

El creador puede establecer la **Posición de aparición** y **Objetivo de cámara de aparición** en Creator Hub editando una **Área de aparición** Entity.

Esto le da al creador una forma visual de establecer el Área de aparición y la dirección hacia la que mirará el avatar cuando aparezca, como se hace con otros objetos en la escena.

Las Áreas de aparición definen dónde aparecen los jugadores cuando acceden directamente a tu escena, ya sea escribiendo las coordenadas o teletransportándose. Tu escena puede tener objetos que bloqueen a los jugadores para moverse si aparecen justo sobre ellos, como árboles o escaleras, o tu escena puede tener un terreno elevado. Sería una mala experiencia para los jugadores si aparecieran sobre algo que no les permitiera moverse. Por eso tienes la opción de establecer una o varias posiciones de aparición en ubicaciones ad hoc.


> [!WARNING]
> **📔 Nota**: Todos los puntos de aparición deben estar dentro de los parcels que componen la escena. No puedes hacer aparecer a un jugador fuera del espacio de estos parcels.


<img src="/docs/_remote/04f3a7c93b45dca9a38de100.webp" alt="" width="200">

El Área de aparición está compuesta por dos entities diferentes que trabajan juntas:

* **Spawn** Entity: se puede encontrar dentro de la **Player** Entity. Define el punto de aparición real del Player.

<img src="/docs/_remote/da6fb5ebe275854d0b596645.webp" alt="" width="200">

* **Objetivo de cámara** Entity: se puede encontrar dentro de la **Área de aparición** Entity. Muévelo hacia la dirección deseada en la que aparecerá el Player.

<img src="/docs/_remote/13b486809fcb8431fd6ecb7c.webp" alt="" width="200">

## Cambiar Áreas de aparición

**Áreas de aparición** y sus **Objetivo de cámara** pueden moverse directamente desde el Scene Editor como cualquier otro objeto en la escena, o modificar sus valores manualmente (como con Transforms en otras Entities).

### Parámetros

Cuando se selecciona un Área de aparición, el creador puede ver y establecer los siguientes valores en la sección de Components:

<img src="/docs/_remote/81d5fae5f7131c97a7c0709a.webp" alt="" width="600">

* **Posición**: Posición de aparición del Avatar. Estas son coordenadas dentro de la escena, relativas al origen de la escena, similares a las que usarías en un componente Transform.
* **Objetivo de cámara de aparición**: La dirección hacia la que mirarán la cámara (y el avatar) una vez aparezcan en la escena. Úsalo para tener un mejor control sobre la primera impresión del jugador cuando entra en tu escena.
* **Área aleatoria**: Añade aleatoriedad al Área de aparición en metros, habilitándolo y deshabilitándolo con el **No aleatorizar** toggle. Cuando está habilitado, se renderiza un área en el Scene Editor, mostrando el área en la que el Player puede aparecer aleatoriamente. El desplazamiento aleatorio se aplica en ambos ejes X y Z, y evita que todos los jugadores aparezcan superpuestos entre sí cuando aparecen, lo cual se ve especialmente mal en escenas concurridas.

<img src="/docs/_remote/87b06caf636a947bd765acb5.webp" alt="" width="600">

## Múltiples Áreas de aparición

<img src="/docs/_remote/80f2cf42b9eb7797e76dccd8.webp" alt="" width="600">

El creador puede tener múltiples **Áreas de aparición** definidas. Para crear un nuevo Área de aparición, sigue estos pasos:

1. Selecciona cualquier Área de aparición, **spawn1** siendo la predeterminada.
2. Todas las Áreas de aparición se muestran en el Panel de la derecha. Puedes añadir nuevas haciendo clic en **+ Añadir nueva Área de aparición**&#x20;

   <img src="/docs/_remote/2d3066e023e44c9ba5f38990.webp" alt="icono" width="200">

   .

<img src="/docs/_remote/24547b03ea234f33b90a044e.webp" alt="" width="600">

3. Habilita el **Spawn principal**&#x20;

   <img src="/docs/_remote/745070068b65895eedbc404a.webp" alt="icono" width="100">

   &#x20;del Área de aparición deseada. Si hay muchas áreas de aparición activadas como **Spawn principal**, el jugador aparecerá aleatoriamente en una de ellas.
4. Previsualiza la escena.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/construir/spawn-areas.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
