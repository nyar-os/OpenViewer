#!/usr/bin/env node
/**
 * OpenViewer CLI — in-process image metadata, previews and thumbnails.
 *
 * Exit codes are stable for automation:
 * - 0: success
 * - 1: runtime failure
 * - 2: usage / argument failure
 * - 3: capability unavailable (`unsupported_platform`, `not_implemented`, `dependency_blocked`)
 */
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createCli, type ParsedOptions } from '@wae/commander';
import {
    auditQuery,
    capabilities,
    configureAudit,
    installApply,
    installPlan,
    inspectImage,
    runGui,
    uninstallApply,
    uninstallPlan,
    version,
    writeThumbnail,
} from './index.js';
import type { ActorKind, CapabilityReport, ImageInfo, InstallPlan, PlannedAction, UninstallPlan } from './index.js';

const EXIT_OK = 0;
const EXIT_FAILURE = 1;
const EXIT_USAGE = 2;
const EXIT_CAPABILITY = 3;

const CAPABILITY_CODES = ['unsupported_platform', 'not_implemented', 'dependency_blocked'];

const here = path.dirname(fileURLToPath(import.meta.url));
const localesDir = path.join(here, '..', 'locales');

const cli = createCli('open-viewer')
    .locales(localesDir)
    .intro('cli.intro')
    .option('--actor <id>', 'cli.opt.actor')
    .option('--actor-kind <kind>', 'cli.opt.actorKind')
    .option('--actor-source <source>', 'cli.opt.actorSource');

function text(options: ParsedOptions, key: string): string | undefined {
    const value = options[key];
    return typeof value === 'string' ? value : undefined;
}

/** Attribute every capability call in this run to an explicit actor. */
function applyActor(options: ParsedOptions): void {
    const kindRaw = text(options, 'actor-kind');
    const actorKind: ActorKind | undefined = kindRaw === 'user' || kindRaw === 'agent' ? kindRaw : undefined;
    configureAudit({
        actorId: text(options, 'actor'),
        actorKind,
        actorSource: text(options, 'actor-source') ?? 'cli',
    });
}

function errorText(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
}

function exitCodeFor(error: unknown): number {
    const message = errorText(error);
    if (CAPABILITY_CODES.some((code) => message.includes(code))) return EXIT_CAPABILITY;
    return EXIT_FAILURE;
}

function reportError(error: unknown): number {
    console.error(errorText(error));
    return exitCodeFor(error);
}

function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
    return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
}

function printImageInfo(info: ImageInfo): void {
    console.log(`path: ${info.path}`);
    console.log(`format: ${info.format}`);
    console.log(`dimensions: ${info.width}x${info.height} (${info.pixelCount} pixels)`);
    console.log(`size: ${formatBytes(info.fileSize)}`);
    console.log(`color: ${info.colorType}, ${info.bitsPerPixel} bits per pixel, alpha ${info.hasAlpha ? 'yes' : 'no'}`);
    if (info.frames != null) console.log(`frames: ${info.frames}${info.animated ? ' (animated)' : ''}`);
    if (!info.withinPixelLimit) console.log('warning: image exceeds the decode pixel ceiling; decode and thumbnails will be refused');
}

function printCapabilityReport(report: CapabilityReport): void {
    console.log(`platform: ${report.platform}`);
    for (const entry of report.entries) {
        const state = entry.state.state;
        const code = entry.state.code ? ` [${entry.state.code}]` : '';
        const detail = entry.state.detail ? ` — ${entry.state.detail}` : '';
        const formats = entry.formats && entry.formats.length > 0 ? ` (${entry.formats.join(', ')})` : '';
        console.log(`${entry.id}: ${state}${code}${formats}${detail}`);
    }
}

function printPlan(plan: InstallPlan | UninstallPlan, actions: PlannedAction[], title: string): void {
    console.log(`${title} ${plan.version} (${plan.platform})`);
    console.log(`config: ${plan.configPath}`);
    if (plan.shortcutPath) console.log(`launcher: ${plan.shortcutPath}`);
    console.log(`elevation required: ${plan.requiresElevation ? 'yes' : 'no'}`);
    if (actions.length === 0) {
        console.log('actions: none');
        return;
    }
    console.log('actions:');
    for (const action of actions.slice(0, 20)) console.log(`  ${action.kind}: ${action.target} (${action.detail})`);
    if (actions.length > 20) console.log(`  ... ${actions.length - 20} more`);
}

const maxOption = {
    coerce: (raw: string | boolean | string[]): unknown => {
        const value = Number(raw);
        if (!Number.isFinite(value) || value < 1 || value > 8192) throw new Error(`invalid --max: ${String(raw)}`);
        return String(Math.floor(value));
    },
};

const limitOption = {
    coerce: (raw: string | boolean | string[]): unknown => {
        const value = Number(raw);
        if (!Number.isFinite(value) || value < 1) throw new Error(`invalid --limit: ${String(raw)}`);
        return String(Math.floor(value));
    },
};

cli.command('info', 'cli.cmd.info')
    .option('--json', 'cli.opt.json')
    .actionCtx(async (options, args) => {
        applyActor(options);
        const imagePath = args[0];
        if (!imagePath) {
            console.error('usage: open-viewer info <path> [--json]');
            return EXIT_USAGE;
        }
        try {
            const info = inspectImage({ path: imagePath });
            if (options.json === true) console.log(JSON.stringify(info, null, 2));
            else printImageInfo(info);
            return EXIT_OK;
        }
        catch (error) {
            return reportError(error);
        }
    });

