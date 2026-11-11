# Infraestructura

Esta página cubre la infraestructura de soporte y los servicios externos que impulsan Decentraland.

## Servicios externos (SaaS)

### LiveKit

**Propósito**: Servidor multimedia de alta calidad para voz y video

[LiveKit](https://livekit.io/) ofrece:

* Enrutamiento de medios WebRTC
* Chat de voz
* Videoconferencias
* Compartir pantalla
* Audio espacial (en desarrollo)

**Control de acceso**: [Comms Gatekeeper](https://github.com/decentraland/docs/tree/main/apis/apis/comms-gatekeeper/README.md) genera tokens con límite de tiempo

**Usado por**:

* Archipelago Workers - voz predeterminada
* Worlds - canales de voz por World
* Events - canales específicos del evento

### SendGrid

**Propósito**: Servicio de entrega de correo electrónico

Envía correos electrónicos de notificación:

* Solicitudes de amistad
* Recordatorios de eventos
* Notificaciones del sistema

**Activado por**: [Sistema de notificaciones](https://github.com/decentraland/docs/tree/main/apis/apis/notifications-workers/README.md)

***

## Indexadores de blockchain

Varios servicios de indexación proporcionan consultas rápidas de blockchain:

### Squids

**Propósito**: indexador de blockchain personalizado

[Subsquid](https://www.sqd.ai/) indexadores basados en:

* Modelos de datos personalizados
* Tiempos de sincronización rápidos
* API de GraphQL

**Índices**:

* Parcelas de LAND (NFTs de LAND)
* Wearables y colecciones
* Actividad del Marketplace
* Nombres ENS

### Satsuma

**Propósito**: proveedor de GraphQL alojado

Alojamiento administrado de subgraphs:

* Alta disponibilidad
* Autoescalado
* Monitoreo incluido

### The Graph

**Propósito**: Protocolo de indexación descentralizada

[The Graph](https://thegraph.com/) red:

* Indexadores descentralizados
* Tarifas de consulta en GRT
* Operado por la comunidad

***

## Broker de mensajes

### NATS

**Propósito**: enrutamiento de mensajes pub/sub

[NATS](https://nats.io/) ofrece:

* Entrega de mensajes en tiempo real
* Patrones pub/sub
* Patrones request/reply
* Compatibilidad con clustering

**Usado por**:

| Servicio        | Propósito                   |
| --------------- | --------------------------- |
| Archipelago     | Actualizaciones de posición |
| Servicio social | Estado de amistad           |
| Worlds          | Eventos del World           |

***

## Procesamiento de medios

### Servicio de conversión de imágenes (Metamorph)

**Propósito**: conversión y optimización de formatos de imagen

Convierte imágenes entre formatos:

* Optimización de WebP
* Generación de miniaturas
* Conversión de formato

**GitHub**: [metamorph](https://github.com/decentraland/metamorph)

### Sistema de imágenes de perfil

**Propósito**: generación de miniaturas del Avatar

Genera imágenes de vista previa del Avatar:

* Tomas de rostro
* Tomas de cuerpo completo
* Múltiples resoluciones

**Componentes**:

* **Consumidor de imágenes de perfil** - Renderiza Avatars
* **Productor de imágenes de perfil** - Escucha cambios de Avatar

***

## Workers de procesamiento de assets

### Convertidor de Asset Bundle

**Propósito**: optimización de assets específica de la plataforma

Convierte los assets desplegados en paquetes optimizados para todas las plataformas.

**Registra los paquetes en**: [Registro de paquetes de assets](https://github.com/decentraland/docs/tree/main/apis/apis/asset-bundle-registry/README.md)

### Generador de LODs

**Propósito**: variantes de nivel de detalle

Genera versiones de modelos 3D con menos polígonos para un mejor rendimiento.

***

## Sistema de recompensas

### API de recompensas

**Propósito**: distribución de recompensas NFT

Otorga recompensas NFT por acciones:

* Finalización del onboarding
* Participación en eventos
* Referidos

***

## Próximos pasos

* Revisa [Servicios de backend](/docs/contributor/contributor-es/arquitectura/services/) para especificaciones de servicio
* Revisa [Catalyst Network](/docs/contributor/contributor-es/arquitectura/catalyst/) para infraestructura de contenido
* Consulta [Descripción general de la arquitectura](/docs/contributor/contributor-es/arquitectura/architecture/) para la imagen completa


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/arquitectura/infrastructure.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
