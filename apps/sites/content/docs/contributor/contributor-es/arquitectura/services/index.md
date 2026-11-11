# Servicios de backend

Esta página ofrece una visión general completa de todos los servicios backend de Decentraland, sus roles arquitectónicos y cómo interactúan dentro del ecosistema. Para especificaciones detalladas de la API y endpoints, consulta el [Referencia de API](https://github.com/decentraland/docs/tree/main/apis/README.md) sección.

## Arquitectura de servicios

El backend de Decentraland se compone de microservicios distribuidos organizados en capas lógicas:

* **Capa descentralizada** - red Catalyst e infraestructura de entrega de contenido
* **Capa de comunicaciones en tiempo real** - descubrimiento de servicios y comunicaciones peer-to-peer
* **Servicios de funcionalidades** - Funcionalidad orientada al usuario (contenido, economía, gamificación)
* **Servicios centrales** - Infraestructura esencial de la plataforma (auth, marketplace, notifications)

***

## Capa descentralizada

### Catalyst

**Propósito**: red de entrega de contenido descentralizada

La red Catalyst forma la base de la infraestructura de contenido de Decentraland. Cada nodo Catalyst es un servidor completo que proporciona:

* **Servidor de contenido** - Almacenamiento y recuperación de entidades (scenes, profiles, wearables)
* **Servicio Lambdas** - endpoints de utilidad y consultas de contenido
* **Servidor de comunicaciones** - transporte de mensajes en tiempo real

**Rol en la arquitectura**: Los nodos Catalyst están distribuidos geográficamente y no requieren permisos. La red proporciona redundancia y disponibilidad de contenido sin control central.

**Integraciones clave**:

* Realm Provider - Anuncia la disponibilidad de nodos
* Asset Bundle Registry - Activa la optimización de assets en el deployment
* Archipelago Workers - Proporciona datos de islas/clústeres

**Referencia de API**: [API de Catalyst](https://github.com/decentraland/docs/tree/main/apis/apis/catalyst/README.md)

### Lamb2

**Propósito**: utilidades Lambda y funciones serverless

Lamb2 proporciona endpoints de utilidad para el consumo y las consultas de contenido que complementan al Content Server:

* \- Agregación y filtrado de scenes
* \- Consultas de profiles con alternativas
* \- Comprobaciones de estado y salud

**Rol en la arquitectura**: Actúa como una capa de conveniencia sobre el acceso bruto a entidades del Content Server, proporcionando patrones de consulta de nivel superior que los clients suelen necesitar.

**Referencia de API**: [API de Lamb2](https://github.com/decentraland/docs/tree/main/apis/apis/lamb2/README.md)

***

## Capa de comunicaciones en tiempo real

### Realm Provider

**Propósito**: descubrimiento de servicios y selección de realms

El Realm Provider es el punto de entrada para todos los clients de Decentraland. Implementa una selección inteligente de realms basada en:

* **Optimización geográfica** - Redirige a los usuarios a los nodos Catalyst más cercanos
* **Monitoreo de salud** - Filtra nodos no saludables o sobrecargados
* **Seguimiento de capacidad** - Proporciona recuentos de usuarios en tiempo real por realm
* **Agregación de scenes populares** - Identifica locations populares en todos los realms

**Rol en la arquitectura**: fuente única de verdad para la infraestructura disponible. Los clients lo consultan una vez al iniciar para descubrir todos los endpoints de servicios (Catalyst, Comms, BFF).

**Integraciones clave**:

* \- Consulta periódicamente todos los nodos Catalyst
* \- Agrega estadísticas de Archipelago
* \- Proporciona una vista unificada de la red distribuida

**Referencia de API**: [API de Realm Provider](https://github.com/decentraland/docs/tree/main/apis/apis/realm-provider/README.md)

### Comms Gatekeeper

**Propósito**: control de acceso para voz y video

El Gatekeeper gestiona el acceso a los servidores multimedia de LiveKit para el chat de voz y video:

* **Generación de tokens** - Crea tokens de acceso con tiempo limitado y permisos específicos
* **Administración de scenes** - Gestiona los permisos de admin de la scene y los baneos
* **Gestión de stream** - Controla el acceso al streaming en vivo y las claves RTMP
* **Controles de privacidad** - Aplica permisos de chat de voz privado

**Rol en la arquitectura**: Capa de seguridad entre clients e infraestructura multimedia. Evita el acceso no autorizado al tiempo que habilita modelos de permisos flexibles (basados en scenes, privados, de comunidad).

**Integraciones clave**:

* LiveKit - Emite tokens JWT para acceso al servidor multimedia
* Worlds API - Valida la propiedad del world para los derechos de admin
* Signed Fetch - Autentica todas las solicitudes mediante firmas de wallet

**Referencia de API**: [API de Comms Gatekeeper](https://github.com/decentraland/docs/tree/main/apis/apis/comms-gatekeeper/README.md)

### Archipelago Workers

**Propósito**: agrupación de comunicaciones y enrutamiento de mensajes

Archipelago implementa el protocolo de agrupación basado en proximidad que agrupa a los jugadores cercanos en "islands":

* **Transporte WebSocket** - Mantiene conexiones persistentes con los clients
* **Seguimiento de posición** - Monitorea las locations de los jugadores en tiempo real
* **Formación de islas** - Agrupa dinámicamente a los jugadores según la distance
* **Enrutamiento de mensajes** - Reenvía mensajes solo a los miembros de la island

**Componentes de la arquitectura**:

* **Conector WebSocket** - Gestiona las conexiones de los clients y la autenticación
* **Archipelago Core** - Implementa el algoritmo de agrupación y la gestión de islands
* **Servicio de estadísticas** - Proporciona métricas de conexión y recuentos de usuarios
* **Integración con NATS** - Publica actualizaciones de posición y enruta mensajes

**Rol en la arquitectura**: Optimiza las comunicaciones peer-to-peer limitando la difusión de mensajes. En lugar de emitir mensajes a todos los usuarios de un realm, los mensajes solo llegan a los jugadores dentro del rango de interacción.

**Integraciones clave**:

* broker de mensajes NATS - Enrutamiento distribuido de mensajes
* Realm Provider - Proporciona estadísticas de islas
* Places API - datos de recuento de usuarios para hot scenes

**Referencia de API**: [Archipelago Workers API](https://github.com/decentraland/docs/tree/main/apis/apis/archipelago-workers/README.md)

***

## Servicios de funcionalidades

### Worlds

**Propósito**: alojamiento aislado de scenes fuera de la cuadrícula principal

Worlds permite a los usuarios desplegar scenes en entornos aislados:

* **Worlds con nombre** - Accesibles mediante URLs personalizadas (por ejemplo, `world.dclworlds.com`)
* **Worlds privados** - control de acceso restringido por NFT
* **Permisos de Worlds** - derechos de admin y deployment gestionados por el owner

**Rol en la arquitectura**: Amplía el modelo de contenido de Decentraland más allá de la cuadrícula fija de LAND. Cada world es una scene independiente con sus propios canales de comunicación y control de acceso.

**Integraciones clave**:

* Consulta indexadores de blockchain para verificar la propiedad del NFT
* Publica actualizaciones del world en NATS para la sincronización en tiempo real de los clients
* Se integra con Comms Gatekeeper para las salas de chat de voz
* Activa el pipeline de procesamiento de assets en el deployment

**Referencia de API**: [API de Worlds](https://github.com/decentraland/docs/tree/main/apis/apis/worlds/README.md)

### Eventos

**Propósito**: gestión y descubrimiento de eventos dentro del world

Gestiona todo el ciclo de vida de los eventos virtuales:

* **CRUD de events** - Creación, actualizaciones y programación
* **Seguimiento de asistentes** - Participación de usuarios y RSVP
* **Descubrimiento** - consultas y filtrado basados en el tiempo
* **Notificaciones** - Integración con Events Notifier

**Rol en la arquitectura**: Proporciona contexto temporal al metaverso. Los events impulsan el descubrimiento y la participación de los usuarios al destacar experiencias sensibles al tiempo.

**Referencia de API**: [API de Events](https://github.com/decentraland/docs/tree/main/apis/apis/events/README.md)

### Lugares

**Propósito**: descubrimiento y curación de puntos de interés

Agrega y selecciona locations interesantes en todo Decentraland:

* **Scenes populares** - Popularidad en tiempo real basada en recuentos de usuarios
* **Lugares destacados** - Locations seleccionadas editorialmente
* **Búsqueda y filtrado** - Descubrimiento por etiquetas, categorías
* **Metadatos de POI** - Descripciones, miniaturas, coordenadas

**Rol en la arquitectura**: Resuelve el problema de descubrimiento en un mundo abierto de gran tamaño. Agrega datos de múltiples fuentes para ayudar a los usuarios a encontrar contenido atractivo.

**Integraciones clave**:

* Catalyst - metadatos de scenes e información de deployment
* Archipelago Stats - recuentos de usuarios en tiempo real por scene
* Comms Gatekeeper - disponibilidad y estado de realms

**Referencia de API**: [API de Places](https://github.com/decentraland/docs/tree/main/apis/apis/places/README.md)

### Atlas Server

**Propósito**: datos del mapa y visualización de parcels

Proporciona datos completos del mapa del world:

* **Propiedad de parcels** - datos de LAND indexados en blockchain
* **Metadatos de scenes** - nombres y coordenadas de scenes desplegadas
* **Tiles del mapa** - imágenes de tiles prerenderizadas para la visualización en la UI
* **Consultas espaciales** - Encuentra parcels por coordenadas o propietario

**Rol en la arquitectura**: proporciona el índice espacial de la cuadrícula de LAND de Decentraland. Esencial para las UIs de mapa y la navegación espacial.

**Referencia de API**: [API del Atlas Server](https://github.com/decentraland/docs/tree/main/apis/apis/atlas-server/README.md)

### Camera Reel

**Propósito**: gestión de capturas de pantalla e imágenes generadas por los usuarios

Gestiona capturas de pantalla e imágenes de los usuarios:

* **Carga de imágenes** - almacenamiento de imágenes autenticado
* **Organización de la galería** - colecciones de capturas de pantalla de usuarios
* **Metadatos** - location de la scene, marca temporal, etiquetas

**Rol en la arquitectura**: Permite compartir contenido y funciones sociales en torno a la fotografía generada por usuarios.

**Referencia de API**: [API de Camera Reel](https://github.com/decentraland/docs/tree/main/apis/apis/camera-reel/README.md)

### Juegos de exploración

**Propósito**: gamificación de onboarding y tutorials

Sistema de quests interactivo para el onboarding de nuevos usuarios:

* **Seguimiento de quests** - progreso a través de misiones del tutorial
* **Finalización de desafíos** - verificación de tareas
* **Integración de recompensas** - Conecta con el sistema de rewards

**Rol en la arquitectura**: Reduce la fricción para los nuevos usuarios al proporcionar una introducción estructurada y orientada a objetivos a las funciones de Decentraland.

**Referencia de API**: [API de Exploration Games](https://github.com/decentraland/docs/tree/main/apis/apis/exploration-games/README.md)

***

## Servicios centrales

### Servidor de autenticación

**Propósito**: autenticación basada en wallet y gestión de sesiones

Implementa el modelo de autenticación de Decentraland centrado en la wallet:

* **Verificación de firma** - Valida las firmas de wallet de Ethereum
* **Emisión de tokens JWT** - Genera tokens de sesión
* **Validación de tokens** - Verifica tokens para recursos protegidos
* **Ciclo de vida de la sesión** - actualización y expiración de tokens

**Rol en la arquitectura**: Proporciona verificación de identidad descentralizada. Los usuarios se autentican mediante firmas de wallet en lugar de contraseñas, en línea con los principios de Web3.

**Flujo de autenticación**:

1. El client solicita un challenge
2. El usuario firma el challenge con la wallet
3. Auth Server verifica la firma y emite un JWT
4. El client incluye el JWT en las solicitudes posteriores a la API

**Referencia de API**: [API del Auth Server](https://github.com/decentraland/docs/tree/main/apis/apis/auth-server/README.md)

### Servicio social

**Propósito**: grafo social y gestión de comunidades

Gestiona las relaciones sociales y las comunidades:

* **Sistema de amigos** - solicitudes, aprobaciones, listas de amigos
* **Lista de bloqueados** - bloqueo de usuarios y privacidad
* **Comunidades** - membresía y descubrimiento de grupos
* **Estado en tiempo real** - notificaciones online/offline de amigos a través de NATS
* **Mensajería privada** - infraestructura de mensajes directos

**Rol en la arquitectura**: Proporciona la capa social que transforma Decentraland de un mundo virtual en un metaverso social.

**Integraciones clave**:

* Catalyst Client - Obtiene datos de profile de los amigos
* NATS - Publica actualizaciones de estado de amigos
* Archipelago - Consulta el estado online y las locations

**Referencia de API**: [API de Social Service](https://github.com/decentraland/docs/tree/main/apis/apis/social-service/README.md)

### Marketplace Server

**Propósito**: integración del marketplace de NFTs

Backend del marketplace de wearables y emotes de Decentraland:

* **Consultas de listings** - Explorar y buscar items del marketplace
* **Historial de transacciones** - registros de compras
* **Datos de precios** - precios y tendencias del mercado
* **Metadatos de colecciones** - colecciones de wearables y emotes

**Rol en la arquitectura**: Conecta los datos de NFT on-chain con una UI de marketplace fácil de usar. Indexa eventos de blockchain para consultas rápidas.

**Referencia de API**: [API del Marketplace Server](https://github.com/decentraland/docs/tree/main/apis/apis/marketplace-server/README.md)

***

## Servicios de assets

### Registro de paquetes de assets

**Propósito**: gestión optimizada de assets

Registro de asset bundles optimizados para la plataforma:

* URLs de asset bundles por plataforma
* Versionado
* Distribución por CDN

**Referencia de API**: [API de Asset Bundle Registry](https://github.com/decentraland/docs/tree/main/apis/apis/asset-bundle-registry/README.md)

**Componentes del sistema**:

* **Convertidor de Asset Bundle** - Construye bundles específicos de la plataforma
* **Generador de LODs** - Crea variantes de nivel de detalle

**Flujo del proceso**:

```mermaid
sequenceDiagram
    participant Catalyst
    participante Queue como Deployments Queue
    participant Converter as AB Converter
    participant Registry
    
    Catalyst->>Queue: New deployment
    Queue->>Converter: Process assets
    Converter->>Converter: Build bundles
    Converter->>Registry: Register bundles
```

### Camera Reel

**Propósito**: gestión de capturas de pantalla e imágenes

Gestión de contenido generado por los usuarios:

* Almacenamiento de capturas de pantalla
* Carga de imágenes
* Organización de la galería

**Referencia de API**: [API de Camera Reel](https://github.com/decentraland/docs/tree/main/apis/apis/camera-reel/README.md)

### Servidor de Credits

**Propósito**: moneda virtual y economía

Gestiona el sistema de créditos virtuales de Decentraland:

* **Seguimiento de saldos** - saldos de créditos de los usuarios
* **Libro mayor de transacciones** - transferencias y gastos de créditos
* **Integración de compras** - conversión de fiat a créditos
* **Sincronización con blockchain** - Consulta indexadores para saldos on-chain

**Rol en la arquitectura**: Proporciona una alternativa monetaria de menor fricción a los tokens blockchain para compras y funciones dentro del world.

**Referencia de API**: [API del Credits Server](https://github.com/decentraland/docs/tree/main/apis/apis/credits-server/README.md)

### Insignias

**Propósito**: sistema de logros y gamificación

Premios de insignias impulsados por events:

* **Definiciones de insignias** - logros disponibles
* **Inventario del usuario** - insignias obtenidas por usuario
* **Concesión automática** - premios de insignias activados por events

**Componentes de la arquitectura**:

* **API de Badges** - consultas de insignias e inventarios de usuarios
* **Procesador de insignias** - Escucha Events Notifier y concede insignias

**Rol en la arquitectura**: Gamifica la participación de los usuarios mediante logros. Proporciona visibilidad y reconocimiento para las actividades de los usuarios.

**Referencia de API**: [API de Badges](https://github.com/decentraland/docs/tree/main/apis/apis/badges/README.md)

### API de recompensas

**Propósito**: distribución de recompensas NFT basada en campañas

Gestiona campañas promocionales de recompensas NFT:

* **Gestión de campañas** - programas de recompensas con tiempo limitado
* **Verificación de elegibilidad** - comprobar la cualificación del usuario
* **Coordinación de airdrops** - distribución de NFT a usuarios elegibles
* **Analítica** - seguimiento de la participación en campañas

**Rol en la arquitectura**: Permite campañas de marketing y adquisición de usuarios mediante recompensas NFT.

**Referencia de API**: [API de recompensas](https://github.com/decentraland/docs/tree/main/apis/apis/rewards/README.md)

### Notifications Workers

**Propósito**: sistema de entrega de notificaciones a usuarios

Infraestructura de notificaciones multicanal:

* **Notificaciones dentro de la app** - consultas de la bandeja de entrada de notificaciones
* **Notificaciones por email** - integración con SendGrid
* **Notificaciones push** - alertas móviles y de escritorio
* **Seguimiento del estado de lectura** - marcar notificaciones como leídas/no leídas

**Componentes de la arquitectura**:

* **Bandeja de entrada de Notifications** - API para consultar notificaciones de los usuarios
* **Procesador de Notifications** - Genera notificaciones a partir de Events Notifier

**Rol en la arquitectura**: Mantiene a los usuarios comprometidos mediante alertas oportunas sobre actividad de amigos, events, rewards y actualizaciones del sistema.

**Referencia de API**: [API de Notifications Workers](https://github.com/decentraland/docs/tree/main/apis/apis/notifications-workers/README.md)

### Events Notifier

**Propósito**: bus de events para los events de la plataforma

Sistema centralizado de publicación de events que activa acciones posteriores:

* **Events de usuario** - inicio de sesión, compras, deployments
* **Eventos del World** - actualizaciones de scenes, acciones de admin
* **Events del sistema** - mantenimiento, actualizaciones

**Rol en la arquitectura**: desacopla a los productores de events de los consumidores. Los servicios publican events sin saber quién los procesa, lo que permite arquitecturas extensibles.

**Integraciones posteriores**:

* Notifications Processor - Crea notificaciones para usuarios
* Badges Processor - Concede badges de logro
* Rewards API - Hace seguimiento de la elegibilidad de campañas

**Referencia de API**: [API de Events Notifier](https://github.com/decentraland/docs/tree/main/apis/apis/events-notifier/README.md)

***

## Patrones de interacción entre servicios

### Patrones comunes de integración

#### Librería del Catalyst Client

Muchos servicios backend usan la **librería Catalyst Client** para interactuar con la red Catalyst descentralizada:

* **Servicio social** - Obtiene datos de profile del usuario
* **API de Events** - Consulta metadatos de scenes
* **Lugares** - Agrega información de scenes
* **Registro de paquetes de assets** - Escucha eventos de despliegue

Esta biblioteca abstrae la lógica de consultas multi-nodo y failover.

#### Broker de mensajes NATS

Las actualizaciones en tiempo real y el flujo de comunicación asíncrona pasan por NATS:

* **Servicio social** - Publica cambios de estado de amigos
* **Worlds** - Difunde actualizaciones de configuración de World
* **Archipelago** - Enruta actualizaciones de posición entre islas
* **Events Notifier** - Publica eventos de la plataforma

NATS permite que los servicios se comuniquen sin acoplamiento directo.

#### Indexadores de blockchain

Los servicios consultan indexadores (no la blockchain directamente) por rendimiento:

* **Worlds** - Verifica la propiedad del NFT NAME
* **Atlas** - Obtiene datos de parcelas de LAND
* **Servidor de Credits** - Comprueba los balances de tokens
* **API de recompensas** - Verifica las condiciones de elegibilidad

Las consultas directas a la blockchain serían demasiado lentas para APIs en tiempo real.

### Pipeline de despliegue de contenido

Cuando un usuario despliega contenido en Catalyst, varios servicios posteriores lo procesan:

```mermaid
diagrama de flujo TB
    Deploy[El usuario despliega contenido]
    Catalyst[Nodo Catalyst]
    Queue[Cola de despliegue]
    
    subgraph Processing
        AB[Convertidor de Asset Bundle]
        Profile[Generador de imágenes de perfil]
        Badges[Procesador de insignias]
        Events[Notificador de eventos]
    fin
    
    Deploy --> Catalyst
    Catalyst --> Queue
    Queue --> AB
    Queue --> Profile
    Queue --> Events
    Events --> Badges
```

**Pasos del pipeline**:

1. **Catalyst** - Almacena datos sin procesar de entities
2. **Cola de despliegue** - Activa el procesamiento asíncrono
3. **Convertidor de Asset Bundle** - Compila bundles optimizados de Unity
4. **Generador de imágenes de perfil** - Renderiza miniaturas de avatar (para entities de perfil)
5. **Events Notifier** - Publica el evento de despliegue
6. **Procesador de insignias** - Comprueba si hay badges de logro de despliegue

### Matriz de dependencias de servicios

| Servicio                       | Dependencias principales     | Proporciona datos a            |
| ------------------------------ | ---------------------------- | ------------------------------ |
| Catalyst                       | Ninguno (descentralizado)    | Todos los servicios            |
| Realm Provider                 | Catalyst, Archipelago Stats  | Todos los clients              |
| Servidor de autenticación      | Ninguno                      | Todos los servicios protegidos |
| Archipelago Workers            | NATS, LiveKit                | Realm Provider, Places         |
| Comms Gatekeeper               | LiveKit, Worlds API          | Clients (generación de tokens) |
| Worlds                         | Catalyst, NATS, Blockchain   | Gatekeeper, Clients            |
| Servicio social                | Catalyst, NATS, Archipelago  | Clientes                       |
| Lugares                        | Catalyst, Archipelago, Comms | Clientes                       |
| Events Notifier                | Ninguno                      | Notifications, Badges, Rewards |
| Registro de paquetes de assets | Catalyst, Deployment Queue   | Clients (assets optimizados)   |
| Notifications Workers          | Events Notifier, SendGrid    | Clientes                       |

***

## Documentación relacionada

### Arquitectura

* [Descripción general de la arquitectura](/docs/contributor/contributor-es/arquitectura/architecture/) - Arquitectura completa del sistema
* [Catalyst Network](/docs/contributor/contributor-es/arquitectura/catalyst/) - Entrega de contenido descentralizada
* [Infraestructura](/docs/contributor/contributor-es/arquitectura/infrastructure/) - Sistemas de soporte (NATS, LiveKit, bases de datos)

### Referencias de API

* [Documentación de API](https://github.com/decentraland/docs/tree/main/apis/README.md) - Especificaciones completas de la API
* [Autenticación](/docs/contributor/contributor-es/autenticacion/authchain/) - Flujos de autenticación basados en Wallet
* [Comunicaciones](/docs/contributor/contributor-es/comunicaciones/overview/) - Protocolos de mensajería en tiempo real

### Desarrollo

* [Guías para colaboradores](/docs/contributor/contributor-es/guias-para-colaboradores/contributor-guides/) - Flujos de trabajo de desarrollo
* [Guía de pruebas](/docs/contributor/contributor-es/guias-para-colaboradores/testing-standards/testing-services-wkc/) - Prácticas de pruebas de servicios


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/arquitectura/services.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
