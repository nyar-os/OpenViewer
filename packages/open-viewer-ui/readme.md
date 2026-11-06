# `@wae/open-viewer-ui`

WebView2 interface for OpenViewer, implemented with [`@vmz/ui`](https://www.npmjs.com/package/@vmz/ui).

## Develop

```bash
pnpm install
pnpm --filter @wae/open-viewer-ui run check
pnpm --filter @wae/open-viewer-ui run build
pnpm --filter @wae/open-viewer-ui run stage
```

`stage` copies `dist/static` into `packages/open-viewer/dist/gui` for the native WebView2 host.

```bash
pnpm run gui
```

The page talks to the native host only through `chrome.webview` messages. Outside the host it reports that the native bridge is unavailable and shows no synthetic image data.
