import { defineConfig } from '@playwright/test';

const PORT = 3000;

export default defineConfig({
	testDir: './tests',
	fullyParallel: true,
	reporter: [['list'], ['html', { open: 'never', outputFolder: 'playwright-report' }]],
	use: {
		baseURL: `http://127.0.0.1:${PORT}`,
		trace: 'retain-on-failure'
	},
	globalSetup: './global-setup.ts',
	webServer: {
		command: '../target/release/benefrance-backend',
		port: PORT,
		reuseExistingServer: false,
		timeout: 30_000,
		env: {
			DATABASE_URL:
				process.env.E2E_DATABASE_URL ?? 'postgres://postgres:postgres@127.0.0.1:55433/benefrance_e2e',
			LISTEN_ADDR: `0.0.0.0:${PORT}`,
			SMTP_HOST: process.env.E2E_SMTP_HOST ?? '127.0.0.1',
			SMTP_PORT: process.env.E2E_SMTP_PORT ?? '11025',
			SMTP_FROM: 'benefrance@example.org',
			SESSION_COOKIE_SECURE: 'false',
			// Serves the real built SPA (technical-specs.md §3) so a
			// page.goto('/mapping') reaches the actual page, not a 404.
			STATIC_DIR: '../frontend/build'
		}
	}
});
