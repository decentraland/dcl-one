> Límites de escenas móviles y objetivos de rendimiento para escenas de Decentraland, y cómo previsualizarlos y medirlos desde Creator Hub.

# Rendimiento

Los dispositivos móviles tienen restricciones de recursos más estrictas que el escritorio o la web. Entender los límites de la escena móvil y cómo previsualizarlos te ayuda a publicar una escena que funcione sin problemas para la audiencia más amplia posible.

## Límites de la escena móvil

La tabla siguiente muestra los límites aplicados al ejecutarse en dispositivos móviles. Alcanzar un **límite suave** activa una advertencia en el panel de rendimiento; alcanzar un **límite duro** impide que la escena se cargue.

| Métrica                              | Límite suave | Límite duro |
| ------------------------------------ | ------------ | ----------- |
| Triángulos                           | 1,000,000    | 1,200,000   |
| Entities                             | 4,800        | 6,000       |
| Meshes (cuerpos)                     | 2,400        | 3,000       |
| Geometrías                           | 1,000        | 2,000       |
| Materials                            | 400          | 500         |
| Textures                             | 400          | 500         |
| Colliders                            | 1,200        | 1,500       |
| emisores VFX                         | 8            | 10          |
| partículas activas                   | 25,000       | 50,000      |
| fuentes de luz                       | 8            | 16          |
| luces de sombra                      | 2            | 4           |
| Tamaño del contenido                 | 120 MB       | 150 MB      |
| Contenido externo                    | 40 MB        | 50 MB       |
| Memoria (RSS del proceso)            | 1,638 MB     | 2,048 MB    |
| Llamadas de dibujo                   | 1,000        | 2,000       |
| Rendimiento (cuanto más alto, mejor) | 90%          | 85%         |

## Luces y partículas

Las filas de luces contabilizan **el contenido de la escena creado** , no lo que finalmente ejecuta el Engine. Cada perfil gráfico activa solo un presupuesto de las fuentes de luz más cercanas (0 en Very Low hasta 8 en High) y puede desactivar las sombras por completo, así que una escena que crea 20 luces sigue pagando el costo de la escena aunque como máximo 8 emitan luz a la vez. Crea hasta el límite; no dependas del presupuesto en tiempo de ejecución.

Las partículas además tienen un límite en tiempo de ejecución: 5,000 partículas por emisor y 50,000 por escena. Cualquier cosa creada por encima de esos límites se ajusta silenciosamente, así que superarlos solo desperdicia presupuesto de la escena.

## Previsualizar el rendimiento móvil

Puedes comprobar el rendimiento móvil de tu escena directamente desde **Creator Hub** sin desplegarlo en un dispositivo físico.

1. Abre tu escena en **Creator Hub**.
2. Selecciona Preview > Mostrar código QR para móvil
3. Abre la **Previsualización de límites de la escena** desde el icono de la esquina superior derecha de tu teléfono (monitor con estadísticas en rojo).

![Panel de límites de la escena móvil en Creator Hub](/docs/_remote/86e4b4fb53b120e0cdcd5a75.webp)


> [!WARNING]
> **Mantén el rendimiento por encima del 90 % en gráficos altos**
> 
> El **Rendimiento** valor es un porcentaje del presupuesto de FPS establecido por el perfil gráfico activo. Una puntuación de **100%** significa que la escena está alcanzando el objetivo completo de FPS.
> 
> Prueba siempre en un dispositivo de gama media como el **Samsung Galaxy A54** y apunta a una **puntuación de Rendimiento superior al 90 % en el perfil gráfico High**. Esto garantiza una experiencia fluida para la mayoría de los jugadores móviles. Consulta [Requisitos de hardware](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/hardware-requirements/) para dispositivos de referencia.
> 
> Puedes cambiar el perfil gráfico desde **Menú del juego → Configuración → Gráficos → desactiva Gráficos dinámicos → cambia de perfil**.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/desarrollar-para-movil/desarrollar/optimize-performance.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
