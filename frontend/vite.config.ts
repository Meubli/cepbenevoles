import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			adapter: adapter({ fallback: 'index.html' })
		})
	],
	server: {
		// Local dev only — in prod the same Axum binary serves both (cf.
		// docs/technical-specs.md §3), no proxy involved.
		proxy: {
			// `ws: true` is required explicitly — the string shorthand form
			// doesn't proxy websocket upgrades, only plain HTTP (needed for
			// /api/ws, the live-refresh socket, technical-specs.md §10).
			'/api': { target: 'http://127.0.0.1:3000', ws: true }
		}
	}
});
