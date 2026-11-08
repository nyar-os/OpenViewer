/**
 * Build and release configuration for OpenViewer.
 *
 * Pure literals only: `wae` parses this file statically and never executes it.
 * Build order is fixed: native → ts → ui → skills → staging.
 */
export default {
    native: {
        crate: 'open-viewer-napi',
        cdylib: 'open_viewer_napi',
        targets: [
            {
                platform: 'win32',
                arch: 'x64',
                triple: 'x86_64-pc-windows-msvc',
                package: '@wae/open-viewer-win32-x64',
                nodeFile: 'open-viewer.win32-x64.node',
                dir: 'packages/open-viewer-win32-x64',
            },
        ],
    },
    packages: [
        {
            name: '@wae/open-viewer-win32-x64',
            dir: 'packages/open-viewer-win32-x64',
            os: ['win32'],
            cpu: ['x64'],
            main: 'open-viewer.win32-x64.node',
        },
    ],
    build: {
        ts: { packages: ['@wae/open-viewer'] },
        ui: { package: '@wae/open-viewer-ui' },
        stage: [
            { from: 'packages/open-viewer-ui/dist/static', to: 'packages/open-viewer/dist/gui' },
        ],
        outputs: ['packages/open-viewer/dist/cli.js', 'packages/open-viewer/dist/gui/index.html'],
    },
    skills: {
        packages: [
            {
                dir: 'packages/open-viewer-skills/skills',
                packageJson: 'packages/open-viewer-skills/package.json',
                appPackageJson: 'packages/open-viewer/package.json',
                cliEntry: 'packages/open-viewer/dist/cli.js',
                commandName: 'open-viewer',
            },
        ],
    },
    release: {
        registry: 'https://registry.npmjs.org',
        tagPattern: '^v(\\d+\\.\\d+\\.\\d+)$',
        packages: [
            { name: '@wae/open-viewer', dir: 'packages/open-viewer' },
            { name: '@wae/open-viewer-skills', dir: 'packages/open-viewer-skills' },
            { name: '@wae/open-viewer-win32-x64', dir: 'packages/open-viewer-win32-x64' },
        ],
    },
    ci: {
        repo: 'nyar-os/OpenViewer',
        workflows: ['check-rust.yml', 'check-typescript.yml', 'publish-npm.yml'],
        requiredJobs: ['check-rust', 'check-typescript'],
    },
    install: {
        shortcutDefault: true,
    },
    audit: {
        actor: { kind: 'flag', id: 'flag', source: 'flag' },
    },
};
