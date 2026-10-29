# Well-Known Components Servers

This section provides comprehensive guidelines for building services using the [Well-Known Components (WKC) standard](https://well-known-components.github.io/documentation/). Following these standards ensures consistency, testability, and maintainability across all Decentraland services.

## Overview

Well-Known Components services are built using a modular architecture based on two main building blocks:

* **[Components](/docs/contributor/contributor-guides/well-known-components/components/)** - Reusable, testable black-box units that encapsulate functionality
* **[Controllers](/docs/contributor/contributor-guides/well-known-components/controllers/)** - Request handlers that manage HTTP and WebSocket communication

### Component Types

We distinguish between two primary types of components:

* **[Adapters](/docs/contributor/contributor-guides/well-known-components/adapters/)** - Components that handle I/O processes and external integrations
* **[Logic Components](/docs/contributor/contributor-guides/well-known-components/logic-components/)** - Components that contain business logic

## Key Principles

WKC services follow these core principles:

1. **Modularity** - Each component is self-contained with clear interfaces
2. **Testability** - Components are designed to be easily unit and integration tested
3. **Dependency Injection** - Components receive their dependencies explicitly
4. **Interchangeability** - Components can be swapped with alternative implementations
5. **Separation of Concerns** - Business logic is separated from I/O operations

## Architecture

```
Service
├── Components
│   ├── Adapters (I/O operations)
│   │   ├── Database adapters
│   │   ├── External API clients
│   │   └── Storage adapters
│   └── Logic (Business logic)
│       ├── Domain logic
│       └── Orchestration
└── Controllers (Transport layer)
    ├── HTTP handlers
    └── WebSocket handlers
```

## Component Sources

Components can be imported from:

* [WKC Organization](https://github.com/well-known-components/) - Standard WKC components
* [Core Components Repository](https://github.com/decentraland/core-components) - Decentraland-specific components

## Getting Started

To build a WKC-compliant service:

1. Review the [Components](/docs/contributor/contributor-guides/well-known-components/components/) documentation to understand component structure
2. Learn about [Adapters](/docs/contributor/contributor-guides/well-known-components/adapters/) for handling external integrations
3. Understand [Logic Components](/docs/contributor/contributor-guides/well-known-components/logic-components/) for implementing business logic
4. Study [Controllers](/docs/contributor/contributor-guides/well-known-components/controllers/) for handling HTTP/WebSocket requests

## Standards Compliance

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this documentation are to be interpreted as described in [RFC 2119](https://datatracker.ietf.org/doc/html/rfc2119).

## Related Standards

* [Dependency Management](/docs/contributor/contributor-guides/dependency-management/) - Managing npm dependencies and peerDependencies
* [Testing Standards](/docs/contributor/contributor-guides/testing-standards/) - Testing patterns for WKC services
* [API Documentation](/docs/contributor/contributor-guides/api-documentation/) - OpenAPI documentation standards
