> Complete API reference for all Decentraland backend services

# Developer Platform API

Welcome to the Decentraland API Reference. This section provides comprehensive documentation for all backend services that power Decentraland.

## Overview

Decentraland's backend infrastructure consists of distributed services organized into four main categories:

* **Decentralized Layer** - Content delivery and decentralized infrastructure
* **Real-Time Communications Layer** - Service discovery, clustering, and communications
* **Feature Services** - User-facing features, content, and gamification
* **Core Services** - Authentication, marketplace, assets, and notifications

All APIs documented here follow OpenAPI specifications and support RESTful conventions unless otherwise noted.

## Service Categories

### Decentralized Layer

The foundational decentralized infrastructure for content delivery:

| Service  | Purpose          | Key Features                                                            |
| -------- | ---------------- | ----------------------------------------------------------------------- |
| Catalyst | Content delivery | Decentralized content servers, entity deployment, and filesystem access |
| Lamb2    | Lambda utilities | Serverless functions for content consumption and utility endpoints      |

### Real-Time Communications Layer

Service discovery and real-time peer-to-peer communications:

| Service             | Purpose                   | Key Features                                                          |
| ------------------- | ------------------------- | --------------------------------------------------------------------- |
| Realm Provider      | Service discovery         | Find available Catalyst servers and communication endpoints           |
| Comms Gatekeeper    | Access control            | Time-limited tokens for LiveKit media server access                   |
| Archipelago Workers | Communications clustering | WebSocket connections, island formation, and message routing via NATS |

### Feature Services

User-facing features including content, economy, and gamification:

| Service           | Purpose            | Key Features                                   |
| ----------------- | ------------------ | ---------------------------------------------- |
| Credits Server    | Virtual credits    | Decentraland's virtual currency system         |
| Badges            | Achievements       | User achievement and badge system              |
| Rewards API       | NFT rewards        | Campaign-based NFT reward distribution         |
| Exploration Games | Onboarding         | Interactive tutorials and quests               |
| Camera Reel       | Screenshots        | User-generated screenshot management           |
| Atlas Server      | Map data           | Parcel information and world map tiles         |
| Places            | Points of interest | Curated locations and featured experiences     |
| Events            | Event management   | Create, schedule, and discover in-world events |
| Worlds            | Isolated scenes    | Personal worlds and custom domains             |

### Core Services

Essential infrastructure for authentication, marketplace, assets, and notifications:

| Service               | Purpose               | Key Features                                       |
| --------------------- | --------------------- | -------------------------------------------------- |
| Auth Server           | Authentication        | Wallet-based authentication and session management |
| Marketplace Server    | NFT marketplace       | Wearables and emotes marketplace integration       |
| Asset Bundle Registry | Asset optimization    | Unity asset bundles for optimized loading          |
| Events Notifier       | Event alerts          | Notifications for upcoming events                  |
| Notifications Workers | User notifications    | Real-time and push notifications                   |
| Social Service        | Friends & communities | Friendship management and private messaging        |

## Getting Started

### Authentication

Most APIs require authentication using Decentraland's wallet-based system:

1. **Connect wallet** - Users sign authentication challenges with their Ethereum wallet
2. **Get tokens** - Auth Server validates signatures and issues JWT tokens
3. **Include tokens** - Add tokens to API requests via `Authorization` header

See the Auth Server API for detailed authentication flows.

### Base URLs

APIs are deployed across the Decentraland network:

* **Production**: `https://peer.decentraland.org` (main Catalyst)
* **Development**: Various test environments
* **Custom**: Self-hosted Catalyst nodes

Most services are accessible through Catalyst nodes. Use the Realm Provider to discover available endpoints.

### Rate Limiting

APIs implement rate limiting to ensure fair usage:

* Default limits vary by service and endpoint
* Authenticated requests typically have higher limits
* Rate limit headers included in responses
* Contact maintainers for increased limits if needed

## API Conventions

All APIs follow these conventions:

* **HTTP Methods** - Standard REST verbs (GET, POST, PUT, DELETE)
* **Status Codes** - Standard HTTP status codes
* **Content Type** - `application/json` unless otherwise specified
* **Error Format** - Consistent error response structure
* **Versioning** - Version info in URL path or headers

## Architecture Context

These APIs are part of a larger distributed system. For architectural context:

* [Architecture Overview](/docs/contributor/architecture/architecture/) - Complete system architecture
* [Backend Services](/docs/contributor/architecture/services/) - Detailed service descriptions
* [Client Architecture](/docs/contributor/architecture/client/) - How clients interact with services

## Contributing

These API specifications are maintained in their respective repositories:

* Report issues via GitHub repositories
* Propose changes through pull requests
* Join technical discussions in the [Decentraland Discord](https://dcl.gg/discord)
* Attend monthly [AMA sessions](https://www.notion.so/decentraland/Decentraland-Foundation-Technical-AMA-230fcf1ca1ec4d49922744fe91e6995d)

## Additional Resources

* [Catalyst API Specs](https://decentraland.github.io/catalyst-api-specs/) - Complete OpenAPI specifications
* [Catalyst Monitor](https://decentraland.github.io/catalyst-monitor) - Network health and status
* [Status Page](https://status.decentraland.org/) - Service status and incident history
* [GitHub Organization](https://github.com/decentraland) - Source code repositories

## OpenAPI Documentation

Each API includes:

* **Interactive documentation** - Try endpoints directly from the docs
* **Request/response examples** - Sample payloads for all operations
* **Schema definitions** - Complete data model documentation
* **Authentication requirements** - Required credentials for each endpoint

Navigate to specific APIs using the sidebar to explore detailed endpoint documentation.
