# Crately

CLI tool and HTTP service for downloading, vectorizing, and semantically searching Rust crate documentation. Submit any crate and Crately will download it from crates.io, extract docs and source, chunk it intelligently, generate embeddings, and make it searchable.

## Features

- **Actor-based pipeline** — built on [acton-reactive](https://github.com/Govcraft/acton-reactive), fully async with no locks
- **Smart chunking** — token-aware splitting using tiktoken, with separate strategies for Markdown and Rust source
- **Vector search** — semantic similarity search over embedded documentation via SurrealDB
- **HTTP API** — submit crates for processing and query status via REST
- **Integrity verification** — SHA-256 checksum validation on all downloads
- **Content deduplication** — BLAKE3 hashing prevents redundant processing
- **XDG-compliant** — config, data, cache, and logs follow XDG Base Directory spec

## Quick Start

```bash
# Set your embeddings API key
export OPENAI_API_KEY="sk-..."

# Run the server
cargo run -- serve

# Submit a crate for processing
curl -X POST http://localhost:3000/crate \
  -H "Content-Type: application/json" \
  -d '{"name": "serde", "version": "1.0.0"}'

# Check status
curl http://localhost:3000/status
```

## Configuration

Config file: `~/.config/crately/config.toml`

Key defaults:
- Port: 3000
- Embedding model: `text-embedding-3-small` (1536 dimensions)
- Chunk size: 500 tokens, overlap: 100
- Storage: in-memory SurrealDB (use `--features rocksdb` for persistence)

## Pipeline

```
Download → Extract → Chunk → Vectorize → Store
```

Each stage runs as an independent actor communicating via message passing. A coordinator tracks crate progress through all stages with timeout detection and retry logic.

## Building

```bash
cargo build                      # In-memory storage
cargo build --features rocksdb   # Persistent storage
```

Requires Rust 1.85+ (edition 2024).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
