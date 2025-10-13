# Crately Pipeline Event Flow

**Visual Documentation of Message Passing and Actor Interactions**

**Document Version**: 1.1
**Last Updated**: 2025-10-13
**Status**: Updated with event-driven vectorization design

## Table of Contents

1. [Event Flow Diagrams](#event-flow-diagrams)
2. [Actor Communication Patterns](#actor-communication-patterns)
3. [Message Routing](#message-routing)
4. [State Transitions](#state-transitions)

## Event Flow Diagrams

### Happy Path: Complete Pipeline Execution

```
┌─────────────┐
│   Client    │
└──────┬──────┘
       │ HTTP POST /crate
       ↓
┌──────────────────────────────────────────────────────────────┐
│                    ServerActor                               │
│  - Validate CrateSpecifier                                   │
│  - Parse features                                            │
└──────┬───────────────────────────────────────────────────────┘
       │
       │ Broadcast: CrateReceived
       ├────────────────────────┬──────────────────┬──────────┐
       ↓                        ↓                  ↓          ↓
┌──────────────┐     ┌──────────────────┐  ┌─────────────┐  ┌──────────────┐
│ Downloader   │     │  Coordinator     │  │   Retry     │  │   Database   │
│   Actor      │     │     Actor        │  │ Coordinator │  │    Actor     │
└──────┬───────┘     └──────┬───────────┘  └─────────────┘  └──────────────┘
       │                    │
       │                    │ Initialize State
       │                    │ Received → Downloading
       ↓                    │
┌──────────────┐            │
│  Download    │            │
│  crate from  │            │
│  crates.io   │            │
└──────┬───────┘            │
       │                    │
       │ Broadcast: CrateDownloaded
       ├────────────────────┤
       ↓                    ↓
┌──────────────┐     ┌──────────────┐
│ FileReader   │     │ Coordinator  │
│   Actor      │     │   (Update)   │
└──────┬───────┘     │ Downloaded   │
       │             │      ↓       │
       │             │ Extracting   │
       ↓             └──────────────┘
┌──────────────┐
│   Extract    │
│ README.md    │
│ Cargo.toml   │
│ lib.rs       │
└──────┬───────┘
       │
       │ Broadcast: DocumentationExtracted
       ├────────────────────┤
       ↓                    ↓
┌──────────────┐     ┌──────────────┐
│  Processor   │     │ Coordinator  │
│    Actor     │     │   (Update)   │
└──────┬───────┘     │  Extracted   │
       │             │      ↓       │
       │             │  Chunking    │
       ↓             └──────────────┘
┌──────────────┐
│ Chunk into   │
│  semantic    │
│  segments    │
└──────┬───────┘
       │
       │ Broadcast: PersistDocChunk (multiple)
       ↓
┌──────────────┐
│   Database   │
│ Store chunks │
└──────────────┘
       │
       │ Broadcast: DocumentationChunked
       ├────────────────────┤
       ↓                    ↓
┌──────────────┐     ┌──────────────┐
│ Vectorizer   │     │ Coordinator  │
│   Actor      │     │   (Update)   │
└──────┬───────┘     │   Chunked    │
       │             │      ↓       │
       │             │ Vectorizing  │
       ↓             └──────────────┘
┌──────────────┐
│  Broadcast:  │
│QueryDocChunks│
└──────┬───────┘
       │
       ↓
┌──────────────┐
│   Database   │
│ Query chunks │
└──────┬───────┘
       │
       │ Broadcast: DocChunksResponse
       ↓
┌──────────────┐
│ Vectorizer   │
│ Batch chunks │
│ (100 each)   │
└──────┬───────┘
       │
       │ For each batch:
       ↓
┌──────────────┐
│  Call OpenAI │
│ Embedding API│
│(simple, no   │
│   retries)   │
└──────┬───────┘
       │
       ├──── Success ──────┬──── Failure
       │                   │
       ↓                   ↓
┌──────────────┐    ┌──────────────┐
│  Broadcast:  │    │  Broadcast:  │
│ Embedding    │    │ Embedding    │
│ Generated    │    │  Failed      │
│(per chunk)   │    │(w/ chunk_ids)│
└──────┬───────┘    └──────┬───────┘
       │                   │
       ├────────┬──────────┼─────────┬────────┐
       ↓        ↓          ↓         ↓        ↓
┌───────────┐ ┌─────┐ ┌────────┐ ┌───────┐ ┌────────┐
│ Database  │ │Con- │ │ Coord- │ │ Retry │ │Console │
│ (persist) │ │sole │ │ inator │ │ Coord │ │(error) │
└───────────┘ └─────┘ └────────┘ └───┬───┘ └────────┘
                                     │
                                     │ (retry flow)
       │                             ↓
       │                      ┌──────────────┐
       │ All batches done     │ Exponential  │
       ↓                      │ Backoff +    │
┌──────────────────────────┐ │   Jitter     │
│ Broadcast:               │ └──────┬───────┘
│ DocumentationVectorized  │        │
└──────┬───────────────────┘        │ Broadcast:
       │                            │ RetryCrate
       ↓                            │ Processing
┌──────────────────────────┐        │
│      Coordinator         │        │
│  Vectorized → Complete   │        └──► (restart query)
└──────┬───────────────────┘
       │
       │ Broadcast: CrateProcessingComplete
       ↓
┌──────────────┐
│  Analytics   │
│  (Future)    │
└──────────────┘
```

### Error Path: Download Failure with Retry

```
┌─────────────┐
│   Client    │
└──────┬──────┘
       │
       ↓
┌──────────────┐
│ ServerActor  │
│ CrateReceived│
└──────┬───────┘
       │
       ├──────────────────────────────┐
       ↓                              ↓
┌──────────────┐              ┌──────────────┐
│ Downloader   │              │ Coordinator  │
│   Actor      │              │ Initialize   │
└──────┬───────┘              └──────────────┘
       │
       │ Download fails (404, timeout, network)
       ↓
┌──────────────────┐
│ Classify error   │
│ - NotFound       │
│ - Timeout        │
│ - NetworkError   │
│ - Extraction     │
└──────┬───────────┘
       │
       │ Broadcast: CrateDownloadFailed
       ↓
┌──────────────────────────────────────────┐
│           Coordinator                     │
│  1. Update state: DownloadFailed         │
│  2. Increment retry_count                │
│  3. Check: retry_count < max_retries?    │
└──────┬───────────────────────────────────┘
       │
       ├──────── YES (retries remaining)
       │         │
       │         │ Calculate delay = backoff * 2^retry
       │         │ Sleep for calculated delay
       │         ↓
       │    ┌──────────────────────────┐
       │    │ Broadcast:               │
       │    │ RetryCrateProcessing     │
       │    └──────┬───────────────────┘
       │           │
       │           ↓
       │    ┌──────────────────────────┐
       │    │   RetryCoordinator       │
       │    │   - Validate specifier   │
       │    │   - Re-broadcast:        │
       │    │     CrateReceived        │
       │    └──────┬───────────────────┘
       │           │
       │           └──────► (Back to ServerActor broadcast)
       │
       └──────── NO (max retries exceeded)
                 │
                 │ Update state: Failed
                 ↓
            ┌──────────────────────────┐
            │ Broadcast:               │
            │ CrateProcessingFailed    │
            │ - stage: "downloading"   │
            │ - error message          │
            └──────────────────────────┘
```

### Timeout Detection Flow

```
┌─────────────────────────────────────────┐
│    CrateCoordinatorActor                │
│                                         │
│  after_start() spawns background task   │
└──────┬──────────────────────────────────┘
       │
       │ Tokio interval: check_interval_secs (30s)
       ↓
┌──────────────────────────────────────────┐
│  Background Task Loop                    │
│  - Wait for interval tick                │
│  - Send: CheckProcessingTimeouts         │
└──────┬───────────────────────────────────┘
       │
       ↓
┌──────────────────────────────────────────┐
│  CrateCoordinatorActor Handler           │
│  - Iterate all processing_states         │
│  - Skip terminal states (Complete/Failed)│
│  - Check: time_since_update > timeout?   │
└──────┬───────────────────────────────────┘
       │
       ├──── For each timed-out crate:
       │
       ↓
┌──────────────────────────────────────────┐
│  Broadcast: CrateProcessingFailed        │
│  - specifier: <crate@version>            │
│  - stage: current status (e.g., "chunking")│
│  - error: "Processing timeout after Xs"  │
└──────────────────────────────────────────┘
```

## Actor Communication Patterns

### Broadcast Pattern (One-to-Many)

```
┌──────────────┐
│   Producer   │
│    Actor     │
└──────┬───────┘
       │
       │ broker.broadcast(Message)
       │
       ├────────────────┬────────────────┬──────────────┐
       ↓                ↓                ↓              ↓
┌──────────────┐ ┌──────────────┐ ┌─────────────┐ ┌─────────────┐
│ Subscriber 1 │ │ Subscriber 2 │ │Subscriber 3 │ │Subscriber 4 │
│   Actor      │ │   Actor      │ │   Actor     │ │   Actor     │
└──────────────┘ └──────────────┘ └─────────────┘ └─────────────┘

Example:
ServerActor broadcasts CrateReceived
    → DownloaderActor (processes)
    → CrateCoordinatorActor (tracks)
    → RetryCoordinator (validates)
```

### Subscription Pattern

```
┌──────────────┐
│    Actor     │
│ handle.subscribe::<MessageType>().await
└──────┬───────┘
       │
       │ Registers interest in MessageType
       ↓
┌──────────────────────────────────┐
│     acton-reactive Broker        │
│  Maintains subscription registry │
└──────────────────────────────────┘
       │
       │ When MessageType is broadcast:
       │
       ↓
┌──────────────┐
│ Deliver to   │
│ all          │
│ subscribers  │
└──────────────┘

Subscriptions in Crately:
- DownloaderActor subscribes to: CrateReceived
- FileReaderActor subscribes to: CrateDownloaded
- ProcessorActor subscribes to: DocumentationExtracted
- VectorizerActor subscribes to: DocumentationChunked
- RetryCoordinator subscribes to: RetryCrateProcessing
- CrateCoordinatorActor subscribes to: ALL pipeline events
```

### Direct Messaging Pattern (Point-to-Point)

```
┌──────────────┐
│   Sender     │
└──────┬───────┘
       │
       │ handle.send(Message).await
       ↓
┌──────────────┐
│  Receiver    │
│    Actor     │
└──────────────┘

Example:
Console actor uses direct messaging:
- console.send(PrintSuccess(...)).await
- console.send(PrintError(...)).await

ServerActor uses direct messaging:
- server.send(StartServer).await
- server.send(StopServer).await

Background task uses direct messaging:
- coordinator_handle.send(CheckProcessingTimeouts).await
```

### Event-Driven Query/Response Pattern (Vectorization)

```
┌──────────────────────────────────────────────────────────────┐
│             Event-Driven Query/Response Pattern              │
│                  (No Synchronous Waiting)                    │
└──────────────────────────────────────────────────────────────┘

Step 1: Request Phase
┌──────────────┐
│ Vectorizer   │
│   Actor      │
└──────┬───────┘
       │
       │ broker.broadcast(QueryDocChunks { specifier })
       │
       ├────────────────────────────────────┐
       ↓                                    ↓
┌──────────────┐                    ┌──────────────┐
│ Database     │                    │ Vectorizer   │
│ (receives    │                    │ (continues   │
│  request)    │                    │  processing) │
└──────┬───────┘                    └──────────────┘
       │
       │ Query database
       │
       ↓
┌──────────────┐
│ Execute      │
│ SurrealDB    │
│ query        │
└──────┬───────┘

Step 2: Response Phase
       │
       │ broker.broadcast(DocChunksResponse { chunks })
       │
       ├────────────────────────────────────┐
       ↓                                    ↓
┌──────────────┐                    ┌──────────────┐
│ Vectorizer   │                    │   (other     │
│ (receives    │                    │  subscribers │
│  response)   │                    │   if any)    │
└──────┬───────┘                    └──────────────┘
       │
       │ Batch chunks
       │ Call API
       │
       ├──── Success ──────┬──── Failure
       │                   │
       ↓                   ↓
┌──────────────┐    ┌──────────────┐
│  Broadcast:  │    │  Broadcast:  │
│ Embedding    │    │ Embedding    │
│ Generated    │    │  Failed      │
└──────────────┘    └──────┬───────┘
                           │
                           ↓
                    ┌──────────────┐
                    │    Retry     │
                    │ Coordinator  │
                    │  (handles    │
                    │   retry)     │
                    └──────────────┘

Key Characteristics:
- No synchronous waiting (no oneshot channels, no .await on response)
- VectorizerActor continues processing after broadcasting request
- DatabaseActor responds via broadcast (loosely coupled)
- Multiple actors can subscribe to response
- Retry handled by separate coordinator actor
- Natural backpressure through message queues
```

## Message Routing

### Message Type Registry

```
Pipeline Events (Broadcast):
├── CrateReceived
│   ├─► DownloaderActor
│   ├─► CrateCoordinatorActor
│   └─► RetryCoordinator
│
├── CrateDownloaded
│   ├─► FileReaderActor
│   └─► CrateCoordinatorActor
│
├── CrateDownloadFailed
│   └─► CrateCoordinatorActor
│
├── DocumentationExtracted
│   ├─► ProcessorActor
│   └─► CrateCoordinatorActor
│
├── DocumentationExtractionFailed
│   └─► CrateCoordinatorActor
│
├── DocumentationChunked
│   ├─► VectorizerActor
│   └─► CrateCoordinatorActor
│
├── QueryDocChunks
│   └─► DatabaseActor
│
├── DocChunksResponse
│   └─► VectorizerActor
│
├── EmbeddingGenerated
│   ├─► DatabaseActor (persist)
│   ├─► Console (display)
│   └─► CrateCoordinatorActor (track)
│
├── EmbeddingFailed
│   ├─► RetryCoordinator (orchestrate retry)
│   ├─► Console (display error)
│   └─► CrateCoordinatorActor (track failure)
│
├── DocumentationVectorized
│   └─► CrateCoordinatorActor
│
├── PersistDocChunk
│   └─► DatabaseActor
│
├── PersistEmbedding (DEPRECATED - replaced by EmbeddingGenerated)
│   └─► DatabaseActor
│
├── RetryCrateProcessing
│   └─► RetryCoordinator
│
├── CrateProcessingComplete
│   └─► (Future: Analytics, Webhooks)
│
└── CrateProcessingFailed
    └─► (Future: Error Tracking)

Internal Messages (Direct):
├── CheckProcessingTimeouts
│   └─► CrateCoordinatorActor (self-message from background task)
│
├── StartServer / StopServer
│   └─► ServerActor
│
├── Init / PrintSuccess / PrintError / ...
│   └─► Console
│
└── DatabaseReady
    └─► (Future: Dependent actors)
```

### Routing Characteristics

**Broadcast Messages**:
- Fire-and-forget delivery
- Multiple subscribers receive independently
- No response expected
- Async, non-blocking
- Order not guaranteed across subscribers

**Direct Messages**:
- Point-to-point delivery
- Single recipient
- Can expect responses (via `AgentReply`)
- Async, non-blocking
- FIFO order per recipient

## State Transitions

### Processing Status State Machine

```
                    ┌─────────────────────┐
                    │     Received        │ (Initial state)
                    └──────────┬──────────┘
                               │ immediate
                               ↓
                    ┌─────────────────────┐
        ┌───────────┤    Downloading      │
        │           └──────────┬──────────┘
        │                      │
        │      ┌───────────────┴───────────────┐
        │      ↓                               ↓
        │  ┌─────────────────────┐    ┌─────────────────────┐
        │  │    Downloaded       │    │  DownloadFailed     │◄─┐
        │  └──────────┬──────────┘    └──────────┬──────────┘  │
        │             │                           │             │
        │             │                           │ retry       │
        │             ↓                           └─────────────┘
        │  ┌─────────────────────┐                      (if retry_count < max)
        │  │    Extracting       │
        │  └──────────┬──────────┘                      (if retry_count >= max)
        │             │                                          │
        │      ┌──────┴───────────────────────┐                 │
        │      ↓                               ↓                 │
        │  ┌─────────────────────┐    ┌─────────────────────┐  │
        │  │    Extracted        │    │ ExtractionFailed    │◄─┤
        │  └──────────┬──────────┘    └──────────┬──────────┘  │
        │             │                           │             │
        │             │                           │ retry       │
        │             ↓                           └─────────────┘
        │  ┌─────────────────────┐
        │  │     Chunking        │
        │  └──────────┬──────────┘
        │             │
        │             ↓
        │  ┌─────────────────────┐
        │  │      Chunked        │
        │  └──────────┬──────────┘
        │             │
        │             ↓
        │  ┌─────────────────────┐
        │  │    Vectorizing      │
        │  └──────────┬──────────┘
        │             │
        │             ↓
        │  ┌─────────────────────┐
        │  │    Vectorized       │
        │  └──────────┬──────────┘
        │             │
        │             ↓
        │  ┌─────────────────────┐
        └─►│     Complete        │ (Terminal)
           └─────────────────────┘

                     OR

           ┌─────────────────────┐
           │      Failed         │ (Terminal - max retries exceeded)
           └─────────────────────┘
```

### State Transition Triggers

```
State               Trigger Event                  Next State(s)
─────────────────────────────────────────────────────────────────
Received            (none - immediate)         →  Downloading

Downloading         CrateDownloaded            →  Downloaded
                    CrateDownloadFailed        →  DownloadFailed

Downloaded          (immediate)                →  Extracting

DownloadFailed      retry_count < max_retries →  Downloading (via retry)
                    retry_count >= max_retries →  Failed

Extracting          DocumentationExtracted     →  Extracted
                    DocumentationExtractionFailed → ExtractionFailed

Extracted           (immediate)                →  Chunking

ExtractionFailed    retry_count < max_retries →  Extracting (via retry)
                    retry_count >= max_retries →  Failed

Chunking            DocumentationChunked       →  Chunked

Chunked             (immediate)                →  Vectorizing

Vectorizing         DocumentationVectorized    →  Vectorized

Vectorized          (immediate)                →  Complete

Complete            (terminal - no transitions)
Failed              (terminal - no transitions)
```

### Utility Methods for State Management

```rust
// Check if status represents a failure
pub const fn is_failure(&self) -> bool {
    matches!(self,
        ProcessingStatus::DownloadFailed
        | ProcessingStatus::ExtractionFailed
        | ProcessingStatus::Failed
    )
}

// Check if status is terminal (no more transitions)
pub const fn is_terminal(&self) -> bool {
    matches!(self,
        ProcessingStatus::Complete
        | ProcessingStatus::Failed
    )
}
```

### State Update Operations

```rust
// Update to new status
fn update_status(&mut self, status: ProcessingStatus) {
    self.status = status;
    self.last_updated = Instant::now();
}

// Update with error information
fn update_with_error(&mut self, status: ProcessingStatus, error: String) {
    self.status = status;
    self.last_updated = Instant::now();
    self.error = Some(error);
}

// Increment retry counter
fn increment_retry(&mut self) {
    self.retry_count += 1;
    self.last_updated = Instant::now();
}
```

---

**Related Documentation**: See [PIPELINE_ARCHITECTURE.md](./PIPELINE_ARCHITECTURE.md) for detailed component descriptions and configuration.

**Architecture Updates**:
- Added Event-Driven Query/Response Pattern section showing vectorization flow
- Updated happy path diagram with batch vectorization and retry coordination
- Added new messages: QueryDocChunks, DocChunksResponse, EmbeddingGenerated, EmbeddingFailed
- Documented separation of retry logic from VectorizerActor into RetryCoordinator
- Illustrated multi-subscriber pattern for embedding events (Database, Console, Coordinator, RetryCoordinator)
