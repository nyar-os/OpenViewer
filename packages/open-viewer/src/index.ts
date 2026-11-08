/**
 * Public Node API for OpenViewer.
 *
 * Every function forwards to the N-API binding. No image parsing or thumbnail
 * generation is implemented in TypeScript.
 */
export * from './native.js';

import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import {
    native,
    type AuditEntry,
    type AuditFilter,
    type AuditOptions,
    type CapabilityReport,
    type DecodedRgba,
    type GuiOptions,
    type ImageInfo,
    type ImagePreview,
    type InstallOptions,
    type InstallPlan,
    type ThumbnailResult,
    type UninstallOptions,
    type UninstallPlan,
} from './native.js';

const here = path.dirname(fileURLToPath(import.meta.url));

/** Default staged WebView content folder shipped with this package. */
export const defaultGuiDir = path.resolve(here, 'gui');

/** Native binding version. */
export function version(): string {
    return native.version();
}

/** Platform capability report with the shared three-state vocabulary. */
export function capabilities(): CapabilityReport {
    return native.capabilities();
}

/** Read image metadata without decoding pixels. */
export function inspectImage(options: { path: string }): ImageInfo {
    return native.info(options);
}

/** Decode an image to base64 RGBA8. */
export function decodeImage(options: { path: string }): DecodedRgba {
    return native.decodeRgba(options);
}

/** Render a PNG preview for the desktop window. */
export function previewImage(options: { path: string; maxEdge?: number }): ImagePreview {
    return native.preview(options);
}

/** Render a thumbnail, optionally writing it as PNG. */
export function writeThumbnail(options: {
    path: string;
    out?: string;
    maxEdge?: number;
    dryRun?: boolean;
}): ThumbnailResult {
    return native.writeThumbnail(options);
}

/** Plan an install or upgrade without touching the host. */
export function installPlan(options: InstallOptions = {}): InstallPlan {
    return native.installPlan(options);
}

/** Apply an install or upgrade. Unsupported stages throw a capability error. */
export function installApply(options: InstallOptions = {}): void {
    native.installApply(options);
}

/** Plan an uninstall without touching the host. */
export function uninstallPlan(options: UninstallOptions = {}): UninstallPlan {
    return native.uninstallPlan(options);
}

/** Apply an uninstall. Unsupported stages throw a capability error. */
export function uninstallApply(options: UninstallOptions = {}): void {
    native.uninstallApply(options);
}

/** Attribute subsequent capability calls to an explicit actor. */
export function configureAudit(options: AuditOptions): void {
    native.configureAudit(options);
}

/** Query the shared audit trail. */
export function auditQuery(filter: AuditFilter = {}): AuditEntry[] {
    return native.auditQuery(filter);
}

const FRAME_MS = 16;

function delay(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Read `window.policy.secondLaunch` from the contract `wae build` writes next
 * to the CLI entry. A missing or unreadable contract means the default policy:
 * focus the existing window instead of opening a second one.
 */
function secondLaunchFromContract(): 'focus' | 'reject' {
    try {
        const value: unknown = JSON.parse(readFileSync(path.join(here, 'wae-runtime.json'), 'utf8'));
        if (typeof value !== 'object' || value === null || Array.isArray(value)) return 'focus';
        const window = (value as { window?: unknown }).window;
        if (typeof window !== 'object' || window === null) return 'focus';
        const policy = (window as { policy?: unknown }).policy;
        if (typeof policy !== 'object' || policy === null) return 'focus';
        return (policy as { secondLaunch?: unknown }).secondLaunch === 'reject' ? 'reject' : 'focus';
    }
    catch {
        return 'focus';
    }
}

/** Run the desktop viewer until the user closes the window. */
export async function runGui(options: GuiOptions = {}): Promise<void> {
    const contentDir = options.contentDir ?? (existsSync(path.join(defaultGuiDir, 'index.html')) ? defaultGuiDir : undefined);
    if (!contentDir && !options.url) {
        throw new Error(
            `GUI content was not found at ${defaultGuiDir}. Run the UI build or pass --content-dir <path>.`,
        );
    }
    native.openGui({
        title: options.title ?? 'OpenViewer',
        contentDir,
        url: options.url,
        width: options.width ?? 1280,
        height: options.height ?? 860,
        secondLaunch: options.secondLaunch ?? secondLaunchFromContract(),
    });
    try {
        while (native.guiStatus()) {
            if (!native.pumpGui()) break;
            await delay(FRAME_MS);
        }
    }
    finally {
        if (native.guiStatus()) native.closeGui();
    }
}
