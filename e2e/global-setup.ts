import { Client } from 'pg';

export const TEST_ADMIN_EMAIL = 'admin@example.org';

const DATABASE_URL =
	process.env.E2E_DATABASE_URL ?? 'postgres://postgres:postgres@127.0.0.1:55433/benefrance_e2e';

/**
 * Seeds one known admin row so the OTP login smoke test has an email that
 * resolves. The `admins` table itself comes from the backend's own
 * migrations (run by the Axum binary at startup, cf. docs/technical-specs.md
 * §2), not from this script — so this retries until that table exists
 * rather than racing the webServer startup.
 */
async function waitForAdminsTableAndSeed(): Promise<void> {
	const deadline = Date.now() + 30_000;

	while (Date.now() < deadline) {
		const client = new Client({ connectionString: DATABASE_URL });
		try {
			await client.connect();
			await client.query(
				`INSERT INTO admins (email, is_superadmin) VALUES ($1, true)
				 ON CONFLICT (email) DO NOTHING`,
				[TEST_ADMIN_EMAIL]
			);
			await client.end();
			return;
		} catch {
			await client.end().catch(() => {});
			await new Promise((resolve) => setTimeout(resolve, 500));
		}
	}

	throw new Error('timed out waiting for the admins table to be migrated');
}

export default async function globalSetup(): Promise<void> {
	await waitForAdminsTableAndSeed();
}
