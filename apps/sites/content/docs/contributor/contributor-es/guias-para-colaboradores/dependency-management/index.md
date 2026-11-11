# Gestión de dependencias

Este documento define las prácticas recomendadas para gestionar dependencias en proyectos JavaScript/TypeScript en todo el ecosistema de Decentraland. Se aplica a front-end, back-end, SDKs, bibliotecas compartidas y cualquier paquete basado en npm/yarn/pnpm.

> **Resumen breve**:
>
> * **Estrategia de versiones** → Usar **versiones exactas/fijas** para `dependencias` (seguridad) y `devDependencies` (entorno de desarrollo coherente). Usa **rangos de versiones** solo para `peerDependencies` (flexibilidad).
> * **Excepción** → Los paquetes de Decentraland (`@dcl/*`, `decentraland-*`) pueden usar rangos de versiones (`^`) ya que son paquetes internos de confianza.
> * **Bibliotecas/paquetes compartidos** → Usar `peerDependencies` con rangos de versiones (^) para bibliotecas compartidas (React, @dcl/schemas, etc.). Usa `dependencias` con versiones exactas solo para utilidades seguras de duplicar.
> * **Aplicaciones/servicios finales** → Usar `dependencias` con versiones exactas para paquetes de tiempo de ejecución (React, ethers, etc.).

## 1. Justificación

Muchas bibliotecas dependen de singletons globales, React Context, identidad de clases, cachés compartidas, enums de schema o pools de DB compartidos. Instalar varias versiones de la misma biblioteca puede causar:

* Contexto no compartido entre copias
* Cachés o pools duplicados
* Fallos en comprobaciones instanceof
* Desajustes de Enum/symbol
* Mayor tamaño del bundle
* Errores de tiempo de ejecución difíciles de diagnosticar

Una gestión correcta de dependencias previene estos problemas.

### No objetivos

Este estándar no **no** intenta:

* Imponer un único package manager (npm, yarn y pnpm son compatibles)
* Garantizar una única copia física en `node_modules` (el enfoque está en la desduplicación en tiempo de ejecución/bundle)

## 2. Definiciones

| Campo                  | Propósito                                                                         | Responsabilidad del consumidor                                 | Se aplica a                |
| ---------------------- | --------------------------------------------------------------------------------- | -------------------------------------------------------------- | -------------------------- |
| `dependencias`         | Paquetes usados internamente por el módulo                                        | Ninguno: no se espera que el consumidor los proporcione        | Aplicaciones y bibliotecas |
| `peerDependencies`     | Paquetes que deben resolverse a una única versión efectiva en tiempo de ejecución | Debe instalarlos (o usar peerDependenciesMeta para opcionales) | **Solo bibliotecas**       |
| `peerDependenciesMeta` | Metadatos para peerDependencies, usados para marcar peers como opcionales         | Ninguno: solo afecta el comportamiento del gestor de paquetes  | **Solo bibliotecas**       |
| `devDependencies`      | Herramientas usadas solo durante el desarrollo                                    | Ninguno                                                        | Aplicaciones y bibliotecas |

### Acerca de peerDependenciesMeta

`peerDependenciesMeta` es un campo que proporciona metadatos sobre tus `peerDependencies`. El caso de uso más común es marcar peers como **opcionales**:

* **Peers requeridos** (predeterminado): el gestor de paquetes advertirá si no está instalado
* **Peers opcionales**: el gestor de paquetes no advertirá si faltan; tu paquete debería manejar su ausencia con elegancia

Esto es útil para paquetes que pueden funcionar con o sin ciertas dependencias (p. ej., una utilidad que funciona con o sin React, o una biblioteca que admite múltiples proveedores Web3).

### Aplicaciones vs. bibliotecas

**Bibliotecas / paquetes compartidos:**

* Usa `peerDependencies` para paquetes que deben resolverse a una única versión efectiva en tiempo de ejecución (React, ethers, @dcl/schemas)
* El consumidor (app) proporciona estas dependencias
* Evita instalaciones duplicadas y conflictos de singleton

**Aplicaciones / servicios finales:**

* A menudo usan `dependencias` para paquetes de tiempo de ejecución (React, ethers, etc.)
* Son el consumidor final, así que la duplicación no es una preocupación
* `peerDependencies` sigue siendo válido si la app pudiera ser consumida como dependencia

