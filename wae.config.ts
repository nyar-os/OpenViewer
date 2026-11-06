/**
 * Build and release configuration for OpenViewer.
 *
 * This file is pure data so it can be parsed statically. `wae build` reads it
 * to order native → TypeScript → UI → skills validation → staging.
 */
export default {
    native: {
        crate: 'open-viewer-napi',
        target: 'win32-x64',
        cdylib: 'open_viewer_napi',
        artifact: 'open-viewer.win32-x64.node',
        platformPackage: '@wae/open-viewer-win32-x64',
    },
    packages: {
        native: {
            dir: 'packages/open-viewer-win32-x64',
            os: ['win32'],
            cpu: ['x64'],
        },
    },
    build: {
        order: ['native', 'typescript', 'ui', 'skills'],
    },
    skills: {
        dir: 'packages/open-viewer-skills/skills',
        check: 'scripts/check-skills.ts',
    },
    release: {
        packages: ['@wae/open-viewer', '@wae/open-viewer-skills', '@wae/open-viewer-win32-x64'],
        tag: 'vX.Y.Z',
    },
    ci: {
        workflows: ['check-rust.yml', 'check-typescript.yml'],
    },
};
