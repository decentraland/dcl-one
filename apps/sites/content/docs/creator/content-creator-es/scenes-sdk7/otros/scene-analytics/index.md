> Haz seguimiento de las visitas, el engagement y la retención de los jugadores en tus escenas publicadas

# Analíticas de la escena

Como creador, es muy valioso hacer seguimiento de las visitas de los jugadores y de la forma en que los jugadores interactúan con tu scene. En lugar de asumir, puedes obtener datos reales y tomar decisiones informadas mientras iteras sobre tus diseños.

La [Creator Hub](https://decentraland.org/create) incluye una **Analytics** sección con métricas detalladas para cada scene que publiques o sobre la que tengas permisos. No necesitas añadir nada a tu scene; estos datos se recopilan automáticamente para todas las scenes publicadas.


> [!NOTE]
> **📔 Nota**: Los datos de Analytics se actualizan una vez al día. Solo los propietarios y operadores de una scene pueden ver sus datos.


## Ver los Analytics de tu scene

1. Abre Creator Hub e inicia sesión con tu cuenta.
2. Selecciona la **Analytics** pestaña en la barra de navegación superior. Verás una lista de todas las scenes publicadas que posees o administras, con métricas principales para cada una.
3. Haz clic en una scene para abrir su página de detalles, con el conjunto completo de métricas organizado en pestañas.

Los datos se registran por scene. Si un World contiene múltiples scenes, cada scene aparece como una entrada separada en la lista, con sus propias métricas.

## La lista de scenes

La lista muestra una fila por scene, con los siguientes campos:

* **Visitas únicas**: Usuarios distintos que visitaron tu scene, contados una sola vez cada uno.
* **Usuarios nuevos**: Usuarios que visitaron tu scene por primera vez.
* **Retención D7**: El porcentaje de usuarios nuevos que regresaron a tu scene al menos una vez durante los 7 días posteriores a su primera visita.
* **Ingresos**: MANA obtenida en tu scene.
* **Tiempo de juego mediano**: Tiempo mediano que cada usuario pasa en tu scene por día: la mitad de las visitas diarias son más largas, la otra mitad más cortas.

Puedes ordenar la lista o buscar por nombre, y fijar en la parte superior las scenes que más te interesan.

## La página de detalles

Haz clic en cualquier scene de la lista para abrir su página de detalles. Junto a las métricas encontrarás una tarjeta con información general sobre la scene: su tasa de likes, si el acceso es público o privado, el World en el que está publicada, quién la publicó por última vez y cuándo se actualizó por última vez. También puedes copiar la URL de la scene, entrar en la scene o crear un evento para ella directamente desde esta página.

Usa el **Rango de fechas** selector para calcular las métricas de los últimos 7, 30 o 60 días, y el **Exportar Analytics** botón para descargar los datos como un .csv.

Los gráficos semana a semana, como las cohortes de retención, los usuarios activos semanales y los visitantes con interacción social, siempre abarcan las últimas 16 semanas completas, independientemente del rango de fechas que elijas.

Una métrica que aún no tiene datos muestra un guion (`-`) en lugar de un número.

Las métricas están organizadas en cuatro pestañas.

### Resumen

Una instantánea del crecimiento, la interacción y la retención de usuarios de tu scene.

* **Visitas totales**: Cada visita a tu scene, incluidas las visitas repetidas del mismo usuario.
* **Visitas únicas**: Usuarios distintos que visitaron tu scene, contados una sola vez cada uno.
* **Usuarios nuevos**: Usuarios que visitaron tu scene por primera vez.
* **Usuarios concurrentes**: El mayor número de usuarios en tu scene al mismo tiempo.
* **Ingresos**: MANA obtenida en tu scene.
* **Retención del día 7**: El porcentaje de usuarios nuevos que regresaron a tu scene al menos una vez durante los 7 días posteriores a su primera visita.
* **Tiempo de juego mediano**: Tiempo mediano que cada usuario pasa en tu scene por día: la mitad de las visitas diarias son más largas, la otra mitad más cortas.
* **Tiempo AFK**: Tiempo promedio que los usuarios pasan en tu scene sin interactuar.
* **Escritorio / móvil**: Usuarios que visitaron tu scene desde un cliente de escritorio o móvil.

### Visitas

Cuántos usuarios visitaron tu scene y cuántos fueron visitantes nuevos o recurrentes.

* **Visitas únicas en los últimos 60 días**: Visitantes únicos, desglosados por plataforma (todas las plataformas, escritorio y móvil). Los usuarios se deduplican entre plataformas: alguien que juega tanto en escritorio como en móvil se cuenta una vez en el total de todas las plataformas y una vez en el total de cada plataforma.
* **Usuarios activos semanales**: Usuarios activos únicos durante cada semana.
* **Flujo semanal de usuarios**: Cómo se distribuyen tus usuarios semanales entre:
  * **Usuarios nuevos**: Usuarios que visitaron tu scene por primera vez.
  * **Usuarios que regresaron**: Usuarios que visitaron la semana pasada y regresaron esta semana.
  * **Usuarios reactivados**: Usuarios que habían dejado de visitar y regresaron esta semana.

### Interacción

Cómo interactúan los usuarios con tu scene una vez que están dentro.

* **Tiempo de juego diario promedio**: Tiempo promedio que cada usuario pasa en tu scene en una sola visita, con un gráfico semana a semana y el cambio respecto a la semana anterior.
* **Tiempo de juego semanal promedio**: Tiempo promedio que cada usuario pasa en tu scene por semana.
* **Interacciones sociales**: Tres señales sociales, mostradas tanto como totales semanales como la proporción de visitantes de esa semana que hizo cada cosa:
  * **Mensajes enviados**: Mensajes de chat enviados por los visitantes en tu scene.
  * **Emotes realizados**: Emotes realizados por los visitantes en tu scene.
  * **Nuevas amistades**: Solicitudes de amistad aceptadas entre visitantes en tu scene.

### Retención

El porcentaje de usuarios que regresan después de su primera sesión.

* **Plataformas**: Tu retención promedio móvil de 60 días, comparada entre escritorio y móvil.
* **Retención del día 7 por semana de cohorte**: El porcentaje de usuarios que regresaron al menos una vez durante los 7 días posteriores a su primera visita, agrupados por la semana en que entraron por primera vez en tu scene. El gráfico muestra una línea de referencia del 20% como valor de referencia saludable.
* **Tasa semanal de abandono**: El porcentaje de usuarios activos de la semana anterior que no regresaron esta semana.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/otros/scene-analytics.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
