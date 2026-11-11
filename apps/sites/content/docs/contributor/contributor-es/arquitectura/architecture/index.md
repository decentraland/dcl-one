# Resumen

Decentraland es una plataforma de mundo virtual descentralizada construida sobre una arquitectura híbrida que combina la propiedad en blockchain, la entrega descentralizada de contenido y servicios centralizados para mejorar la experiencia del usuario.

## Principios arquitectónicos

### Descentralización donde importa

* **Responsabilidad** - LAND, wearables y names son NFTs en Ethereum
* **Gobernanza** - DAO controla los smart contracts críticos y el tesoro
* **Contenido** - Distribuido a través de nodos Catalyst
* **Identidad** - Autenticación basada en wallet (sin contraseñas)

### Centralización para rendimiento

* **Servicios en tiempo real** - Comunicaciones WebSocket, chat de voz
* **Descubrimiento** - Búsqueda, eventos, contenido destacado
* **Funciones sociales** - Amigos, comunidades, notificaciones
* **Optimización** - Procesamiento de assets, generación de imágenes

## Capas del sistema

### Clientes

Los jugadores interactúan a través de varios clientes:

* **Unity Explorer** - Cliente de escritorio principal (Windows, Mac, Linux)
* **Web Explorer** - Cliente basado en navegador
* **CLI** - Herramienta para desarrolladores para la creación y el despliegue de scenes
* **Clientes comunitarios** (móvil, VR, etc.) - Implementaciones alternativas del cliente de referencia usando diferentes tecnologías de renderizado.

Todos los clientes se autentican mediante wallets cripto y siguen los mismos protocolos.

### Descubrimiento (Realm Provider)

Punto de entrada único que proporciona a los clientes una descripción de realm:

* Nodos Catalyst disponibles (ubicación, capacidad, estado de salud)
* Endpoints de servicios de backend

Permite una selección inteligente de server basada en la geografía y la carga.

### Contenido descentralizado (Catalyst Network)

Red sin permisos de nodos Catalyst que almacenan y sirven:

* Definiciones de scenes y assets
* Perfiles de avatar y wearables

Cada nodo valida y almacena contenido de forma independiente. Los nodos sincronizan los deployments pero operan de forma autónoma. Véase [Catalyst Network](/docs/contributor/contributor-es/arquitectura/catalyst/) para más detalles.

### Servidores de funciones centralizadas

Los microservicios de backend proporcionan funcionalidades mejoradas:

* **Descubrimiento de servicios** - Selección de realm y monitoreo de salud
* **Comunicaciones** - Clustering de WebSocket, control de acceso de voz/video
* **Funciones de contenido** - Worlds, eventos, lugares, datos de atlas
* **Funciones sociales** - Amigos, comunidades, notificaciones
* **Funciones de la plataforma** - Logros, recompensas, créditos virtuales
* **Procesamiento de assets** - Optimización, miniaturas, traducciones

Consulta [Servicios de backend](/docs/contributor/contributor-es/arquitectura/services/) para documentación completa de los servicios.

### Infraestructura

Sistemas de soporte y servicios de terceros:

* **Broker de mensajes** - NATS para la comunicación entre servicios
* **Servidor de media** - LiveKit para voz y video
* **Indexadores de blockchain** - Consultas rápidas de datos de NFT
* **Servicio de correo electrónico** - SendGrid para notificaciones

Consulta [Infraestructura](/docs/contributor/contributor-es/arquitectura/infrastructure/) para más detalles.

### Blockchain

Los smart contracts en Ethereum y Polygon gestionan:

* **Responsabilidad** - LAND, wearables, names, estates
* **Marketplace** - Trading y royalties
* **DAO** - Gobernanza y tesorería
* **Moneda** - MANA token

## Modelo de seguridad

### Identidad y autenticación

* **Firmas de wallet** - Sin contraseñas, autenticación mediante firmas de Ethereum
* **Servidor de autenticación** - Emite tokens JWT después de verificar la firma
* **Fetch firmado** - Las API autentican las solicitudes usando firmas de wallet

### Validación de contenido

* **Verificación de firma** - Todos los deployments firmados por la wallet
* **Propiedad del puntero** - Solo el propietario de LAND puede desplegar en las coordenadas
* **Límites de tamaño** - El tamaño del contenido está restringido por el número de parcels

### Control de acceso

* **Tokens con tiempo limitado** - Los tokens JWT y de LiveKit expiran
* **Limitación de tasa** - Límites de tasa de la API por IP/usuario
* **Protección contra DDoS** - Red Catalyst distribuida

## Enfoque de escalabilidad

### Escalado horizontal

* **Nodos Catalyst** - Añadir nodos a medida que crece el contenido
* **Servicios de backend** - Los microservicios sin estado escalan de forma independiente
* **Islas** - Las comunicaciones se dividen en grupos basados en la proximidad

### Optimización

* **Paquetes de assets** - Assets optimizados para la plataforma (paquetes de Unity)
* **CDN** - Los assets estáticos se sirven a través de CDN
* **Caché** - Caché agresiva en todas las capas
* **Indexadores** - Los datos de blockchain se almacenan en caché en bases de datos rápidas

## Próximos pasos

Profundiza más en áreas arquitectónicas específicas:

* [**Servicios de backend**](/docs/contributor/contributor-es/arquitectura/services/) - Especificaciones detalladas de servicios e integraciones
* [**Catalyst Network**](/docs/contributor/contributor-es/arquitectura/catalyst/) - Entrega de contenido y despliegue
* [**Infraestructura**](/docs/contributor/contributor-es/arquitectura/infrastructure/) - Sistemas de soporte y servicios de terceros

## Recursos adicionales

* [Referencia de API](https://github.com/decentraland/docs/tree/main/apis/README.md) - Documentación completa de la API
* [Guías para colaboradores](/docs/contributor/contributor-es/guias-para-colaboradores/contributor-guides/) - Estándares de desarrollo
* [Organización de GitHub](https://github.com/decentraland) - Repositorios de código fuente


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/arquitectura/architecture.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
