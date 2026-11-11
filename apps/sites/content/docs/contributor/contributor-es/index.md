# Bienvenido, colaborador

Decentraland es un mundo virtual propiedad de sus usuarios, desarrollado de forma abierta y listo para crecer con las contribuciones de personas de todo el mundo.

Esta documentación detalla el protocolo abierto de Decentraland, desde la descripción general de la arquitectura hasta las capacidades específicas de cada componente. Aquí encontrarás los recursos que necesitas para:

1. Diseñar productos y aplicaciones en el ecosistema
2. Realizar investigaciones e investigar el contenido del mundo
3. Crear tu propio World Explorer
4. Perseguir ideas novedosas que podrían terminar en esta lista

**¡Construyamos juntos el metaverso!**

## Lo que encontrarás aquí

Esta documentación está diseñada para ayudarte a comprender y contribuir a los proyectos de Decentraland. Tanto si eres un nuevo colaborador como un desarrollador experimentado, encontrarás:

* **Descripción general de la arquitectura** - Comprender el protocolo completo de Decentraland
* **Estándares de código** - Aprender cómo escribimos código limpio y mantenible
* **Prácticas de pruebas** - Comprender nuestras estrategias integrales de pruebas
* **Pautas de documentación** - Descubrir cómo documentamos nuestro trabajo
* **Referencias de API** - Documentación completa de todos los servicios

## ¿Por dónde empezar?

Si no sabes por dónde empezar, es una buena idea echar un vistazo a la [Arquitectura](/docs/contributor/contributor-es/arquitectura/architecture/) sección para conocer los distintos componentes y su propósito general antes de profundizar en secciones más detalladas.

Para los colaboradores, comienza explorando la [Guías para colaboradores](/docs/contributor/contributor-es/guias-para-colaboradores/contributor-guides/) sección, que cubre todo, desde los estándares de pruebas hasta las pautas de calidad del código.

### Enlaces rápidos

<table data-view="cards"><thead><tr><th></th><th></th><th></th><th data-hidden data-card-target data-type="content-ref"></th></tr></thead><tbody><tr><td><strong>Arquitectura</strong></td><td>Descripción general de la arquitectura del sistema</td><td></td><td><a href="/docs/contributor/contributor-es/arquitectura/architecture/">Resumen</a></td></tr><tr><td><strong>Guías para colaboradores</strong></td><td>Pautas y buenas prácticas</td><td></td><td><a href="/docs/contributor/contributor-es/guias-para-colaboradores/contributor-guides/">Resumen</a></td></tr><tr><td><strong>Referencia de API</strong></td><td>Documentación de API para los servicios</td><td></td><td><a href="https://github.com/decentraland/docs/tree/main/apis/README.md">https://github.com/decentraland/docs/tree/main/apis/README.md</a></td></tr></tbody></table>

## Recursos adicionales

Para obtener información adicional y seguir aprendiendo, quizá te interese revisar:

* El [sesiones de Ask Me Anything](https://www.notion.so/decentraland/Decentraland-Foundation-Technical-AMA-230fcf1ca1ec4d49922744fe91e6995d?d=8508f4ca20eb4623b578feb235f86cef#bff4e53f1c664d66802e4770f46b959c), donde los miembros de la comunidad pueden presentar preguntas, comentarios o inquietudes a los equipos técnicos de la Decentraland Foundation. Se organizan mensualmente y puedes asistir libremente!
* El [página de estado](https://status.decentraland.org/), para ver el estado actual de la plataforma, el historial de incidentes y algunas métricas.
* El [Registros de Decisiones de Arquitectura](https://adr.decentraland.org/), una colección de documentos de diseño que describen el estado y la evolución de diferentes aspectos de Decentraland.
* El [organización de Decentraland en GitHub](https://github.com/decentraland), donde se alojan todos los repositorios de código abierto.

## Contribuir

Al seguir estos estándares, ayudas a mantener la calidad y la coherencia del código en todos los proyectos de Decentraland. Estas pautas se actualizan continuamente para reflejar nuestras mejores prácticas en evolución y las lecciones aprendidas de nuestra experiencia de desarrollo.

Si tienes preguntas o sugerencias sobre estos estándares, ponte en contacto con la comunidad de desarrollo de Decentraland.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/readme.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
