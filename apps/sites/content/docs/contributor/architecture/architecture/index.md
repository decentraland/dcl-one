# Architecture Overview

Decentraland is a decentralized virtual world platform built on a hybrid architecture that combines blockchain ownership, decentralized content delivery, and centralized services for enhanced user experience.

## Architectural Principles

### Decentralization Where It Matters

* **Ownership** - Land, wearables, and names are NFTs on Ethereum
* **Governance** - DAO controls critical smart contracts and treasury
* **Content** - Distributed across Catalyst nodes
* **Identity** - Wallet-based authentication (no passwords)

### Centralization for Performance

* **Real-time services** - WebSocket communications, voice chat
* **Discovery** - Search, events, featured content
* **Social features** - Friends, communities, notifications
* **Optimization** - Asset processing, image generation

## System Layers

### Clients

Players interact through various clients:

* **Unity Explorer** - Primary desktop client (Windows, Mac, Linux)
* **Web Explorer** - Browser-based client
* **CLI** - Developer tooling for scene creation and deployment
* **Community Clients** (mobile, VR, etc) - Alternative implementations for the reference client using different rendering technologies.

All clients authenticate via crypto wallets and follow the same protocols.

### Discovery (Realm Provider)

Single entry point that provides clients with a reaml description:

* Available Catalyst nodes (location, capacity, health)
* Backend service endpoints

Enables intelligent server selection based on geography and load.

### Decentralized Content (Catalyst Network)

Permissionless network of Catalyst nodes that store and serve:

* Scene definitions and assets
* Avatar profiles and wearables

Each node independently validates and stores content. Nodes synchronize deployments but operate autonomously. See [Catalyst Network](/docs/contributor/architecture/catalyst/) for details.

### Centralized Feature Servers

Backend microservices provide enhanced functionality:

* **Service discovery** - Realm selection and health monitoring
* **Communications** - WebSocket clustering, voice/video access control
* **Content features** - Worlds, events, places, atlas data
* **Social features** - Friends, communities, notifications
* **Platform features** - Achievements, rewards, virtual credits
* **Asset processing** - Optimization, thumbnails, translations

See [Backend Services](/docs/contributor/architecture/services/) for comprehensive service documentation.

### Infrastructure

Supporting systems and third-party services:

* **Message Broker** - NATS for inter-service communication
* **Media Server** - LiveKit for voice and video
* **Blockchain Indexers** - Fast queries for NFT data
* **Email Service** - SendGrid for notifications

See [Infrastructure](/docs/contributor/architecture/infrastructure/) for details.

### Blockchain

Smart contracts on Ethereum and Polygon manage:

* **Ownership** - LAND, wearables, names, estates
* **Marketplace** - Trading and royalties
* **DAO** - Governance and treasury
* **Currency** - MANA token

## Security Model

### Identity & Authentication

* **Wallet signatures** - No passwords, authentication via Ethereum signatures
* **Auth Server** - Issues JWT tokens after signature verification
* **Signed Fetch** - APIs authenticate requests using wallet signatures

### Content Validation

* **Signature verification** - All deployments signed by wallet
* **Pointer ownership** - Only LAND owner can deploy to coordinates
* **Size limits** - Content size restricted by parcel count

### Access Control

* **Time-limited tokens** - JWT and LiveKit tokens expire
* **Rate limiting** - API rate limits per IP/user
* **DDoS protection** - Distributed Catalyst network

## Scalability Approach

### Horizontal Scaling

* **Catalyst nodes** - Add nodes as content grows
* **Backend services** - Stateless microservices scale independently
* **Islands** - Communications split into proximity-based groups

### Optimization

* **Asset bundles** - Platform-optimized assets (Unity bundles)
* **CDN** - Static assets served via CDN
* **Caching** - Aggressive caching at all layers
* **Indexers** - Blockchain data cached in fast databases

## Next Steps

Dive deeper into specific architectural areas:

* [**Backend Services**](/docs/contributor/architecture/services/) - Detailed service specifications and integrations
* [**Catalyst Network**](/docs/contributor/architecture/catalyst/) - Content delivery and deployment
* [**Infrastructure**](/docs/contributor/architecture/infrastructure/) - Supporting systems and third-party services

## Additional Resources

* [API Reference](/docs/apis/) - Complete API documentation
* [Contributor Guides](/docs/contributor/contributor-guides/contributor-guides/) - Development standards
* [GitHub Organization](https://github.com/decentraland) - Source code repositories
