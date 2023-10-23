/**
 * Native binding loader for `@wae/open-viewer`.
 *
 * The N-API module is the only implementation path: CLI, window and agent
 * skills all call these functions. Platform packages are optional dependencies;
 * a missing binary produces an actionable error instead of a silent fallback.
 */
import { createRequire } from 'node:module';
import os from 'node:os';

const require = createRequire(import.meta.url);

export type CapabilityStateName = 'ready' | 'unsupportedPlatform' | 'notImplemented' | 'dependencyBlocked';

export interface CapabilityState {
    state: CapabilityStateName;
    code?: string;
    detail?: string;
    platform?: string;
}

export interface CapabilityEntry {
    id: string;
    title: string;
    state: CapabilityState;
    formats?: string[];
}

export interface CapabilityReport {
    schema: string;
    app: string;
    version: string;
    platform: string;
    entries: CapabilityEntry[];
}

export interface ImageInfo {
    path: string;
    format: string;
    width: number;
    height: number;
    pixelCount: number;
    fileSize: number;
    colorType: string;
    bitsPerPixel: number;
    hasAlpha: boolean;
    frames: number | null;
    animated: boolean;
    withinPixelLimit: boolean;
}

export interface DecodedRgba {
    width: number;
    height: number;
    bytes: number;
    rgbaBase64: string;
}

export interface ImagePreview {
    info: ImageInfo;
    sourceWidth: number;
    sourceHeight: number;
    width: number;
    height: number;
    pngBase64: string;
    dataUrl: string;
}

export interface ThumbnailWrite {
    dryRun?: false;
    path: string;
    width: number;
    height: number;
    sourceWidth: number;
    sourceHeight: number;
    bytes: number;
}

export interface ThumbnailDryRun {
    dryRun: true;
    width: number;
    height: number;
    sourceWidth: number;
    sourceHeight: number;
    bytes: number;
}

export type ThumbnailResult = ThumbnailWrite | ThumbnailDryRun;

export interface InstallOptions {
    noShortcut?: boolean;
    resetConfig?: boolean;
}

export interface UninstallOptions {
    keepConfig?: boolean;
}

export interface PlannedAction {
    kind:
        | 'write_config'
        | 'reuse_config'
        | 'reset_config'
        | 'create_shortcut'
        | 'write_marker'
        | 'remove_shortcut'
        | 'remove_config'
        | 'keep_config'
        | 'remove_marker';
    target: string;
    detail: string;
}

export interface InstallPlan {
    app: string;
    version: string;
    platform: string;
    configDir: string;
    configPath: string;
    markerPath: string;
    shortcutPath: string | null;
    installed: boolean;
    installedVersion: string | null;
    upgrade: boolean;
    requiresElevation: boolean;
    autoStart: boolean;
    actions: PlannedAction[];
}

export interface UninstallPlan {
    app: string;
    version: string;
    platform: string;
    configPath: string;
    shortcutPath: string | null;
    markerPath: string;
    configExists: boolean;
    shortcutExists: boolean;
    requiresElevation: boolean;
    actions: PlannedAction[];
}

export type ActorKind = 'user' | 'agent';

export interface AuditOptions {
    actorId?: string;
    actorKind?: ActorKind;
    actorSource?: string;
    session?: string;
    host?: string;
}

export interface AuditFilter {
    kind?: ActorKind;
    id?: string;
    capability?: string;
    fromMs?: number;
    toMs?: number;
    limit?: number;
}

export interface AuditEntry {
    schema: string;
    timestampMs: number;
    durationMs: number;
    actor: { kind: ActorKind; id: string; source: string };
    host: string | null;
    session: string | null;
    app: string;
    appVersion: string;
    capability: string;
    mode: 'dry_run' | 'execute';
    params: unknown;
    targets: { count: number; summary: string | null };
    result: 'success' | 'failure' | 'rejected';
    errorCode: string | null;
}

export interface GuiOptions {
    title?: string;
    contentDir?: string;
    url?: string;
    width?: number;
    height?: number;
}

export interface NativeBinding {
    version(): string;
    capabilities(): CapabilityReport;
    info(options: { path: string }): ImageInfo;
    decodeRgba(options: { path: string }): DecodedRgba;
    preview(options: { path: string; maxEdge?: number }): ImagePreview;
    writeThumbnail(options: { path: string; out?: string; maxEdge?: number; dryRun?: boolean }): ThumbnailResult;
    installPlan(options?: InstallOptions): InstallPlan;
    installApply(options?: InstallOptions): void;
    uninstallPlan(options?: UninstallOptions): UninstallPlan;
    uninstallApply(options?: UninstallOptions): void;
    configureAudit(options: AuditOptions): void;
    auditQuery(filter?: AuditFilter): AuditEntry[];
    openGui(options: GuiOptions): void;
    pumpGui(): boolean;
    closeGui(): void;
    guiStatus(): boolean;
}

const platformPackages = ['@wae/open-viewer-win32-x64/open-viewer.win32-x64.node'];

export function nativeCandidates(): string[] {
    const override = process.env.OPEN_VIEWER_NATIVE;
    return override ? [override, ...platformPackages] : [...platformPackages];
}

function load(): NativeBinding {
    const errors: string[] = [];
    for (const candidate of nativeCandidates()) {
        try {
            return require(candidate) as NativeBinding;
        }
        catch (error) {
            errors.push(`${candidate}: ${error instanceof Error ? error.message : String(error)}`);
        }
    }
    throw new Error(
        `OpenViewer native binding is not available for ${os.platform()}-${os.arch()}.\n` +
            `Set OPEN_VIEWER_NATIVE to a built .node file, or install a supported platform package.\n` +
            `Tried:\n${errors.map((line) => `  - ${line}`).join('\n')}`,
    );
}

export const native = load();
