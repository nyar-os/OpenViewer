# OpenViewer

A local image viewer: inspect format, dimensions, colour type and frame metadata,
decode pixels, and render PNG previews or thumbnails without leaving the process.
It reads images; it does not edit, batch re-encode or delete them.

A WebView2 window ships alongside a command line and a Node API. Both consume the
same native capability surface.

## Layout

| Path | Contents |
|---|---|
| `crates/core` | Image domain model, decoding, metadata, thumbnails, capability states and install plans |
| `crates/napi` | N-API transport, window lifecycle and IPC |
| `packages/open-viewer` | Node API and `open-viewer` CLI with `en-US` and `zh-CN` text |
| `packages/open-viewer-ui` | VMZ static interface for the desktop window |
| `packages/open-viewer-skills` | Agent skills written against the published CLI |
| `packages/open-viewer-win32-x64` | Staged `.node` addon for Windows x64 |
| `scripts` | Native build, staging and skill validation |

## Install

```bash
npm install -g @wae/open-viewer
```

The addon is published for Windows x64 only. Other platforms have no binary and
are not claimed as supported.

## Commands

| Command | Purpose |
|---|---|
| `open-viewer info <path>` | Print format, dimensions, colour type and file size |
| `open-viewer thumbnail <path> --out <path>` | Render a PNG thumbnail (`--dry-run` previews, `--max` sets the edge limit) |
| `open-viewer capabilities` | List capability states for this build |
| `open-viewer audit` | Query the append-only audit log |
| `open-viewer install` | Plan an install or upgrade (`--apply` currently reports `not_implemented`) |
| `open-viewer uninstall` | Plan a removal (`--apply` currently reports `not_implemented`) |
| `open-viewer gui` | Open the desktop viewer window |
| `open-viewer version` | Print the package version |

Exit codes: `0` success, `1` runtime failure, `2` usage error, `3` capability
unavailable.

## Capability states

| Capability | State |
|---|---|
| `image-decode` | ready for `bmp`, `gif`, `ico`, `jpeg`, `png`, `pnm`, `tga`, `tiff`, `webp` |
| `image-metadata` | ready |
| `image-thumbnail` | ready, PNG output, never upscales |
| `image-preview` | ready |
| `image-animation-playback` | `not_implemented` |
| `image-exif` | `not_implemented` |
| `image-color-management` | `not_implemented` |
| `image-raw` | `dependency_blocked` |
| `image-heic` | `dependency_blocked` |
| `gui-host` | ready on Windows, `unsupported_platform` elsewhere |
| `install-plan` | ready |
| `install-apply` | `not_implemented` |
| `uninstall-apply` | `not_implemented` |

Refusals carry a stable code and are never silently degraded into a success.
Decoding is refused above 100,000,000 pixels.

## Decoding in process

Parsing, scaling and PNG encoding are library calls inside the addon. OpenViewer
does not invoke external binaries or shell pipelines for image work. A capability
whose library is unavailable is reported as `dependency_blocked` instead.

## Audit

Every capability call is recorded with the shared `wae.audit` schema, including
the actor id, actor kind (`user` or `agent`) and entry point. CLI, window and
skills write to the same trail.

## Development

```bash
pnpm install
pnpm run build:napi   # cargo build + stage the .node
pnpm run build:ts     # CLI and Node API
pnpm run build:ui     # static interface, staged into dist/gui
pnpm run check:skills # SKILL.md commands and flags against the real CLI
pnpm run check:rust   # fmt, clippy and tests
```

`pnpm run gui` builds everything and opens the desktop window.

## Platform claims

Only `win32-x64` is built and smoke-tested. macOS, Linux and Arm hosts are
unsupported, not unverified-but-working. Format support is listed above; any
format not listed is not claimed.

## License

See [`License.md`](./License.md). Cargo and npm manifests in this repository all
declare `Apache-2.0`.
