# Resumen

Esta sección contiene las guías para contribuyentes y las mejores prácticas para los servicios de Decentraland. Estas guías garantizan la coherencia, la calidad y la mantenibilidad en todos los proyectos.

## Secciones

* [Documentación de API](/docs/contributor/contributor-es/guias-para-colaboradores/api-documentation/) - Estándares de OpenAPI para documentar y publicar APIs de servicios
* [Estándares de prueba](/docs/contributor/contributor-es/guias-para-colaboradores/testing-standards/) - Guías completas para escribir tests en diferentes tipos de aplicaciones y servicios
* [Componentes bien conocidos](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/well-known-components/README.md) - Estándares para construir servicios modulares y testeables usando la arquitectura WKC
* [Estándares de UI](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/ui-standards/README.md) - Patrones de Redux Toolkit y RTK Query para construir aplicaciones modernas de React
* [Estándares de UI web](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/web-ui-standards/README.md) - Estándares de la librería de componentes usando UI2, Material UI y el sistema de diseño de Decentraland

## Acerca de estos estándares

Las palabras clave "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY" y "OPTIONAL" en estos estándares deben interpretarse como se describe en [RFC 2119](https://datatracker.ietf.org/doc/html/rfc2119).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/contributor-guides.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
