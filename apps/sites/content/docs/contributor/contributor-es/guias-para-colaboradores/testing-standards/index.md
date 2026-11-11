# Estándares de pruebas

Esta sección describe la forma en que se realizan las pruebas en los distintos servicios y UIs de los proyectos de Decentraland.

Las palabras clave "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY" y "OPTIONAL" en este documento deben interpretarse como se describe en [RFC 2119](https://datatracker.ietf.org/doc/html/rfc2119).

## Resumen

Las pruebas son una parte crítica de nuestro proceso de desarrollo. Mantenemos altos estándares de cobertura y calidad de las pruebas en todos nuestros proyectos. Esta documentación ofrece pautas completas para:

* [**Escribir pruebas**](/docs/contributor/contributor-es/guias-para-colaboradores/testing-standards/writing-tests/) - Principios y patrones generales para escribir pruebas mantenibles y claras
* [**Probar UIs**](/docs/contributor/contributor-es/guias-para-colaboradores/testing-standards/testing-uis/) - Probar componentes React, sagas de Redux, reducers, selectors y action creators
* [**Servicios de prueba (Well-Known-Component)**](/docs/contributor/contributor-es/guias-para-colaboradores/testing-standards/testing-services-wkc/) - Probar servicios modulares construidos con la arquitectura well-known-component
* [**Probar servicios (obsoleto)**](/docs/contributor/contributor-es/guias-para-colaboradores/testing-standards/testing-services-deprecated/) - Enfoque heredado para probar servicios (como referencia)
* [**Pruebas End-to-End**](/docs/contributor/contributor-es/guias-para-colaboradores/testing-standards/testing-e2e/) - Pruebas E2E con Cypress (En progreso)

## Filosofía de pruebas

Nuestro enfoque de pruebas enfatiza:

* **Claridad** - Las pruebas deben ser autoexplicativas y fáciles de entender
* **Mantenibilidad** - Las pruebas deben ser fáciles de actualizar cuando cambian los requisitos
* **Aislamiento** - Cada prueba debe ejecutarse de forma independiente sin afectar a las demás
* **Cobertura** - Deben probarse todos los caminos críticos y casos límite
* **Valor** - Las pruebas deben proporcionar confianza de que el código funciona como se espera

## Enlaces rápidos

* [Cómo describir contextos con `describe`](/docs/contributor/contributor-es/guias-para-colaboradores/testing-standards/writing-tests/#describing-and-building-context)
* [Cómo escribir expectativas con `it`](/docs/contributor/contributor-es/guias-para-colaboradores/testing-standards/writing-tests/#describing-expectations-and-executing-code)
* [Qué probar y cuándo](/docs/contributor/contributor-es/guias-para-colaboradores/testing-standards/writing-tests/#what-to-test)
* [Cuándo y cómo simular](/docs/contributor/contributor-es/guias-para-colaboradores/testing-standards/writing-tests/#when-to-mock)

## Estándares relacionados

* [Gestión de dependencias](/docs/contributor/contributor-es/guias-para-colaboradores/dependency-management/) - Gestionar dependencias de npm y peerDependencies
* [Estándares de UI](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/ui-standards/README.md) - Patrones de Redux y RTK Query
* [Documentación de API](/docs/contributor/contributor-es/guias-para-colaboradores/api-documentation/) - Estándares de documentación de OpenAPI


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/testing-standards.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