## 3. Cuándo usar cada campo

> **Referencia rápida**: Ver [Sección 4](#4-examples) para paquetes comunes y su ubicación recomendada.

### Usa peerDependencies para (solo bibliotecas):

**En bibliotecas/paquetes compartidos**, usa `peerDependencies` para paquetes que deben resolverse a una única versión efectiva en tiempo de ejecución:

* React, Redux, wagmi, ethers, viem
* @dcl/schemas y bibliotecas similares de todo el ecosistema
* decentraland/connect
* drivers de DB cuando se comparten pools
* Cualquier biblioteca que dependa de singletons o contexto

✅ Correcto (biblioteca):

```json
{
  "peerDependencies": {
    "react": "^18.0.0",
    "@dcl/schemas": "^20.0.0"
  }
}
```

❌ Incorrecto (biblioteca usando dependencies para bibliotecas compartidas):

```json
{
  "dependencies": {
    "react": "^18.0.0"
  }
}
```

### Usa dependencies para:

* Utilidades seguras de duplicar (lodash-es, date-fns)
* **En aplicaciones**: Paquetes de tiempo de ejecución como React, ethers (cuando la app es el consumidor final)

> **Importante**: Usa siempre **versiones exactas/fijas** en `dependencias` para seguridad.

✅ Correcto (biblioteca):

```json
{
  "dependencies": {
    "lodash-es": "4.17.21",
    "date-fns": "3.6.0"
  }
}
```

✅ Correcto (app):

```json
{
  "dependencies": {
    "react": "18.3.1",
    "ethers": "6.13.0",
    "lodash-es": "4.17.21"
  }
}
```

### peerDependencies opcionales (peerDependenciesMeta)

Para paquetes reutilizables que funcionan con o sin ciertas dependencias, usa `peerDependenciesMeta` peerDependenciesMeta para marcarlos como opcionales:

✅ Correcto:

```json
{
  "peerDependencies": {
    "react": "^18.0.0",
    "ethers": "^6.0.0"
  },
  "peerDependenciesMeta": {
    "ethers": {
      "optional": true
    }
  }
}
```

**Lo que significa:**

* `react` es **requerido**: El consumidor debe instalarlo o el gestor de paquetes advertirá
* `ethers` es **opcionales**: El consumidor no necesita instalarlo; tu paquete debería comprobar su presencia antes de usarlo

**Casos de uso:**

* Paquetes que funcionan tanto en entornos React como no React
* Bibliotecas que admiten múltiples proveedores Web3 (ethers, viem, etc.)
* Utilidades que mejoran otras bibliotecas pero no son necesarias

### Usa devDependencies para:

* Herramientas (TypeScript, ESLint, testers, bundlers)

> **Importante**: Usa siempre **versiones exactas/fijas** en `devDependencies` para asegurar un entorno de desarrollo consistente en todo el equipo.

✅ Correcto:

```json
{
  "devDependencies": {
    "typescript": "5.4.5",
    "eslint": "8.57.0",
    "vitest": "1.6.0"
  }
}
```

## 4. Ejemplos

### Paquetes que DEBEN ser peerDependencies (contexto compartido / singletons)

* `react`, `react-dom`, `react-redux`, `react-router-dom`
* `redux`, `@reduxjs/toolkit`
* `ethers`, `viem`, `wagmi`
* `@dcl/schemas`, `@dcl/ui-env`, `@dcl/crypto`
* `decentraland-dapps`, `decentraland-ui`, `decentraland-ui2`, `decentraland-connect`
* `pg`, `pg-pool`

### Paquetes que DEBERÍAN ser dependencies (seguros para duplicar)

* `lodash-es`, `date-fns`
* `uuid`, `nanoid`
* `zod`, `ajv`
* `ms`, `mitt`, `fp-future`

## 5. Estándares relacionados

* [Estándares de prueba](/docs/contributor/contributor-es/guias-para-colaboradores/testing-standards/)
* [Estándares de UI](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/ui-standards/README.md)
* [Estándares de UI web](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/web-ui-standards/README.md)
* [Arquitectura WKC](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/well-known-components/README.md)
* [Directrices de documentación de API](/docs/contributor/contributor-es/guias-para-colaboradores/api-documentation/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/dependency-management.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
