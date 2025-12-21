# System Architecture

This document provides a comprehensive overview of the Delayed Streams Modeling system architecture.

## Overview

The delayed-streams-modeling project implements real-time Speech-to-Text (STT) and Text-to-Speech (TTS) using the Kyutai Delayed Streams Modeling technique. The system consists of server components (Rust, TypeScript) and client components (Rust) organized for production deployment.

## Component Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Client Applications                      │
├──────────────┬──────────────┬──────────────────────────────┤
│   Web Client │ kyutai-stt-cli │   tts-rs                    │
│   (Browser)  │   (Rust CLI) │   (Rust CLI)                  │
└──────┬───────┴──────┬───────┴──────────┬───────────────────┘
       │              │                   │
       │ WSS/HTTPS    │ WebSocket         │ WebSocket
       │              │                   │
┌──────▼──────────────▼───────────────────▼───────────────────┐
│              Reverse Proxy (Caddy/nginx)                     │
│  - SSL Termination                                           │
│  - Load Balancing                                            │
│  - Request Routing                                           │
└──────────────────────────┬────────────────────────────────┬─┘
                           │                                │
              ┌────────────▼────────┐          ┌────────────▼────────┐
              │   moshi-server      │          │   auth-server       │
              │   (Rust)            │          │   (TypeScript)      │
              │                     │          │                     │
              │  - STT Module       │          │  - Better Auth      │
              │  - TTS Module       │          │  - User Management  │
              │  - Batched ASR      │          │  - JWT Generation   │
              │  - Authentication   │          └─────────────────────┘
              └─────────┬───────────┘
                        │
         ┌──────────────┼──────────────┐
         │              │              │
    ┌────▼────┐   ┌────▼────┐   ┌────▼────┐
    │ Candle  │   │  Mimi   │   │  Model  │
    │ (ML)    │   │ (Audio) │   │ Weights │
    └─────────┘   └─────────┘   └─────────┘
         │
    ┌────▼────┐
    │  CUDA   │
    │  GPU    │
    └─────────┘
```

## Core Components

### 1. moshi-server (Rust)

**Location**: `server/rust/moshi/moshi-server/`

**Purpose**: High-performance server for real-time STT and TTS inference.

**Key Modules**:
- **STT (Speech-to-Text)**
  - `asr.rs`: Single-stream ASR module
  - `batched_asr.rs`: Batched ASR for concurrent streams
  - Supports real-time streaming with Voice Activity Detection (VAD)
  
- **TTS (Text-to-Speech)**
  - `tts.rs`: TTS inference module
  - `tts_preprocess.rs`: Text preprocessing for TTS
  - Supports multiple voices and streaming audio generation
  
- **Core Infrastructure**
  - `main.rs`: Server entry point, module orchestration, GPU auto-configuration
  - `auth.rs`: Better Auth JWT validation
  - `protocol.rs`: Binary WebSocket protocol implementation
  - `utils.rs`: GPU detection, Hugging Face downloads, path resolution
  - `logging.rs`: Custom structured logging with rotation
  - `metrics.rs`: Prometheus metrics for observability
  
- **ML Components**
  - `lm.rs`: Language model implementation
  - `mimi.rs`: Mimi audio codec

**Technology Stack**:
- **Rust**: Core language for performance
- **Candle**: ML framework (CUDA/Metal support)
- **Axum**: Async web framework
- **Tokio**: Async runtime
- **WebSocket**: Real-time bidirectional communication

### 2. auth-server (TypeScript)

**Location**: `server/typescript/auth-server/`

**Purpose**: User authentication and session management using Better Auth.

**Features**:
- Email/password authentication
- JWT token generation with session caching
- User approval workflow (pending/approved/rejected states)
- Database integration for user persistence

**Technology Stack**:
- **TypeScript/Node.js**
- **Better Auth**: Authentication framework
- **Database**: PostgreSQL/SQLite (configurable)

### 3. Client Applications

#### kyutai-stt-cli (Rust CLI)

**Location**: `client/rust/kyutai-stt-cli/`

**Purpose**: Standalone STT client for testing and batch processing.

**Subcommands**:
- `file`: Stream audio files to a moshi-server via WebSocket
- `mic`: Real-time microphone streaming to a moshi-server via WebSocket

**Features**:
- File-based transcription (server streaming)
- Real-time microphone input (server streaming)
- Word-level timestamps
- Optional VAD step logging
- Automatic JWT token generation from BETTER_AUTH_SECRET

#### tts-rs (Rust CLI)

**Location**: `client/rust/tts-rs/`

**Purpose**: Standalone TTS client for audio generation.

**Features**:
- Text-to-audio conversion
- Multiple voice support
- WebSocket streaming
- Audio playback and file export

#### Web Client (Next.js)

**Documentation**: `docs/nextjs-web-client-design.md`

**Purpose**: Browser-based real-time transcription UI.

**Features**:
- Real-time microphone streaming
- Live transcription display
- Voice Activity Detection visualization
- Session history and export
- Better Auth integration

## Data Flow

### STT (Speech-to-Text) Flow

```
Microphone → Web Audio API → Opus Encoding → WebSocket
    ↓
