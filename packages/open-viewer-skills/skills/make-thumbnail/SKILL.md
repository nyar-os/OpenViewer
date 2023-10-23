---
name: make-thumbnail
description: Render a PNG thumbnail from one local image with OpenViewer, preview the result first, and write only the requested output file.
---

# Make a PNG thumbnail

Use the installed `open-viewer` CLI. Decoding, scaling and PNG encoding happen
inside the native capability surface, not in the agent.

## Workflow

1. Confirm the source with `open-viewer info <path> --json`.
2. Preview the exact size with `open-viewer thumbnail <path> --dry-run --max <n>`.
   Report the returned width, height and byte count.
3. Ask for or confirm the destination, then run
   `open-viewer thumbnail <path> --out <path> --max <n>`.
4. Report the written path, final dimensions and bytes from the command output.
5. On exit code 3, report the capability refusal and its stable code instead of
   trying another external converter.

## Rules

- Thumbnails never upscale. A small source keeps its original dimensions.
- The output is always PNG.
- The default maximum edge is 1024. Accept only 1-8192.
- A thumbnail write must never target the source path.

## Safety

- Write only the explicit `--out` destination. Do not overwrite the source.
- Do not batch-convert directories unless the user explicitly asks for each
  file and destination.
- Do not invent dimensions or success. Use the command's returned values.
- If the destination exists and the user did not approve replacing it, stop and
  ask.
