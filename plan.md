# Rust Rewrite Plan

## Goal

Rewrite Bilive-Coyote as a native Rust application, not a TypeScript-to-Rust translation.

This plan is intended to be read directly by the implementation agent. The current project is the complete behavior and protocol source of truth. The Rust version should keep the user-facing contracts stable while rebuilding the internal architecture around Rust-native ownership, typed messages, async tasks, cancellation, and explicit state snapshots.

## Non-Goals

- Do not preserve the current TypeScript class layout.
- Do not port the current EventBus pattern.
- Do not introduce a plugin system before it is needed.
- Do not rewrite the frontend in this pass.
- Do not change the HTTP API, panel WebSocket event shape, or `config.json` shape unless the user explicitly approves it.
- Do not commit secrets. Use local config or environment only for manual connection tests.

## Executor Startup Strategy

The executor may have a 200K-token context window. Do not ask it to blindly read every file.

The repository itself is small enough to understand, but dependencies, generated files, and lockfiles can waste context. The executor should first build a compact behavior inventory, then read implementation files by subsystem as it rewrites each subsystem.

Start with this read order:

1. Read `plan.md` first. Treat it as the rewrite contract.
2. Read `README.md`, `docs/development.md`, `docs/bilibili-sources.md`, `docs/coyote.md`, and `docs/setup.md` to understand user-facing behavior.
3. Read `package.json`, `config.json`, and `src/config/*.ts` to lock the config/API compatibility contract.
4. Read `public/index.html`, `public/js/*.js`, and `public/css/main.css` to preserve frontend expectations. Do not rewrite the frontend in this pass.
5. Read `src/server/*.ts` to capture HTTP routes and panel WebSocket event shapes.
6. Read `src/coyote/*.ts` to capture the DG-LAB protocol and pairing behavior.
7. Read `src/bilibili/**/*.ts` to capture Open Platform, Broadcast, live socket, parser, signer, and WBI behavior.
8. Read `src/engine/*.ts` to capture gift mapping and strength semantics.

Ignore unless specifically needed:

- `node_modules/`
- `bun.lock`
- `.git/`
- build artifacts such as `dist/`
- editor or tool-generated files

Before implementation, the executor should produce a short inventory with:

- stable external contracts
- source subsystem responsibilities
- frontend assumptions
- Rust module plan
- open questions or risky ambiguities

If the inventory conflicts with this plan, stop and ask the user before implementing.

## How Much Implementation Detail To Expose

The executor should receive firm decisions for:

- Runtime, web framework, WebSocket stack, TLS stack, config persistence, logging, and static asset embedding.
- Process architecture: actor-like managers, typed commands, typed status snapshots, and cancellation.
- External contracts: HTTP endpoints, panel WebSocket events, config JSON, Coyote QR behavior, and Bilibili source names.
- Protocol responsibilities and test targets.

The executor may decide:

- Exact private function names.
- Small file boundaries inside each module.
- Local parsing helper structure.
- Whether a small helper is generic or duplicated when duplication is clearer.
- Exact error enum granularity, as long as API responses and logs remain clean.

The executor must not decide:

- Replacing `axum`, `tokio`, `tokio-tungstenite`, or `rustls` without discussion.
- Introducing global mutable state or broad `Arc<Mutex<App>>` ownership.
- Replacing enum-based Bilibili source dispatch with boxed trait objects unless there is a concrete need.
- Changing frontend API contracts as a side effect of the rewrite.

## Technology Stack

Use this baseline stack:

```toml
tokio = { version = "1", features = ["macros", "rt-multi-thread", "signal", "sync", "time", "net"] }
axum = { version = "0.8", features = ["ws", "json"] }
tokio-tungstenite = { version = "0.29", default-features = false, features = ["connect", "rustls-tls-webpki-roots"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tokio-util = "0.7"
futures-util = "0.3"
flate2 = "1"
brotli = "8"
hmac = "0.12"
sha2 = "0.10"
hex = "0.4"
md-5 = "0.10"
urlencoding = "2"
base64 = "0.22"
qrcode = "0.14"
uuid = { version = "1", features = ["v4", "serde"] }
clap = { version = "4", features = ["derive"] }
rust-embed = "8"
mime_guess = "2"
local-ip-address = "0.6"
```

