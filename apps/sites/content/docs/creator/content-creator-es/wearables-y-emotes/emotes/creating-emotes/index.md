> Consejos y guías para crear Emotes de Decentraland.

# Creando emotes

Esta documentación cubrirá las especificaciones del archivo, lo básico de la animación en Blender, la forma correcta de exportar un Emote y cómo importarlo en el Builder.


> [!NOTE]
> **💡 Consejo**: Instala el [plugin Blender de Decentraland Tools](https://extensions.blender.org/add-ons/decentraland-tools/). Incluye varias funciones útiles para ayudarte a editar y exportar modelos 3D, wearables y emotes.


#### Tabla de especificaciones de animación

| Tasa de fotogramas               | 30 fps                                    |
| -------------------------------- | ----------------------------------------- |
| Longitud máxima                  | 10 segundos (300 fotogramas)              |
| Animaciones por archivo          | 1                                         |
| Formato de exportación           | .glb                                      |
| Tasa de muestreo                 | 1 por defecto (2 o 3 si es necesario)     |
| Tamaño máximo del archivo        | 3 MB                                      |
| Distancia máxima de la animación | 1 metro (frente/atrás, izquierda/derecha) |
| Altura máxima de la animación    | 4 metros                                  |

Puedes encontrar una explicación más detallada de las especificaciones de animación [**abajo**](#the-animation-specifications).

### **Recursos**

Esta documentación explica la configuración de Rig 1.0, sus controles y funciones.

[Rig de Blender de Decentraland](https://github.com/decentraland/docs/tree/main/creator/images/emotes/Avatar_File.blend)


> [!NOTE]
> Si usas Maya, puedes descargar este [Rig de Maya](https://github.com/decentraland/docs/tree/main/creator/images/emotes/DCL_Maya_Rig.ma) y [selector](https://github.com/decentraland/docs/tree/main/creator/images/emotes/emoteAvatar.pkr) proporcionado por [SparkleStudios](https://www.sparkles.studio/) ❤️.


## **Antes de empezar**

### **Tasa de fotogramas**

Antes de comenzar, es importante comprobar la tasa de fotogramas. Las animaciones de Decentraland deben tener una tasa de fotogramas de 30 fps. El archivo del rig viene configurado a 24 fps, así que cámbialo a 30 antes de animar (una tasa de fotogramas incorrecta afectará la velocidad de la animación). Esa opción se puede encontrar en Output Properties (el icono de la impresora) debajo de Format, como se muestra a continuación:

![Asegúrate de que la tasa de fotogramas esté configurada en 30 fps antes de empezar.](/docs/_remote/53650a16525c3ac92e8eef4c.webp)

Asegúrate de que la tasa de fotogramas esté configurada en 30 fps antes de empezar.

### **Pose Mode**

En Blender, un rig se puede ver en tres modos diferentes: Object Mode, Edit Mode y Pose Mode. Las animaciones solo se pueden hacer en Pose Mode (en ese modo, los controles tienen colores). Con el rig seleccionado, encontrarás esa opción en un menú desplegable, en la parte superior derecha.

![Cambiando a Pose Mode.](/docs/_remote/2d50e8d239676da8c48b0736.webp)

Cambiando a Pose Mode.

### **Interfaz para animaciones**

En el archivo del rig, además de las dos ventanas del viewport (vista frontal y lateral), hay tres más en la parte inferior: un ***Graph Editor***, ***un Dope Sheet***, y una ***Timeline***.

* ***Graph Editor***: En este editor, es posible editar las curvas de animación de cada propiedad de transform de los controles seleccionados. Esas curvas muestran cómo se está calculando la interpolación y se pueden editar para lograr el efecto deseado en la animación. Tanto aquí como en el dope sheet, la ***Only Show Selected*** está activada, lo que significa que solo incluirá los canales relacionados con el control seleccionado. Esto se puede activar y desactivar simplemente haciendo clic en el icono de flecha.
* ***Dope Sheet***: Aquí puedes editar los keyframes. Este también es el lugar donde puedes crear nuevas animaciones o revisar las múltiples creadas. Ten en cuenta que, para tener acceso a la animación, el ***Action Editor*** debe estar seleccionado. Esta opción está justo al lado del *Dope Sheet* icono, en un menú desplegable.
* ***Timeline***: Aquí es donde se encuentran la línea de tiempo y los controles de reproducción. Aquí, el ***Auto Keying*** está activado, lo que significa que cada vez que se manipula un control se crea automáticamente un keyframe. Siempre puedes desactivar esa función haciendo clic en el punto junto a los controles de reproducción.

¡Con este espacio de trabajo tienes todo lo necesario para empezar a animar!

![](/docs/_remote/4dfc916aa7bf8bcc00b1659f.webp)

Estas son las ventanas inferiores. La superior está en el ***Graph Editor,*** la del medio en el ***Dope Sheet,*** y la inferior es la ***Timeline.*** La flecha roja superior muestra el ***Only Show Selected*** herramienta y la inferior muestra el ***Auto Keying***.


> [!NOTE]
> **💡 ¡Consejo!**
> 
> Como Blender es altamente personalizable, este también es un buen momento para configurar el diseño que mejor se adapte a ti, añadiendo, ajustando o eliminando ventanas. Cada animador tiene sus propias preferencias, ¡así que siéntete libre de editar el diseño como quieras!


## Primeros pasos

#### **Pose inicial**

En el archivo del rig proporcionado, ya hay una acción, la ***Starting\_Pose***. Teniendo en cuenta que todas las acciones del avatar parten de la pose idle, **te recomendamos encarecidamente empezar tu animación desde esa pose y usarla también en el último fotograma**. Esto hará que la transición de Idle a Emote sea mejor y la animación más fluida.


> [!NOTE]
> **💡 ¡Consejo!**
> 
> Si quieres hacer una animación en bucle, no tienes que empezar la animación desde la Pose inicial. ¡Siéntete libre de usar la pose que tenga más sentido para tu animación!\*\*


Para que un bucle se ejecute con fluidez, el primer y el último keyframe deben ser idénticos. Por ejemplo, un bucle de 4 segundos a 30 fps necesita la misma pose en el fotograma 1 y en el fotograma 121. Si difieren en algo, los jugadores verán un salto visible cada vez que se repita el clip.

#### Área de animación <a href="#ground-reference-and-animation-area" id="ground-reference-and-animation-area"></a>

En el archivo del avatar proporcionado, encontrarás una colección llamada ***Animation\_Area\_Reference***. Consta de tres objetos que ayudarán visualmente a establecer los límites de la animación: ***Animation\_Area\_Reference***, ***Ground\_Reference*** y ***Root\_Animation\_Area***.

* Animation\_Area\_Reference es un cubo de 4x4, que es la caja delimitadora. La malla del avatar y los props deben permanecer dentro de esta área durante toda la animación. Los props pueden moverse libremente siempre que permanezcan dentro del área del cubo.
* Ground\_Reference es un plano proporcionado para ayudar a comprobar si hay penetración en el suelo durante la animación. También contiene información importante: el círculo más pequeño, que tiene un diámetro de 2 metros, define el área donde se puede mover el root del avatar. Eso significa que, mientras esté en el suelo, el avatar solo puede moverse 1 metro en cada dirección: frente, atrás, izquierda y derecha.
* Root\_Animation\_Area es un cilindro que define el área donde puedes mover el root/centro de gravedad del avatar (CTRL\_Avatar\_UpperBody o Avatar\_Hips). El cilindro tiene 4 metros de altura, lo que significa que el root puede subir hasta arriba siempre que ninguna malla quede fuera de la caja delimitadora. Las piernas, los brazos y otras partes del cuerpo del avatar pueden estar fuera del cilindro, siempre que nunca estén fuera de Animation\_Area\_Reference.

![Referencia del área de animación.](/docs/_remote/8ef56331d4f29f23223dc576.webp)

En resumen, el avatar puede moverse a la derecha, a la izquierda, hacia adelante y hacia atrás mientras el root permanezca dentro del cilindro, lo que significa 1 metro en cualquiera de las cuatro direcciones.

![Movimiento del avatar.](/docs/_remote/7949b480b39476fbab88c60f.webp)

En cuanto a la altura, puede subir hasta arriba (máximo 4 metros) siempre que el avatar permanezca dentro de la caja delimitadora. A continuación se muestran algunos casos de altura del avatar bien hechos (todo el cuerpo dentro de la caja delimitadora) y algunos mal hechos (malla fuera de la caja delimitadora).

| ![](/docs/_remote/cbee2350215541a459589276.webp) | ![](/docs/_remote/4774d078d1fff2f442dda8f4.webp) |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

| ![](/docs/_remote/d51139e66aeafc6d74965579.webp) | ![](/docs/_remote/93bbdf9a30346096af762dea.webp) |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

![](/docs/_remote/da31c95fa83a05f64398f245.webp)

Aquí hay algunos ejemplos de emotes que están dentro de los límites del área de animación.

| ![](/docs/_remote/989fc61d99ce5203d6197a0e.webp) | ![](/docs/_remote/aa48c499a9817ab4725cfe89.webp) |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |


> [!NOTE]
> **💡 ¡Atención!**
> 
> Ten cuidado con estos límites, porque cruzarlos podría causar problemas de jugabilidad.


## **Creación de una animación**

El archivo blend tiene un clip de animación listo para editar: *StartingPose\_Avatar*. Puedes duplicar y renombrar ese clip de animación como mejor te parezca. ¡No hace falta crear uno desde cero!

En la *Browse Action* sección, simplemente haz clic en el ***Create A New Action*** botón para duplicar la animación actual. Para renombrar el clip, solo haz clic en el texto y escribe otra cosa.

Blender 4.4 introdujo *Slotted Actions*, el icono a la derecha de la *Browse Action* sección de versiones anteriores. No hace falta tocar eso si estás creando un emote sin prop, así que puedes dejarlo tal como está. Si estás animando el avatar, asegúrate de que la slotted action sea Avatar\_Animation.

![](/docs/_remote/75912c834d5cbe632654a9f0.webp)

Crea una nueva animación duplicando la existente o haciendo clic en ***Unlink Action*** y luego ***New***.

### **Exploración y eliminación de animaciones**

En Blender, puedes tener varias pistas de animación en el mismo archivo. Es posible explorarlas haciendo clic en el menú desplegable Browse Action. Todas las animaciones con una F (Fake User) se guardarán. Para eliminar una animación, pulsa Mayús en el teclado y haz clic en la X. Después de hacer eso, la animación mostrará un 0 junto a ella, lo que significa que se eliminará la próxima vez que cierres Blender o vuelvas a abrir el archivo.

![](/docs/_remote/cf72bdeab774263920c04bb9.webp)

Explorando animaciones: las que tienen una F se guardarán, y las que tienen un 0 se eliminarán.

Otra forma de eliminar animaciones sin tener que recargar Blender es cambiando el Display Mode de View Layer a Blender File. Expande Actions y elimina cualquier animación no deseada haciendo clic derecho sobre ella y seleccionando Delete.

![](/docs/_remote/3e2be89fb22f3ee63cc39ef1.webp)

Puedes eliminar animaciones directamente desde Blender File en Display Mode, en el outliner.


> [!NOTE]
> **💡 ¡Consejo!**
> 
> No edites siempre la misma pista de animación. Antes de hacer cambios importantes, simplemente duplica la animación. Así tendrás una versión de respaldo por si te arrepientes de borrar o cambiar algo. ¡También es una buena forma de seguir el progreso realizado hasta ahora!


![](/docs/_remote/75912c834d5cbe632654a9f0.webp)

Duplicando clips de animación.

### **Nomenclatura**

**El nombre de una animación debe empezar con una letra mayúscula y, si el nombre tiene más de una palabra, las palabras deben separarse con \_.** No uses espacios ni caracteres especiales. Aquí tienes algunos ejemplos de nomenclatura:

* Snowfall
* Rainbow\_Dance
* Throw\_Money
* Talk\_To\_Hand

### **Anulaciones de Emote**

Las anulaciones de Emote ocurren cuando los deform bones no tienen un keyframe establecido en uno de los parámetros. Sin un keyframe, ese hueso no tendrá la información de dónde debe estar, cuánto se ha rotado y escalado, dejando ese canal abierto. La consecuencia es que si reproduces un emote en World y luego activas el tuyo mientras el anterior todavía se estaba reproduciendo, la información de ubicación, rotación y escala será anulada por el emote anterior, lo que provocará una combinación de ambos. A menos que esto se haga a propósito, afectará a tu animación; a veces con un resultado divertido, pero otras dejará el emote completamente estropeado. A continuación hay un ejemplo de una anulación de emote.

![](/docs/_remote/4d3cf50ba093eedb38f43aee.webp)

Para evitar eso, selecciona todas las capas que tengan bones (que se pueden encontrar en ***Object Data Properties*** > ***Skeleton*** > ***Layers***). Luego, en ***Pose Mode***, deja el cursor de la línea de tiempo en el primer fotograma de tu animación y, con el mouse en ***Viewport Display***, presiona ***A*** para seleccionar todo. En el ***Graph Editor***, haz doble clic en el ***Eye*** junto al canal del armature para hacer visibles todos los canales. Con todos los bones seleccionados, pulsa ***I*** para establecer un keyframe. Haz lo mismo para el último fotograma.

**Asegúrate de seleccionar los deform bones, ¡esto es especialmente importante!** Los deform bones se pueden encontrar en la última capa inferior y se muestran como bones verdes en el ***Viewport***.

![](/docs/_remote/359c9a9a72aea9590225e2c7.webp) Establecer keyframes en todos los bones en el primer y el último fotograma evita las anulaciones de emote.

## **Las especificaciones de animación**

### **La longitud de la animación**

La longitud máxima de una animación es **10 segundos** o **300 fotogramas**. Recuerda poner keyframe a las propiedades de cada control en el primer y el último fotograma.


> [!WARNING]
> ⚠️ Los canales con la visibilidad desactivada en el Graph Editor no se pondrán en keyframe, no se eliminarán ni siquiera se mostrarán en el Action Editor. A menos que se haya hecho intencionalmente, presta especial atención a la visibilidad.


![](/docs/_remote/91ee3dde0d4a300d0eb09a66.webp)

¡Haz visibles los canales antes de poner keyframes!

### **Número de animaciones**

Si se trata de un emote estándar (sin prop), el archivo exportado solo puede tener una animación. Para los emotes 2.0 puedes tener un clip para el avatar y un clip para el prop. Si se duplicaron animaciones durante el proceso, asegúrate de eliminarlas todas antes de exportar. Conserva solo la versión final. Los emotes en secuencia que necesitan muchas animaciones para funcionar (acción de inicio, acción de bucle y acción de fin) no están soportados por ahora.

### **Formato**

Las animaciones deben exportarse como .**GLB**. El archivo solo puede contener el esqueleto deformador y la animación. **La malla, los controles y cualquier otro objeto no deben exportarse**. Puedes encontrar más detalles sobre cómo exportar [**abajo**](#exporting).

### **Muestreo**

Como las constraints no se pueden exportar, la única forma de exportar el clip de animación es haciéndole baking, lo que significa que las posiciones, rotación y escala de todos los deform bones se pondrán en keyframe en cada fotograma de la animación. Si el clip es demasiado largo, como hasta 300 fotogramas, tendrá 300 keyframes después de exportarlo y cuantos más keyframes tenga, más pesado será el archivo.

El muestreo es una buena forma de optimizar la animación. La tasa de muestreo definirá con qué frecuencia se horneará un keyframe en la animación. Por ejemplo, si la tasa de muestreo se configura en 2, eso significa que se creará un keyframe cada dos fotogramas. Una tasa de muestreo de 3 horneará un keyframe cada tres fotogramas y así sucesivamente. Cuanto más alta sea la tasa de muestreo, más ligero será el archivo.

Sin embargo, la desventaja es que la animación empezará a ser cada vez menos fluida, ya que pierde algunos keyframes importantes (se distribuyen a lo largo de la animación de forma desigual). También es importante tener en cuenta que **el muestreo NO divide el número de fotogramas de la animación por la tasa de muestreo**.

Normalmente, una **tasa de muestreo de 2 o 3** hará el trabajo. Esos números pueden optimizar la animación sin comprometer la calidad.


> [!NOTE]
> **💡 ¡Consejo!**
> 
> Si el número de fotogramas de la animación puede dividirse por la tasa de muestreo, ¡eso es bueno! Significa que el fotograma final se horneará, preservando la transición del final al inicio de la animación.


### **Tamaño del archivo**

El tamaño máximo del archivo es **3 MB**. Si el archivo supera ese tamaño después de exportarlo, intenta comprobar si la malla no se exportó por accidente o si la animación no supera los 10 segundos. Si sigue superando los 3 MB, intenta experimentar con la tasa de muestreo, ya que los valores más altos mejorarán la optimización.

Si el emote contiene algún modelo 3D adicional, las texturas de esos modelos no pueden superar un tamaño de 1024 píxeles.

## **Exportando**

Como solo queremos que se exporten el armature y la animación, desactiva la visibilidad de la malla y de cualquier objeto que no sea el armature antes de exportar, como se muestra a continuación:

![](/docs/_remote/3b011135dba6d9de9e62a101.webp)

¡Desactiva la visibilidad de la malla antes de exportar!

Tu archivo también debe contener solo la única acción que estás exportando. El rig trae una `Starting_Pose` acción, así que [elimínala](#browsing-and-deleting-animations) junto con cualquier otra acción sobrante, de lo contrario el archivo exportado `.glb` contendrá dos clips en lugar de la [una animación por archivo](#number-of-animations) que Decentraland espera. Guarda una copia editable de tu `.blend` antes de eliminar cualquier cosa.

Para exportar, ve a *File* > *Export* > *glTF2.0 (.glb, .gltf)*

![](/docs/_remote/3142125b130d9e9715dddd3d.webp)

Para la configuración de exportación, expande Include y en Limit to activa Visible Objects. Luego, expande la pestaña Data, expande Armature y activa Export Deformation Bones Only.

| ![](/docs/_remote/446d202239ea82992946aa43.webp) | ![](/docs/_remote/eef0447fdcd598e5153a2669.webp) |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

Si necesitas muestrear la animación, expande la pestaña Animation, expande Sampling Animations y elige el número de muestras deseado.

| ![](/docs/_remote/b5ee46c688eb78bff045e6a1.webp) | ![](/docs/_remote/6813e1cd152caee20fe515fb.webp) |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

¡Eso es todo para exportar la animación!

## Referencias

Si todavía no estás seguro de por dónde empezar o necesitas alguna referencia o inspiración, aquí tienes algunos clips de animación que pueden ayudarte con eso. ¡Pueden ser un buen material de estudio!

[Idle.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/idle.glb)

[Jump.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/jump.glb)

[Walk.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/walk.glb)

[Run.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/run.glb)

[Pose\_Jump.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/pose_jump.glb)

[Pose\_Spin.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/pose_spin.glb)

[Spotlight.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/spotlight.glb)

[Fashionista.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/fashionista.glb)

[Chic.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/chic.glb)

[Flag\_Emote.glb](https://github.com/decentraland/docs/tree/main/creator/images/emotes/Flag_Emote.glb)

[Flag\_Emote.blend](https://github.com/decentraland/docs/tree/main/creator/images/emotes/Flag_Emote_Final.blend)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/wearables-y-emotes/emotes/creating-emotes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