cli.command('thumbnail', 'cli.cmd.thumbnail')
    .option('--out <path>', 'cli.opt.out')
    .option('--max <n>', 'cli.opt.max', maxOption)
    .option('--dry-run', 'cli.opt.dryRun')
    .option('--json', 'cli.opt.json')
    .actionCtx(async (options, args) => {
        applyActor(options);
        const imagePath = args[0];
        if (!imagePath) {
            console.error('usage: open-viewer thumbnail <path> --out <path> [--max <n>] [--dry-run] [--json]');
            return EXIT_USAGE;
        }
        const out = text(options, 'out');
        const dryRun = options['dry-run'] === true;
        if (!out && !dryRun) {
            console.error('thumbnail requires --out <path> unless --dry-run is set');
            return EXIT_USAGE;
        }
        const maxRaw = text(options, 'max');
        try {
            const result = writeThumbnail({ path: imagePath, out, maxEdge: maxRaw ? Number(maxRaw) : undefined, dryRun });
            if (options.json === true) console.log(JSON.stringify(result, null, 2));
            else if ('dryRun' in result && result.dryRun) {
                console.log(`dry_run: would write ${result.width}x${result.height} (${result.bytes} bytes)`);
            }
            else console.log(`wrote ${result.path} (${result.width}x${result.height}, ${result.bytes} bytes)`);
            return EXIT_OK;
        }
        catch (error) {
            return reportError(error);
        }
    });

cli.command('capabilities', 'cli.cmd.capabilities')
    .option('--json', 'cli.opt.json')
    .actionCtx(async (options) => {
        applyActor(options);
        try {
            const report = capabilities();
            if (options.json === true) console.log(JSON.stringify(report, null, 2));
            else printCapabilityReport(report);
            return EXIT_OK;
        }
        catch (error) {
            return reportError(error);
        }
    });

cli.command('audit', 'cli.cmd.audit')
    .option('--filter-actor <id>', 'cli.opt.filterActor')
    .option('--filter-kind <kind>', 'cli.opt.filterKind')
    .option('--capability <name>', 'cli.opt.capability')
    .option('--limit <n>', 'cli.opt.auditLimit', limitOption)
    .actionCtx(async (options) => {
        applyActor(options);
        const limitRaw = text(options, 'limit');
        const kindRaw = text(options, 'filter-kind');
        const kind: ActorKind | undefined = kindRaw === 'user' || kindRaw === 'agent' ? kindRaw : undefined;
        try {
            const entries = auditQuery({
                id: text(options, 'filter-actor'),
                kind,
                capability: text(options, 'capability'),
                limit: limitRaw ? Number(limitRaw) : undefined,
            });
            for (const entry of entries) console.log(JSON.stringify(entry));
            return EXIT_OK;
        }
        catch (error) {
            return reportError(error);
        }
    });

cli.command('install', 'cli.cmd.install')
    .option('--apply', 'cli.opt.apply')
    .option('--no-shortcut', 'cli.opt.noShortcut')
    .option('--reset-config', 'cli.opt.resetConfig')
    .option('--json', 'cli.opt.json')
    .actionCtx(async (options) => {
        applyActor(options);
        const installOptions = {
            noShortcut: options['no-shortcut'] === true,
            resetConfig: options['reset-config'] === true,
        };
        try {
            const plan = installPlan(installOptions);
            if (options.json === true) console.log(JSON.stringify(plan, null, 2));
            else printPlan(plan, plan.actions, 'install plan for OpenViewer');
            if (options.apply === true) {
                installApply(installOptions);
                console.log('install applied');
            }
            return EXIT_OK;
        }
        catch (error) {
            return reportError(error);
        }
    });

cli.command('uninstall', 'cli.cmd.uninstall')
    .option('--apply', 'cli.opt.apply')
    .option('--keep-config', 'cli.opt.keepConfig')
    .option('--json', 'cli.opt.json')
    .actionCtx(async (options) => {
        applyActor(options);
        const uninstallOptions = { keepConfig: options['keep-config'] === true };
        try {
            const plan = uninstallPlan(uninstallOptions);
            if (options.json === true) console.log(JSON.stringify(plan, null, 2));
            else printPlan(plan, plan.actions, 'uninstall plan for OpenViewer');
            if (options.apply === true) {
                uninstallApply(uninstallOptions);
                console.log('uninstall applied');
            }
            return EXIT_OK;
        }
        catch (error) {
            return reportError(error);
        }
    });

cli.command('gui', 'cli.cmd.gui')
    .option('--title <text>', 'cli.opt.title', { default: 'OpenViewer' })
    .option('--content-dir <path>', 'cli.opt.contentDir')
    .actionCtx(async (options) => {
        applyActor(options);
        const title = typeof options.title === 'string' ? options.title : 'OpenViewer';
        try {
            await runGui({ title, contentDir: text(options, 'content-dir') });
            return EXIT_OK;
        }
        catch (error) {
            return reportError(error);
        }
    });

cli.command('version', 'cli.cmd.version').actionCtx(async (options) => {
    applyActor(options);
    try {
        console.log(version());
        return EXIT_OK;
    }
    catch (error) {
        return reportError(error);
    }
});

const code = await cli.parse(process.argv);
process.exitCode = code;