Notes:

- Treat these as baseline crate choices. Patch and compatible minor updates are fine if `cargo` resolves them cleanly and the architectural choices stay the same.
- Use `rustls`, not native TLS or OpenSSL. Single-binary distribution should not depend on platform TLS bindings.
- Use `anyhow` only in startup glue and binaries. Domain modules should prefer `thiserror`.
- Use `qrcode` as SVG data URL unless PNG compatibility becomes necessary.
- Use `rust-embed` so release builds embed `web/`, while debug builds may serve directly from the filesystem if that keeps frontend iteration fast.

## Final Repository Shape

The final native Rust project should look like this:

```text
Cargo.toml
Cargo.lock
src/
  main.rs
  app.rs
  config/
  http/
  panel/
  coyote/
  bilibili/
    mod.rs
    live_socket/
    open_platform/
    broadcast/
  engine/
web/
  index.html
  css/
  js/
docs/
README.md
```

During implementation, it is acceptable to build the Rust application in a temporary `rust/` directory to avoid colliding with the TypeScript `src/`. The final state should be native: Rust code in root `src/`, frontend in `web/`, old TypeScript build files removed or clearly archived only if the user asks.

## Architecture

Use actor-like long-running tasks with typed handles.

Recommended top-level state:

```rust
struct AppState {
    config: ConfigStore,
    bilibili: BilibiliHandle,
    coyote: CoyoteHandle,
    strength: StrengthHandle,
    panel: PanelHub,
}
```

Each handle should be a small cloneable command sender plus status reader. The actual managers live in background tasks.

Use channel types intentionally:

- `mpsc` for commands where backpressure matters.
- `watch` for latest status snapshots.
- `broadcast` for panel event fanout, where slow panel clients may miss old events.

Avoid storing WebSocket sinks in shared locks. Each WebSocket session should own its write half in a writer task and receive outgoing commands through a channel.

## Core Domain Types

Model the domain directly:

```rust
enum Channel {
    A,
    B,
}

enum RuleChannel {
    A,
    B,
    Both,
}

enum CoinType {
    Gold,
    Silver,
    All,
}

struct GiftEvent {
    gift_name: String,
    gift_id: Option<u64>,
    coin_type: CoinType,
    uname: String,
    num: u32,
}

struct GiftRule {
    gift_name: String,
    gift_id: Option<u64>,
    coin_type: CoinType,
    channel: RuleChannel,
    strength_add: u8,
    duration: u64,
}
```

Use serde renames to preserve JSON compatibility with the current frontend and `config.json`.

## Config

Preserve the current `config.json` shape:

```json
{
  "bilibili": {
    "source": "open-platform",
    "openPlatform": {
      "appKey": "",
      "appSecret": "",
      "code": "",
      "appId": 0
    },
    "broadcast": {
      "roomId": 0
    }
  },
  "coyote": {
    "wsPort": 9999
  },
  "server": {
    "httpPort": 3000,
    "host": "0.0.0.0"
  },
  "rules": [],
  "safety": {
    "limitA": 80,
    "limitB": 80,
    "decayEnabled": true,
    "decayRate": 2
  }
}
```

Implement:

- `ConfigStore::load_or_default(path)`
- `ConfigStore::get()`
- `ConfigStore::update(partial)`
- `ConfigStore::set_rules(rules)`
- atomic persistence with temp file plus rename
- config watch channel for components that need live updates

Keep validation in Rust DTOs. Reject invalid API input with HTTP 400 and a short JSON error.