moshi-server receives Opus frames
    ↓
Mimi Audio Codec decodes to mel-spectrograms
    ↓
Delayed Streams Model generates text tokens
    ↓
Text Decoder produces transcription
    ↓
WebSocket sends partial/final text to client
    ↓
Client displays transcription with timestamps
```

### TTS (Text-to-Speech) Flow

```
User Text Input → WebSocket Request
    ↓
moshi-server receives text
    ↓
Text Preprocessing (normalization, phonemization)
    ↓
Delayed Streams Model generates audio tokens
    ↓
Mini Audio Codec encodes to waveform
    ↓
WebSocket streams PCM/Opus audio chunks
    ↓
Client plays audio in real-time
```

### Authentication Flow

```
User Login → auth-server validates credentials
    ↓
auth-server generates JWT with session data
    ↓
JWT stored in cookie (better-auth.session_token)
    ↓
Client includes JWT in WebSocket connection
    ↓
moshi-server validates JWT using BETTER_AUTH_SECRET
    ↓
User status checked (approved/pending/rejected)
    ↓
Connection accepted or rejected with status code
```

## Deployment Architecture

### Production Deployment (Recommended)

```
Internet
    │
    ├──► Caddy (Port 443/80)
    │       │
    │       ├──► /api/auth/* → auth-server (Port 3001)
    │       │
    │       └──► /* → moshi-server (Port 8080)
    │               - /api/asr-streaming (WebSocket STT)
    │               - /api/tts (HTTP TTS)
    │               - /api/tts_streaming (WebSocket TTS)
    │               - /metrics (Prometheus)
    │               - /api/status (Health)
    │
    └──► Static Files (CDN or Caddy)
            - Web client assets
```

**Benefits**:
- Automatic SSL/TLS via Let's Encrypt
- Single domain for all services
- WebSocket support with proper headers
- Load balancing capability
- Health check integration

**Configuration**: See `docs/REVERSE_PROXY_SETUP.md`

### Development Deployment

```
localhost:3000 - Web Client (Next.js dev server)
localhost:3001 - auth-server (Better Auth)
localhost:8080 - moshi-server (Rust)
```

## Configuration

### Environment Variables

**moshi-server**:
- `BETTER_AUTH_SECRET`: JWT validation secret (required for auth)
- `MOSHI_VRAM_RESERVED_MB`: Reserved GPU VRAM (default: 2048)
- `MOSHI_MODEL_PARAMS_BILLIONS`: Model size hint for batch sizing
- `MOSHI_PER_BATCH_ITEM_MB`: VRAM per batch item estimate

**auth-server**:
- `BETTER_AUTH_SECRET`: JWT signing secret (must match moshi-server)
- `DATABASE_URL`: Database connection string
- `APP_URL`: Application base URL

### Configuration Files

**moshi-server** (`configs/stt/*.toml`, `configs/tts/*.toml`):
- `configs/stt/config-stt-en-hf.toml`: English STT (2.6B model)
- `configs/stt/config-stt-en_fr-hf.toml`: English/French STT (1B model)
- `configs/tts/config-tts.toml`: High-quality TTS (n_q=16)
- `configs/tts/config-tts-fast.toml`: Fast TTS (n_q=4)
- `configs/tts/config-tts-realtime.toml`: Real-time TTS (n_q=8)

Each config specifies:
- Model paths (Hugging Face repos or local files)
- Module configuration (STT/TTS/BatchedASR)
- Inference parameters (batch size, sampling, etc.)
- Logging and warmup settings

## GPU Auto-Configuration

The moshi-server automatically detects and configures for the available GPU:

1.  **Capability Detection**: Queries CUDA device for compute capability (SM version)
2. **VRAM Calculation**: Determines available VRAM and reserves memory for system
3. **Batch Size Adjustment**: Calculates safe batch size based on model size and available VRAM
4. **DType Selection**: Chooses F16/BF16/F32 based on GPU capabilities (SM 8.0+ → BF16, SM 7.5+ → F16, else → F32)
5. **Early Warning**: Logs critical warnings if VRAM is insufficient

This prevents CUDA_ERROR_OUT_OF_MEMORY on GPUs with limited VRAM (e.g., RTX 2070 8GB).

## Monitoring and Observability

### Prometheus Metrics

**Endpoint**: `/metrics`

**Key Metrics**:
- `inference_latency_seconds`: Inference duration histogram
- `ws_close_total`: WebSocket close events by code
- `connection_error_total`: Connection errors by type
- `auth_error_total`: Authentication errors
- `system_free_vram_bytes`: Available GPU VRAM
- `system_gpu_utilization_percent`: GPU usage
- `warmup_duration_seconds`: Module warmup time
- `warmup_success_total / warmup_failure_total`: Warmup results

### Structured Logging

**Log Format**: JSON or human-readable with icons
**Rotation**: Daily and size-based (default: 100MB, 10 files)
**Levels**: TRACE, DEBUG, INFO, WARN, ERROR

**Log Styles**:
-  **Compact**: Icon only, minimal output
- **Pretty**: Icons + level names + colors (default)
- **Verbose**: Full details with file/line numbers

### Health Checks

**`/api/health`**: Simple health check for load balancers.

**`/api/status`**: Detailed status including:
- Server uptime and build info
- Module capacity (total/used/available slots)
- Authentication configuration
- Module types and paths

## Security Considerations

### Authentication

- **JWT Validation**: All WebSocket connections validate JWT signatures
- **User Approval**: Admin approval required for new users
- **Session Expiration**: Configurable JWT expiration times
- **Secret Management**: `BETTER_AUTH_SECRET` must be kept secure

### Transport Security

- **TLS/SSL**: Required for production (via reverse proxy)
- **WSS**: WebSocket Secure for encrypted audio streaming
- **HTTPS**: All HTTP endpoints served over TLS

### API Security

- **Rate Limiting**: Implemented via WebSocket close codes (4004)
- **Capacity Management**: Server rejects connections when at capacity (4000)
- **Input Validation**: Authentication tokens and protocol messages validated

## Performance Optimization

### STT Optimizations

- **Batched ASR**: Process multiple streams concurrently
- **Streaming Inference**: Emit partial results for low latency
- **VAD Integration**: Reduce processing for silence

### TTS Optimizations

- **Streaming Generation**: Stream audio chunks as they're generated
- **Configurable Quality**: Trade quality for speed (n_q parameter)
- **Voice Caching**: Voice embeddings cached after first use

### Infrastructure Optimizations

- **CUDA Graphs**: Reduce kernel launch overhead (planned)
- **Memory Pooling**: Reuse tensor allocations
- **Event Tracking**: Can be disabled for performance
- **Flash Attention**: Optimized attention mechanism support

## Troubleshooting

### Common Issues

**CUDA_ERROR_OUT_OF_MEMORY**:
- Increase `MOSHI_VRAM_RESERVED_MB`
- Use lower precision model (FP16 instead of FP32)
- Reduce batch size in config
- Use `-lowram` or `-sm75` configs for older GPUs

**WebSocket Connection Rejected**:
- Check JWT token validity (`/api/status` for auth config)
- Verify `BETTER_AUTH_SECRET` matches between servers
- Check user approval status in database

**Glitchy/Distorted TTS Audio**:
- Use faster config (`configs/tts/config-tts-fast.toml`)
- Increase client prebuffer
- Check Real-Time Factor (RTF) metrics

**See Also**:
- `docs/MOSHI_SERVER_SETUP.md` - Detailed server setup
- `docs/TTS_STREAMING_TROUBLESHOOTING.md` - TTS-specific issues
- `docs/PERFORMANCE_BENCHMARKING.md` - Performance tuning

## Future Enhancements

### Planned Features

- CUDA Graphs integration for lower latency
- Multi-GPU support for horizontal scaling
- Quantization support (INT8/INT4) for faster inference
- WebRTC support for better audio quality
- Real-time translation (STT → TTS in different language)

### Research Directions

- Improved VAD models
- Multi-speaker TTS
- Emotion/style control for TTS
- Fine-tuning pipelines for domain-specific models

## References

- **Main Paper**: [Streaming Sequence-to-Sequence Learning with Delayed Streams Modeling](https://arxiv.org/abs/2509.08753)
- **Project Website**: [kyutai.org](https://kyutai.org/)
- **Hugging Face**: [kyutai Collections](https://huggingface.co/kyutai)
- **GitHub**: [kyutai-labs/moshi](https://github.com/kyutai-labs/moshi)
