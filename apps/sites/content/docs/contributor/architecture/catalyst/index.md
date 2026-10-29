# Catalyst Network

The Catalyst network is Decentraland's decentralized content delivery infrastructure. Multiple independent operators run Catalyst nodes that store and serve content.

## Overview

The Catalyst network operates as a fully-meshed distributed system where nodes communicate directly with each other to synchronize content and maintain network health.

![Catalyst Network Topology](/docs/contributor/images/catalyst.webp)

## Catalyst Components

Each Catalyst node consists of:

### Content Server

**Purpose**: Asset storage and retrieval

Stores and serves:

* Scene definitions
* 3D models and textures
* Audio files
* Scripts
* Avatars and wearables

**API Reference**: [Catalyst API](/docs/apis/catalyst/)

### Lambdas

**Purpose**: Serverless query functions

Provides queryable endpoints:

* Entity searches
* Profile queries
* Content metadata
* Deployment history

Lambdas run as separate services that query the Content Server's database.

### Nginx

**Purpose**: Reverse proxy and load balancing

Routes requests to appropriate services:

* Content Server for uploads
* Lambdas for queries
* Static asset serving

***

## Content Deployment

### Deployment Flow

```mermaid
sequenceDiagram
    participant CLI
    participant Content as Content Server
    participant Queue as Deployments Queue
    participant Services as Backend Services
    
    CLI->>Content: POST /entities
    Content->>Content: Validate & store
    Content-->>CLI: Deployment accepted
    Content->>Queue: New deployment event
    Queue->>Services: Trigger processing
```

### Deployment Steps

1. **Compile**: CLI bundles scene using build-ecs
2. **Sign**: Creator signs deployment with wallet
3. **Upload**: CLI uploads to any Catalyst node
4. **Validate**: Catalyst validates signatures and content
5. **Store**: Content saved to storage
6. **Propagate**: Event published to deployment queue
7. **Process**: Backend services process new content

### Content Validation

Catalyst validates:

* Digital signatures (ownership proof)
* Content hashes (integrity)
* Parcel permissions (land ownership)
* File size limits
* Content policy compliance

***

## Deployments Pipeline

When content is deployed, multiple systems react:

### Asset Processing

```mermaid
flowchart LR
    Deploy[New Deployment]
    Queue[Deployments to SQS]
    AB[Asset Bundle Converter]
    Profile[Profile Images]
    Badges[Badges]
    
    Deploy --> Queue
    Queue -->|3D assets| AB
    Queue -->|Avatar changes| Profile
    Queue -->|Achievement check| Badges
```

**Triggered Services**:

* **Asset Bundle Converter** - Creates optimized bundles
* **Profile Images Producer** - Generates avatar thumbnails
* **Badges Processor** - Checks deployment achievements

### Listening Services

Services that monitor deployments:

* **Worlds** - For world deployments
* **Comms Gatekeeper** - For world voice config
* **Atlas Server** - For map updates

***

## Content Replication

Catalyst nodes replicate content to ensure availability:

### Replication Strategy

* **Active Syncing**: Nodes actively sync new deployments
* **Pull-based**: Nodes fetch missing content on-demand
* **Gossip Protocol**: Deployment announcements propagate

### Content Addressing

All content is content-addressed:

* Files identified by IPFS CID (Content Identifier)
* Immutable once deployed
* Deduplication automatic

***

## Archipelago Workers (Comms)

Communications infrastructure runs alongside Catalyst:

### Components

#### WebSocket Connector

* Maintains client connections
* Protocol handling
* Connection authentication

**API Reference**: [Archipelago Workers API](/docs/apis/archipelago-workers/)

#### Archipelago Core

* **Island formation** - Groups nearby players
* **Peer limits** - Enforces max peers per island (typically 100)
* **Position updates** - Broadcasts player positions
* **Message routing** - Routes chat and emotes

#### Stats Service

* Active user counts per scene
* Connection statistics
* Performance metrics

***

## Blockchain Integration

### Subgraph Indexer

Catalyst uses a dedicated indexer:

```mermaid
flowchart LR
    Blockchain[Ethereum/Polygon]
    Subgraph[Indexer]
    Catalyst[Catalyst]
    
    Blockchain -->|Events| Subgraph    
    Catalyst -->|Queries| Subgraph
```

The **Indexer** indexes:

* Land ownership (LAND NFTs)
* Wearable & Emotes collections
* Name registrations
* NFTs transfers

***

## Next Steps

* Review [Backend Services](/docs/contributor/architecture/services/) for service details
* See [Infrastructure](/docs/contributor/architecture/infrastructure/) for supporting systems
* Check [Architecture Overview](/docs/contributor/architecture/architecture/) for the complete picture