## HTTP API Contract

Keep these endpoints:

```text
GET  /api/status
POST /api/bilibili/start
POST /api/bilibili/stop
GET  /api/bilibili/status
GET  /api/coyote/status
GET  /api/coyote/qrcode
POST /api/coyote/strength
POST /api/coyote/emergency
GET  /api/config
PUT  /api/config
GET  /api/config/rules
PUT  /api/config/rules
GET  /ws/panel
```

The main HTTP server uses `axum`.

The Coyote App WebSocket should remain on the configured Coyote port and use the bridge ID path:

```text
ws://<lan-ip>:<coyote.wsPort>/<bridge-id>
```

The QR content must remain:

```text
https://www.dungeon-lab.com/app-download.php#DGLAB-SOCKET#<ws-url>
```

## Panel Events

Keep the current panel event style:

```json
{ "type": "gift", "data": {} }
{ "type": "strength", "data": {} }
{ "type": "bilibili:status", "data": {} }
{ "type": "coyote:status", "data": {} }
```

Panel clients connect to `/ws/panel`. On slow clients, dropping old events is acceptable. Status can always be recovered from `GET /api/status`.

## Bilibili Design

Do not use a trait-object plugin system for the first Rust version.

Use enum dispatch:

```rust
enum BilibiliStart {
    OpenPlatform(OpenPlatformStart),
    Broadcast(BroadcastStart),
}
```

`BilibiliManager` owns the active source task:

1. Receive `Start`.
2. Cancel the current source task with `CancellationToken`.
3. Wait for it to exit.
4. Start the selected source.
5. Publish status through `watch`.
6. Persist source-specific config after a successful start.

Shared live socket logic belongs in `bilibili/live_socket/`:

- packet building
- packet frame parsing
- auth packet send
- heartbeat
- reconnect policy
- deflate and brotli decompression
- JSON message extraction from control-character-delimited bodies

Source modules should only provide source-specific concerns:

```text
bilibili/open_platform/
  signer
  API start/end game
  parser
  source task

bilibili/broadcast/
  WBI
  room/danmu info fetch
  parser
  source task
```

The live socket should emit `serde_json::Value` or a narrow internal message type to the source parser. Source parsers emit `GiftEvent`.

## Coyote Design

Use a dedicated Coyote manager and a separate axum listener on `coyote.wsPort`.

Responsibilities:

- generate a stable bridge ID for the process
- accept App WebSocket connections on `/<bridge-id>`
- complete bind handshake
- allow only one paired App at a time
- close the old App session when a new App pairs
- send heartbeat every 30 seconds
- parse App strength feedback
- expose latest `CoyoteStatus` through `watch`
- accept `CoyoteCommand` through `mpsc`

Suggested commands:

```rust
enum CoyoteCommand {
    SendStrength { channel: Channel, mode: u8, value: u8 },
    Clear { channel: Channel },
}
```

Suggested status:

```rust
struct CoyoteStatus {
    paired: bool,
    strength_a: u8,
    strength_b: u8,
    limit_a: u8,
    limit_b: u8,
}
```

Keep Coyote protocol encode/decode as pure functions with tests.

## Strength Engine

The strength engine should be an actor, not a passive helper.

Inputs:

- gift events from Bilibili
- manual strength API commands
- emergency stop API command
- Coyote App status feedback
- config updates
- 1 second decay tick

Outputs:

- Coyote commands
- panel `gift` events
- panel `strength` events
- latest strength status snapshot

It owns:

- current strength for A/B
- baseline for manual/App feedback
- active gift expiries
- App limits
- effective safety limits

Rules:

- Gift strength adds temporary delta with expiry.
- Manual strength sets baseline and clears gift expiries for that channel.
- App feedback updates local baseline and clears expiries if App value differs.
- Emergency sends strength 0 and clear for both channels.
- Effective limit is `min(config_limit, app_limit)`.
- Decay runs only when `safety.decayEnabled` is true.

