---
name: inspect-image
description: Inspect one local image with OpenViewer and report format, dimensions, colour type, frame metadata and file size without decoding pixels in the agent.
---

# Inspect a local image

Use the installed `open-viewer` CLI for all image facts. Do not decode or
reimplement image parsing in the agent.

## Workflow

1. Run `open-viewer info <path> --json` for the requested file.
2. Report `format`, `width`, `height`, `pixelCount`, `fileSize`, `colorType`,
   `bitsPerPixel`, `hasAlpha`, `frames` and `animated` exactly as returned.
3. Treat `frames` as `null` when the container does not expose a cheap frame
   count. Do not guess a frame count.
4. If the command exits with code 3, read the capability state first. Run
   `open-viewer capabilities --json` and report the matching entry and its
   `code` (`unsupported_platform`, `not_implemented` or `dependency_blocked`).
5. If `withinPixelLimit` is false, explain that decode and thumbnails are
   refused for that image and report the pixel ceiling.

## Reporting

State what was verified from the command output and what was refused. Never
describe an unverified format or platform as supported.

## Safety

- This workflow is read-only.
- Do not modify, re-encode or delete the source image.
- Do not claim metadata that the CLI did not return.
- Keep paths as provided by the user. Do not infer alternate locations.
