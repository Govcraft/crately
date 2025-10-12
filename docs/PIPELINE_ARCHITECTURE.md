# Crately Pipeline Architecture

**Document Version**: 1.0
**Last Updated**: 2025-10-12
**Status**: Complete Implementation

## Table of Contents

1. [Overview](#overview)
2. [Architecture Principles](#architecture-principles)
3. [Pipeline Stages](#pipeline-stages)
4. [Actor System](#actor-system)
5. [Message Types](#message-types)
6. [Event Flow](#event-flow)
7. [Error Handling](#error-handling)
8. [Configuration](#configuration)
9. [State Management](#state-management)
10. [Testing Strategy](#testing-strategy)

## Overview

Crately implements an event-driven, actor-based crate processing pipeline that downloads, extracts, processes, vectorizes, and stores Rust crate documentation. The architecture is built on the **acton-reactive** framework, following pure actor model principles with zero blocking synchronization primitives.

### Key Features

- **Event-Driven**: All communication happens through message passing
- **Stateless Workers**: Download, extraction, processing, and vectorization actors maintain no internal state
- **Stateful Coordination**: Single coordinator actor tracks all pipeline state
- **Lock-Free Concurrency**: Uses `DashMap` for concurrent access without blocking
- **Automatic Retry**: Exponential backoff with configurable retry policies
- **Timeout Detection**: Background task monitors stuck pipelines
- **Database Persistence**: Embedded SurrealDB with RocksDB backend

### Pipeline Flow

```
HTTP Request → CrateReceived → Download → Extract → Chunk → Vectorize → Complete
       ↓             ↓            ↓          ↓         ↓         ↓          ↓
   ServerActor  Coordinator  Downloader FileReader Processor Vectorizer Database
```

## Architecture Principles

### Actor Model

The system strictly adheres to actor model principles:

1. **No Shared State**: All state is encapsulated within actors
2. **Message Passing Only**: Actors communicate exclusively via messages
3. **No Blocking Primitives**: Zero use of `Mutex`, `RwLock`, or other locks
4. **Cheap Handle Cloning**: `AgentHandle` is `Arc`-based for efficient distribution
5. **Supervision**: `RetryCoordinator` supervises pipeline failures and orchestrates retries

### Event-Driven Design

- **Broadcast Events**: Actors broadcast state changes to all interested parties
- **Subscription Model**: Actors subscribe to specific message types
- **Loose Coupling**: Actors don't know about each other, only message types
- **Temporal Decoupling**: Message sending is non-blocking, processing is asynchronous

### Separation of Concerns

- **Stateless Workers**: Focus on single-responsibility transformations
- **Stateful Coordinator**: Manages pipeline state and transitions
- **Retry Logic**: Centralized in `RetryCoordinator` and `CrateCoordinatorActor`
- **Persistence**: Isolated in `DatabaseActor`
- **Configuration**: Managed by `ConfigManager`

## Pipeline Stages

### Stage 1: Receipt

**Actor**: `ServerActor`
**Input**: HTTP POST `/crate` with JSON body
**Output**: `CrateReceived` broadcast event

**Responsibilities**:
- Accept HTTP requests with crate metadata
- Validate `CrateSpecifier` (name and version)
- Parse optional features array
- Broadcast `CrateReceived` to initiate pipeline

**Message Types**:
- **Receives**: `StartServer`, `StopServer`
- **Broadcasts**: `CrateReceived`, `ServerStarted`

---

### Stage 2: Coordination (Initialization)

**Actor**: `CrateCoordinatorActor`
**Input**: `CrateReceived` event
**Output**: State tracking initialization, transition to `Downloading`

**Responsibilities**:
- Initialize `CrateProcessingState` for the new crate
- Track processing start time
- Set initial status to `Received`, then immediately transition to `Downloading`
- Monitor all subsequent pipeline events
- Handle retry logic with exponential backoff
- Detect timeouts via background monitoring task
- Broadcast `CrateProcessingComplete` or `CrateProcessingFailed`

**State Tracking**:
```rust
pub struct CrateProcessingState {
    pub specifier: CrateSpecifier,
    pub features: Vec<String>,
    pub status: ProcessingStatus,  // Received → Downloading → Downloaded → ...
    pub started_at: Instant,
    pub last_updated: Instant,
    pub retry_count: u32,
    pub error: Option<String>,
}
```

**Processing Status Enum**:
```rust
pub enum ProcessingStatus {
    Received,            // Initial state
    Downloading,         // Downloading crate tarball
    Downloaded,          // Download complete
    DownloadFailed,      // Download error
    Extracting,          // Extracting documentation
    Extracted,           // Extraction complete
    ExtractionFailed,    // Extraction error
    Chunking,            // Chunking documentation
    Chunked,             // Chunking complete
    Vectorizing,         // Generating embeddings
    Vectorized,          // Vectorization complete
    Complete,            // Pipeline finished
    Failed,              // Pipeline failed permanently
}
```

**Message Types**:
- **Subscribes to**: `CrateReceived`, `CrateDownloaded`, `CrateDownloadFailed`, `DocumentationExtracted`, `DocumentationExtractionFailed`, `DocumentationChunked`, `DocumentationVectorized`
- **Broadcasts**: `RetryCrateProcessing`, `CrateProcessingFailed`, `CrateProcessingComplete`
- **Receives**: `CheckProcessingTimeouts`

---

### Stage 3: Download

**Actor**: `DownloaderActor`
**Input**: `CrateReceived` event
**Output**: `CrateDownloaded` or `CrateDownloadFailed` broadcast

**Responsibilities**:
- Download crate tarball from crates.io API
- Extract tarball to cache directory (`download_dir/crate-version/`)
- Track download duration
- Classify errors into specific `DownloadErrorKind`

**Download URL Format**:
```
https://crates.io/api/v1/crates/{crate}/{version}/download
```

**Error Classification**:
- `NotFound` - HTTP 404 or crate not found
- `Timeout` - Request timeout
- `NetworkError` - Connection or DNS issues
- `ExtractionError` - Tarball extraction failure
- `Other` - Unknown errors

**Configuration**:
```rust
pub struct DownloadConfig {
    pub download_dir: PathBuf,       // Cache directory
    pub crates_io_url: String,       // API base URL
    pub timeout_secs: u64,           // HTTP timeout
}
```

**Message Types**:
- **Subscribes to**: `CrateReceived`
- **Broadcasts**: `CrateDownloaded`, `CrateDownloadFailed`

---

### Stage 4: Documentation Extraction

**Actor**: `FileReaderActor`
**Input**: `CrateDownloaded` event
**Output**: `DocumentationExtracted` or `DocumentationExtractionFailed` broadcast

**Responsibilities**:
- Read documentation files from extracted crate directory
- Extract content from:
  - `README.md` (optional)
  - `Cargo.toml` (required - metadata)
  - `lib.rs` or `src/lib.rs` or `src/main.rs` (required - source documentation)
- Track file count and total bytes extracted
- Enforce file size limits

**Directory Structure**:
```
download_dir/crate-version/
    └── crate-version/        # Nested from tarball extraction
        ├── README.md
        ├── Cargo.toml
        └── src/
            └── lib.rs
```

**Error Classification**:
- `FileNotFound` - Required file missing
- `FileTooLarge` - File exceeds size limit
- `PermissionDenied` - Access denied
- `InvalidFormat` - Malformed file
- `Other` - Unknown errors

**Configuration**:
```rust
pub struct ReadConfig {
    pub max_file_size_bytes: u64,   // Size limit per file
    pub timeout_secs: u64,           // Read operation timeout
}
```

**Message Types**:
- **Subscribes to**: `CrateDownloaded`
- **Broadcasts**: `DocumentationExtracted`, `DocumentationExtractionFailed`

---

### Stage 5: Documentation Chunking

**Actor**: `ProcessorActor`
**Input**: `DocumentationExtracted` event
**Output**: `DocumentationChunked` broadcast, multiple `PersistDocChunk` broadcasts

**Responsibilities**:
- Re-read documentation files from extracted directory
- Concatenate documentation with section separators
- Split text into semantic chunks (paragraph boundaries)
- Apply configurable chunk size and overlap
- Generate chunk IDs following naming convention
- Create `ChunkMetadata` for each chunk
- Broadcast `PersistDocChunk` for each chunk
- Estimate total token count

**Chunk ID Format**:
```
{crate_name}_{version}_{index:03}
Example: serde_1_0_0_000, serde_1_0_0_001, ...
```

**Chunking Strategy**:
- Split on paragraph boundaries (`\n\n`)
- Target chunk size: configurable characters
- Chunk overlap: configurable characters for context preservation
- Maintains semantic meaning by respecting paragraph structure

**Configuration**:
```rust
pub struct ProcessConfig {
    pub chunk_size: usize,      // Target characters per chunk
    pub chunk_overlap: usize,   // Overlap between chunks
}
```

**Chunk Metadata**:
```rust
pub struct ChunkMetadata {
    pub content_type: String,     // "markdown"
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    pub token_count: usize,        // Estimated tokens (~4 chars/token)
    pub char_count: usize,
    pub parent_module: Option<String>,
    pub item_type: Option<String>,
    pub item_name: Option<String>,
}
```

**Message Types**:
- **Subscribes to**: `DocumentationExtracted`
- **Broadcasts**: `PersistDocChunk` (multiple), `DocumentationChunked`

---

### Stage 6: Vectorization

**Actor**: `VectorizerActor`
**Input**: `DocumentationChunked` event
**Output**: `DocumentationVectorized` broadcast, multiple `PersistEmbedding` broadcasts

**Responsibilities**:
- Generate embeddings for each chunk using OpenAI-compatible API
- Implement retry logic with exponential backoff
- Handle rate limiting (429) and server errors (5xx)
- Validate vector dimensions match configuration
- Broadcast `PersistEmbedding` for each vector
- Track vectorization duration

**API Integration**:
- **Endpoint**: Configurable (default OpenAI)
- **Model**: `text-embedding-3-small` (default)
- **Dimension**: 1536 (configurable)
- **Format**: `float` (f32 vectors)
- **Authentication**: Bearer token from `OPENAI_API_KEY` or `EMBEDDING_API_KEY`

**Retry Strategy**:
- Exponential backoff starting at configured delay
- Retry on rate limits (429) and server errors (5xx)
- Network error retry with backoff
- Maximum retry attempts: configurable

**Configuration**:
```rust
pub struct VectorizeConfig {
    pub model_name: String,          // Embedding model
    pub model_version: String,       // Model version string
    pub vector_dimension: usize,     // Expected vector size
    pub api_endpoint: String,        // API URL
    pub request_timeout_secs: u64,   // HTTP timeout
    pub max_retries: u32,            // Retry attempts
    pub retry_delay_secs: u64,       // Initial retry delay
}
```

**Message Types**:
- **Subscribes to**: `DocumentationChunked`
- **Broadcasts**: `PersistEmbedding` (multiple), `DocumentationVectorized`

---

### Stage 7: Completion

**Actor**: `CrateCoordinatorActor`
**Input**: `DocumentationVectorized` event
**Output**: `CrateProcessingComplete` broadcast

**Responsibilities**:
- Transition state to `Complete`
- Calculate total processing duration
- Broadcast `CrateProcessingComplete` with statistics:
  - Total duration in milliseconds
  - Number of stages completed (4: download → extract → chunk → vectorize)
  - Final features list

**Completion Event**:
```rust
pub struct CrateProcessingComplete {
    pub specifier: CrateSpecifier,
    pub features: Vec<String>,
    pub total_duration_ms: u64,
    pub stages_completed: u32,     // Always 4
}
```

**Message Types**:
- **Receives**: `DocumentationVectorized`
- **Broadcasts**: `CrateProcessingComplete`

## Actor System

### Core Actors (Always Active)

#### Console
**Purpose**: Terminal output formatting with color support
**Pattern**: Simple actor (returns handle only)
**State**: Stateless
**Messages**: `Init`, `PrintSuccess`, `PrintError`, `PrintWarning`, `PrintProgress`, `PrintSeparator`, `SetRawMode`

#### ConfigManager
**Purpose**: Configuration management with hot-reload support
**Pattern**: Service actor (returns handle + config)
**State**: Configuration loaded at startup
**Messages**: Configuration reload messages (future)
**Note**: Configuration is loaded synchronously at startup for immediate availability

#### DatabaseActor
**Purpose**: Embedded SurrealDB persistence with RocksDB backend
**Pattern**: Service actor (returns handle + database info)
**State**: Database connection and schema
**Connection**: `rocksdb://<path>` with namespace `crately`, database `production`
**Messages**:
- **Query**: `QueryCrate`, `ListCrates`, `QuerySimilarDocs`
- **Persist**: `PersistCrate`, `PersistDocChunk`, `PersistEmbedding`, `PersistCodeSample`
- **Events**: `DatabaseReady`, `DatabaseError`
- **Responses**: `CrateQueryResponse`, `CrateListResponse`, `SimilarDocsResponse`

#### RetryCoordinator
**Purpose**: Centralized retry management with policy-based retry logic
**Pattern**: Simple actor (returns handle only)
**State**: Retry policy configuration
**Messages**: `RetryCrateProcessing`
**Behavior**: Subscribes to `RetryCrateProcessing` and re-broadcasts `CrateReceived` after validation

### Pipeline Actors (Always Active)

#### CrateCoordinatorActor
**Purpose**: Stateful processing state tracker for pipeline coordination
**Pattern**: Simple actor (returns handle only)
**State**: `HashMap<CrateSpecifier, CrateProcessingState>` for tracking all active crates
**Responsibilities**:
- Track processing state for all crates
- Coordinate state transitions through pipeline stages
- Handle retry logic with exponential backoff
- Broadcast completion or failure events
- Monitor timeouts via background task

**Background Task**: Spawns a periodic timeout detection task that checks for stuck pipelines every `check_interval_secs` and broadcasts `CrateProcessingFailed` for timed-out crates.

#### DownloaderActor
**Purpose**: Stateless crate download worker
**Pattern**: Simple actor (returns handle only)
**State**: Configuration and HTTP client (immutable)
**Subscriptions**: `CrateReceived`

#### FileReaderActor
**Purpose**: Stateless documentation extraction worker
**Pattern**: Simple actor (returns handle only)
**State**: Configuration (immutable)
**Subscriptions**: `CrateDownloaded`

#### ProcessorActor
**Purpose**: Stateless documentation chunking worker
**Pattern**: Simple actor (returns handle only)
**State**: Configuration (immutable)
**Subscriptions**: `DocumentationExtracted`

#### VectorizerActor
**Purpose**: Stateless embedding generation worker
**Pattern**: Simple actor (returns handle only)
**State**: Configuration (immutable)
**Subscriptions**: `DocumentationChunked`

### Server Actors (Server Mode Only)

#### ServerActor
**Purpose**: Axum HTTP server lifecycle management
**Pattern**: Simple actor (returns handle only)
**State**: Server configuration, actors registry, database info
**Endpoints**: `POST /crate`
**Messages**: `StartServer`, `StopServer`, `ServerStarted`, `ServerReloaded`

#### KeyboardHandler
**Purpose**: Interactive keyboard event handling (raw mode)
**Pattern**: Simple actor (returns handle only)
**State**: Actors registry for shutdown coordination
**Behavior**: Listens for 'q' (quit) and 'r' (reload) keys
**Messages**: `KeyPressed`, `StopKeyboardHandler`, `KeyboardHandlerStarted`
**Note**: Skipped in test environments (requires TTY)

### Actor Lifecycle

**Initialization Sequence** (via `ActorSystem::initialize()`):
1. Launch acton-reactive runtime
2. Spawn Console actor → Send `Init`
3. Spawn ConfigManager actor → Load configuration
4. Spawn DatabaseActor → Initialize schema → Broadcast `DatabaseReady`
5. Spawn RetryCoordinator
6. Spawn CrateCoordinatorActor
7. Spawn DownloaderActor → Subscribe to `CrateReceived`
8. Spawn FileReaderActor → Subscribe to `CrateDownloaded`
9. Spawn ProcessorActor → Subscribe to `DocumentationExtracted`
10. Spawn VectorizerActor → Subscribe to `DocumentationChunked`

**Server Mode Initialization** (via `ActorSystem::initialize_server_actors()`):
1. Spawn KeyboardHandler (if not in test mode)
2. Spawn ServerActor
3. Set `server_mode` flag to `true`

**Shutdown Sequence** (via `ActorSystem::shutdown()`):
1. Stop ServerActor (if server mode) → Send `StopServer` → Drain connections → Stop actor
2. Stop KeyboardHandler (if server mode) → Send `StopKeyboardHandler` → Stop actor
3. Stop VectorizerActor
4. Stop ProcessorActor
5. Stop FileReaderActor
6. Stop DownloaderActor
7. Stop CrateCoordinatorActor
8. Stop RetryCoordinator
9. Stop DatabaseActor → Close connections
10. Stop ConfigManager
11. Stop Console (last, so logging works throughout)
12. Clear actor registry
13. Shutdown acton-reactive runtime
14. Flush logs (WorkerGuard dropped)

## Message Types

### Pipeline Events (Broadcast)

#### CrateReceived
```rust
pub struct CrateReceived {
    pub specifier: CrateSpecifier,    // Validated crate name and version
    pub features: Vec<String>,        // Optional feature flags
}
```
**Broadcasted by**: `ServerActor`
**Subscribed by**: `DownloaderActor`, `CrateCoordinatorActor`, `RetryCoordinator`

#### CrateDownloaded
```rust
pub struct CrateDownloaded {
    pub specifier: CrateSpecifier,
    pub features: Vec<String>,
    pub extracted_path: PathBuf,      // Path to extracted directory
    pub download_duration_ms: u64,
}
```
**Broadcasted by**: `DownloaderActor`
**Subscribed by**: `FileReaderActor`, `CrateCoordinatorActor`

#### CrateDownloadFailed
```rust
pub struct CrateDownloadFailed {
    pub specifier: CrateSpecifier,
    pub features: Vec<String>,
    pub error_message: String,
    pub error_kind: DownloadErrorKind,
}
```
**Broadcasted by**: `DownloaderActor`
**Subscribed by**: `CrateCoordinatorActor`

#### DocumentationExtracted
```rust
pub struct DocumentationExtracted {
    pub specifier: CrateSpecifier,
    pub features: Vec<String>,
    pub documentation_bytes: u64,     // Total bytes read
    pub file_count: u32,              // Number of files extracted
    pub extracted_path: PathBuf,
}
```
**Broadcasted by**: `FileReaderActor`
**Subscribed by**: `ProcessorActor`, `CrateCoordinatorActor`

#### DocumentationExtractionFailed
```rust
pub struct DocumentationExtractionFailed {
    pub specifier: CrateSpecifier,
    pub features: Vec<String>,
    pub error_message: String,
    pub error_kind: ExtractionErrorKind,
    pub elapsed_ms: u64,
}
```
**Broadcasted by**: `FileReaderActor`
**Subscribed by**: `CrateCoordinatorActor`

#### DocumentationChunked
```rust
pub struct DocumentationChunked {
    pub specifier: CrateSpecifier,
    pub features: Vec<String>,
    pub chunk_count: u32,
    pub total_tokens_estimated: u32,
}
```
**Broadcasted by**: `ProcessorActor`
**Subscribed by**: `VectorizerActor`, `CrateCoordinatorActor`

#### DocumentationVectorized
```rust
pub struct DocumentationVectorized {
    pub specifier: CrateSpecifier,
    pub features: Vec<String>,
    pub vector_count: u32,
    pub embedding_model: String,
    pub vectorization_duration_ms: u64,
}
```
**Broadcasted by**: `VectorizerActor`
**Subscribed by**: `CrateCoordinatorActor`

#### CrateProcessingComplete
```rust
pub struct CrateProcessingComplete {
    pub specifier: CrateSpecifier,
    pub features: Vec<String>,
    pub total_duration_ms: u64,
    pub stages_completed: u32,        // Always 4
}
```
**Broadcasted by**: `CrateCoordinatorActor`
**Subscribed by**: (Future: DatabaseActor for analytics)

#### CrateProcessingFailed
```rust
pub struct CrateProcessingFailed {
    pub specifier: CrateSpecifier,
    pub stage: String,                // "downloading", "extracting", etc.
    pub error: String,
}
```
**Broadcasted by**: `CrateCoordinatorActor`
**Subscribed by**: (Future: Error tracking)

#### RetryCrateProcessing
```rust
pub struct RetryCrateProcessing {
    pub specifier: CrateSpecifier,
    pub features: Vec<String>,
    pub retry_attempt: u32,
}
```
**Broadcasted by**: `CrateCoordinatorActor`
**Subscribed by**: `RetryCoordinator`

### Database Persistence Messages (Broadcast)

#### PersistDocChunk
```rust
pub struct PersistDocChunk {
    pub specifier: CrateSpecifier,
    pub chunk_index: usize,
    pub chunk_id: String,             // Unique chunk identifier
    pub content: String,              // Chunk text content
    pub source_file: String,          // Source file name
    pub metadata: ChunkMetadata,
}
```
**Broadcasted by**: `ProcessorActor`
**Subscribed by**: `DatabaseActor`

#### PersistEmbedding
```rust
pub struct PersistEmbedding {
    pub chunk_id: String,
    pub specifier: CrateSpecifier,
    pub vector: Vec<f32>,             // Embedding vector
    pub model_name: String,
    pub model_version: String,
}
```
**Broadcasted by**: `VectorizerActor`
**Subscribed by**: `DatabaseActor`

### Internal Actor Messages (Direct)

#### CheckProcessingTimeouts
```rust
pub struct CheckProcessingTimeouts;
```
**Sent by**: CrateCoordinatorActor background task (periodic)
**Handled by**: `CrateCoordinatorActor`
**Purpose**: Trigger timeout detection for stuck pipelines

## Event Flow

### Happy Path: Complete Pipeline Execution

```
1. HTTP Request → ServerActor
   ↓
2. ServerActor → Broadcast: CrateReceived
   ↓
3. DownloaderActor receives CrateReceived
   CrateCoordinatorActor receives CrateReceived → Initialize state (Received → Downloading)
   RetryCoordinator receives CrateReceived → Validates specifier
   ↓
4. DownloaderActor → Download tarball → Extract → Broadcast: CrateDownloaded
   ↓
5. FileReaderActor receives CrateDownloaded
   CrateCoordinatorActor receives CrateDownloaded → Update state (Downloaded → Extracting)
   ↓
6. FileReaderActor → Read files → Broadcast: DocumentationExtracted
   ↓
7. ProcessorActor receives DocumentationExtracted
   CrateCoordinatorActor receives DocumentationExtracted → Update state (Extracted → Chunking)
   ↓
8. ProcessorActor → Chunk text → Broadcast: PersistDocChunk (multiple) + DocumentationChunked
   ↓
9. DatabaseActor receives PersistDocChunk messages → Store chunks
   VectorizerActor receives DocumentationChunked
   CrateCoordinatorActor receives DocumentationChunked → Update state (Chunked → Vectorizing)
   ↓
10. VectorizerActor → Generate embeddings → Broadcast: PersistEmbedding (multiple) + DocumentationVectorized
    ↓
11. DatabaseActor receives PersistEmbedding messages → Store vectors
    CrateCoordinatorActor receives DocumentationVectorized → Update state (Vectorized → Complete)
    ↓
12. CrateCoordinatorActor → Broadcast: CrateProcessingComplete
```

### Error Path: Download Failure with Retry

```
1. HTTP Request → ServerActor
   ↓
2. ServerActor → Broadcast: CrateReceived
   ↓
3. DownloaderActor receives CrateReceived
   CrateCoordinatorActor receives CrateReceived → Initialize state (Received → Downloading)
   ↓
4. DownloaderActor → Download fails → Broadcast: CrateDownloadFailed
   ↓
5. CrateCoordinatorActor receives CrateDownloadFailed
   → Update state (DownloadFailed)
   → Increment retry_count
   → Check retry_count < max_retries
   ↓
6a. If retries remaining:
    → Calculate delay = retry_backoff_secs * 2^retry_count
    → Sleep for delay
    → Broadcast: RetryCrateProcessing
    ↓
7a. RetryCoordinator receives RetryCrateProcessing
    → Re-broadcast: CrateReceived (restart pipeline from beginning)

6b. If max retries exceeded:
    → Update state (Failed)
    → Broadcast: CrateProcessingFailed
```

### Timeout Path: Stuck Pipeline Detection

```
1. CrateCoordinatorActor background task (every check_interval_secs)
   → Send: CheckProcessingTimeouts to self
   ↓
2. CrateCoordinatorActor handles CheckProcessingTimeouts
   → Iterate all non-terminal states
   → Check: time_since_update > timeout_secs
   ↓
3. For each timed-out crate:
   → Broadcast: CrateProcessingFailed
   → State remains as-is (coordinator tracks timeout)
```

## Error Handling

### Error Classification

#### Download Errors
- **NotFound**: Crate or version doesn't exist on crates.io
- **Timeout**: HTTP request exceeded timeout
- **NetworkError**: Connection, DNS, or network issues
- **ExtractionError**: Tarball decompression or extraction failed
- **Other**: Unknown errors

#### Extraction Errors
- **FileNotFound**: Required file (Cargo.toml, lib.rs) missing
- **FileTooLarge**: File exceeds configured size limit
- **PermissionDenied**: Insufficient permissions to read file
- **InvalidFormat**: File content is malformed
- **Other**: Unknown errors

#### Embedding Errors
- **MissingApiKey**: API key not found in environment
- **RequestFailed**: HTTP request error
- **ApiError**: API returned error response
- **ParseError**: Response parsing failed
- **InvalidResponse**: Unexpected response format
- **DimensionMismatch**: Vector size doesn't match configuration

### Retry Strategy

#### Configuration
```rust
pub struct CoordinatorConfig {
    pub max_retries: u32,          // Maximum retry attempts
    pub retry_backoff_secs: u64,   // Base backoff duration
    pub timeout_secs: u64,         // Pipeline timeout
    pub check_interval_secs: u64,  // Timeout check frequency
}
```

#### Retry Logic
- **Exponential Backoff**: `delay = retry_backoff_secs * 2^retry_count`
- **Retry Conditions**: Download failures, extraction failures
- **Max Retries**: Configurable via `coordinator.max_retries`
- **Retry Actor**: `RetryCoordinator` validates retry requests and re-broadcasts `CrateReceived`

#### Timeout Detection
- **Background Task**: Spawned in `CrateCoordinatorActor::spawn()`
- **Interval**: `check_interval_secs` (default: 30s)
- **Logic**: For each non-terminal state, check if `time_since_update > timeout_secs`
- **Action**: Broadcast `CrateProcessingFailed` for timed-out crates
- **States Monitored**: All except `Complete` and `Failed` (uses `is_terminal()` utility)

### Error Recovery

1. **Transient Failures**: Automatically retried with exponential backoff
2. **Permanent Failures**: Marked as `Failed`, broadcast `CrateProcessingFailed`
3. **Timeout Failures**: Detected via background task, broadcast `CrateProcessingFailed`
4. **Network Failures**: Retried with exponential backoff in `VectorizerActor`
5. **API Rate Limiting**: Automatically handled with backoff in `VectorizerActor`

## Configuration

### Pipeline Configuration Structure

```rust
pub struct PipelineConfig {
    pub coordinator: CoordinatorConfig,
    pub download: DownloadConfig,
    pub read: ReadConfig,
    pub process: ProcessConfig,
    pub vectorize: VectorizeConfig,
}
```

### Coordinator Configuration
```rust
pub struct CoordinatorConfig {
    pub max_retries: u32,          // Default: 3
    pub retry_backoff_secs: u64,   // Default: 5 (5s, 10s, 20s)
    pub timeout_secs: u64,         // Default: 300 (5 minutes)
    pub check_interval_secs: u64,  // Default: 30
}
```

### Download Configuration
```rust
pub struct DownloadConfig {
    pub download_dir: PathBuf,       // Default: $XDG_CACHE_HOME/crately/downloads
    pub crates_io_url: String,       // Default: https://crates.io/api/v1
    pub timeout_secs: u64,           // Default: 60
}
```

### Read Configuration
```rust
pub struct ReadConfig {
    pub max_file_size_bytes: u64,   // Default: 10 MB
    pub timeout_secs: u64,           // Default: 30
}
```

### Process Configuration
```rust
pub struct ProcessConfig {
    pub chunk_size: usize,           // Default: 1000 characters
    pub chunk_overlap: usize,        // Default: 200 characters
}
```

### Vectorize Configuration
```rust
pub struct VectorizeConfig {
    pub model_name: String,          // Default: "text-embedding-3-small"
    pub model_version: String,       // Default: "v1"
    pub vector_dimension: usize,     // Default: 1536
    pub api_endpoint: String,        // Default: OpenAI embeddings API
    pub request_timeout_secs: u64,   // Default: 60
    pub max_retries: u32,            // Default: 3
    pub retry_delay_secs: u64,       // Default: 2
}
```

### Configuration Loading

Configuration is loaded synchronously at startup by `ConfigManager::spawn()`:
1. Load from config file (if exists)
2. Apply environment variable overrides
3. Return `(AgentHandle, Config)` tuple
4. Config is immediately available to ActorSystem

### Hot Reload

Future feature: Configuration changes can be propagated via `PipelineConfigChanged` message to update actor configurations without restart.

## State Management

### Coordinator State

The `CrateCoordinatorActor` maintains a `HashMap` of all active crate processing states:

```rust
processing_states: HashMap<CrateSpecifier, CrateProcessingState>
```

Each state tracks:
- Current processing status
- Retry count
- Timestamps (start, last update)
- Error information (if any)

### State Transitions

States transition automatically as the coordinator receives pipeline events:

```
Received (initial)
    ↓
Downloading (immediate transition)
    ↓
Downloaded ↔ DownloadFailed (retry)
    ↓
Extracting
    ↓
Extracted ↔ ExtractionFailed (retry)
    ↓
Chunking
    ↓
Chunked
    ↓
Vectorizing
    ↓
Vectorized
    ↓
Complete (terminal)

Failed (terminal, after max retries)
```

### State Utilities

```rust
impl ProcessingStatus {
    pub const fn is_failure(&self) -> bool;   // Check if status represents failure
    pub const fn is_terminal(&self) -> bool;  // Check if status is terminal
}

impl CrateProcessingState {
    pub fn time_since_update(&self) -> u64;      // Seconds since last update
    pub fn total_duration_ms(&self) -> u64;      // Total processing time
}
```

## Testing Strategy

### Actor Spawn Tests
- Verify each actor spawns successfully
- Verify actors can be stopped gracefully
- Test actor initialization with custom configuration

### Message Handler Tests
- Test each message handler in isolation
- Verify correct state transitions
- Test error handling and retry logic
- Validate broadcast and subscription patterns

### Integration Tests
- End-to-end pipeline execution
- Error path testing (download failures, extraction failures)
- Retry logic validation
- Timeout detection verification
- Database persistence verification

### Test Isolation
- Use `ActorSystem::initialize_with_db_path()` for unique database per test
- Temporary database directories prevent lock contention
- KeyboardHandler skipped in test mode (requires TTY)

### Test Utilities
- `create_test_crate_structure()` - Creates mock crate directory structure
- `generate_mock_embedding()` - Provides deterministic mock embeddings
- Timeout wrappers prevent test hangs

## Future Enhancements

### Planned Features (Not Yet Implemented)

1. **Batch Processing**: Process multiple crates in a single request
2. **Priority Queues**: Prioritize popular crates or urgent requests
3. **Incremental Updates**: Update only changed documentation
4. **Cache Invalidation**: Intelligent cache management
5. **Metrics & Analytics**: Performance metrics, success rates, error tracking
6. **Parallel Vectorization**: Batch embedding API calls
7. **Custom Embedding Models**: Support for alternative embedding providers
8. **Database Query Optimization**: Implement efficient vector similarity search
9. **Code Sample Extraction**: Extract and store code examples
10. **Webhook Notifications**: Notify external systems on completion

### Architecture Evolution

The current architecture is designed to support future enhancements without major refactoring:

- **Horizontal Scaling**: Multiple worker actors can be spawned
- **Pluggable Components**: Easy to swap implementations (e.g., different embedding APIs)
- **Event Sourcing**: All state changes are event-driven
- **Extensibility**: New pipeline stages can be added by subscribing to events

---

**Document Status**: Complete and accurate as of implementation.
**Last Updated**: 2025-10-12
**Related Issues**: Closes #50, Part of EPIC #51