## Static Frontend

Move `public/` to `web/`.

The first Rust rewrite should keep the frontend JS/CSS/HTML behavior stable. Only update paths and small API compatibility issues if needed.

Use `rust-embed` and `mime_guess` to serve:

- `/` as `index.html`
- `/css/...`
- `/js/...`
- other embedded assets if added later

## Error Handling

Use domain error enums:

```rust
enum ConfigError {}
enum BilibiliError {}
enum CoyoteError {}
enum ApiError {}
```

Map API errors to JSON:

```json
{ "error": "short readable message" }
```

Use:

- 400 for validation errors
- 404 for missing routes or unavailable QR
- 500 for unexpected internal errors

Connection and protocol parse issues should be logged with `tracing`, not surfaced as noisy panics.

## Logging

Use `tracing`.

Default log level should be practical for users:

```text
info for lifecycle
warn for recoverable protocol/network problems
error for exhausted reconnect or failed startup
debug/trace for raw protocol details
```

Support:

```text
RUST_LOG=debug
```

## CLI

Use `clap`.

Initial CLI:

```text
bilive-coyote --config config.json
```

Optional later flags:

```text
--host
--http-port
--coyote-port
```

Do not add a large command surface in the first rewrite.

## Testing Plan

Unit tests are required for:

- Coyote message encode/decode
- Coyote strength feedback parsing
- Bilibili packet encode/decode
- nested deflate/brotli packet handling
- JSON extraction from control-character-delimited Bilibili bodies
- Open Platform signature generation
- Open Platform gift parser
- Broadcast gift parser
- WBI signing
- config default load and atomic update behavior
- strength engine gift expiry, decay, limits, manual override, App feedback, emergency stop

Integration or smoke tests:

- `GET /api/status` returns expected shape
- config update round trip
- panel WebSocket receives status events

Manual tests:

- start app
- open panel
- generate Coyote QR
- pair DG-LAB App
- start Open Platform source with local credentials
- start Broadcast source with a room ID
- send manual strength
- emergency stop

## Verification Commands

Before submitting the Rust rewrite:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

If frontend assets are retained, also manually open the panel and verify it connects to `/ws/panel`.

## Commit Plan

Prefer these commits:

1. `chore: scaffold native rust application`
2. `feat: port config store and http api contract`
3. `feat: add panel websocket and embedded frontend`
4. `feat: implement coyote websocket protocol`
5. `feat: implement bilibili live socket core`
6. `feat: implement bilibili open platform source`
7. `feat: implement bilibili broadcast source`
8. `feat: implement strength engine`
9. `chore: replace bun distribution with cargo release workflow`
10. `docs: update setup and development docs for rust`

If a commit becomes too large, split by behavior rather than by file type.

## Acceptance Criteria

The rewrite is acceptable when:

- The project builds as a Rust binary from the repository root.
- The frontend opens and uses the Rust backend without behavior regressions.
- Existing `config.json` continues to work.
- Both Bilibili sources can be started from the panel.
- Coyote QR pairing works on LAN.
- Manual strength and emergency stop work.
- Gift rules update strength and log gift events.
- Decay and effective limits behave as documented.
- No TypeScript runtime or Bun dependency is required to run or build the app.
- Tests cover protocol parsing and strength behavior.

## Executor Guidance

Read the current TypeScript only to understand behavior, edge cases, protocol quirks, and frontend contracts.

Do not copy its class structure, event bus structure, or lifecycle ownership model. If a Rust implementation starts to look like TypeScript with borrow checker syntax, stop and redesign around tasks, channels, typed enums, and status snapshots.

When in doubt, choose the smaller Rust-native design:

- enum over trait object
- owned task over shared mutable manager
- pure parser function over parser object
- typed command over stringly event
- `watch` snapshot over repeated shared lock reads
- explicit cancellation over boolean lifecycle flags
