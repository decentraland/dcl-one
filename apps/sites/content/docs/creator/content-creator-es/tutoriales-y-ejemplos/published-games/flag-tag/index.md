> Juego multijugador de mantener alejados con rondas cronometradas, tablas de clasificación persistentes, proyectiles reutilizados y un presupuesto de iluminación basado en proximidad.

# Flag Tag

![Vista previa de Flag Tag](/docs/_remote/a8bf7285858279143edb58ed.webp)

Un juego de keep-away en un Multiplayer Server 24/7. Los jugadores compiten por mantener una bandera durante rondas de cinco minutos, usando boomerangs y banana traps mientras tablas de clasificación diarias, mensuales y de todos los tiempos registran los resultados.

* [Juega en Decentraland](https://decentraland.org/jump?realm=flagtag.dcl.eth)

## Información del proyecto

**Mantenido por:** Comunidad · iillee

[Ver la fuente upstream](https://github.com/iillee/flagtag)

**Licencia:** No identificado en este catálogo. Consulta el repositorio upstream y al mantenedor para conocer los términos aplicables antes de reutilizarlo.


> [!WARNING]
> **Código mantenido por la comunidad — revísalo antes de usarlo**
> 
> Este proyecto comunitario se mantiene fuera de las organizaciones de GitHub de Decentraland Foundation y DAO. Su código fuente y su despliegue en vivo pueden cambiar de forma independiente.
> 
> La inclusión en este catálogo no implica una auditoría de seguridad ni garantiza la seguridad. Revisa el código, las dependencias y los scripts de instalación antes de ejecutarlo o reutilizarlo.
> 
> [Reportar un problema del catálogo](https://github.com/decentraland/docs/issues/new). Para posibles vulnerabilidades, sigue la política de seguridad del proyecto upstream. No incluyas detalles de explotación ni credenciales en incidencias públicas.


## Características clave

* Servidor multijugador
* Almacenamiento para la progresión del jugador, desbloqueos
* Enmascarado de Avatar para animaciones personalizadas

## Explora el código

Estos enlaces apuntan a una instantánea específica de la fuente para que las explicaciones sigan siendo útiles a medida que continúa el desarrollo upstream. Son referencias de aprendizaje, no una evaluación de seguridad.

### Reutilizar visuales de proyectiles por variante de modelo

El projectile pool precrea diez entities ocultas para cada color de boomerang. Los helpers de adquirir y liberar encuentran una entity disponible y la devuelven a su posición oculta después de detener su sonido.

**Contexto del proyecto:** El código de vuelo debe posicionar las entities adquiridas y devolverlas cuando termine; el pool depende de los assets de boomerang del proyecto y del helper de sonido.

**Referencias de la fuente:**

* [El projectile pool](https://github.com/iillee/flagtag/blob/d5d9c8fbf0f69eadb4d79c3ae4054e8947f256b6/src/systems/projectile/pool.ts#L11-L37)
* [Helpers de adquirir y liberar](https://github.com/iillee/flagtag/blob/d5d9c8fbf0f69eadb4d79c3ae4054e8947f256b6/src/systems/projectile/pool.ts#L48-L66)

### Limitar las luces activas alrededor del jugador

proximityLightSystem comprueba periódicamente, selecciona las luces más cercanas con comparaciones de distancia al cuadrado y arrays reutilizables, y cambia solo las luces cuyo estado activo difiere. La escena limita el conjunto activo a ocho y lo controla mediante su helper de día/noche.

**Contexto del proyecto:** Lee la configuración y las constantes en el mismo archivo para entender la conversión de world-position y los ajustes de intensidad específicos del dispositivo.

**Referencias de la fuente:**

* [proximityLightSystem](https://github.com/iillee/flagtag/blob/d5d9c8fbf0f69eadb4d79c3ae4054e8947f256b6/src/systems/proximityLights.ts#L109-L168)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/published-games/flag-tag.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
