import { expect, test } from '@playwright/test';
import { Client } from 'pg';
import { readLatestOtpCode } from './helpers/mailpit';

const DATABASE_URL =
	process.env.E2E_DATABASE_URL ?? 'postgres://postgres:postgres@127.0.0.1:55433/benefrance_e2e';

const LOGOUT_TEST_ADMIN_EMAIL = 'logout-e2e-admin@example.org';

async function seedAdmin(): Promise<void> {
	const client = new Client({ connectionString: DATABASE_URL });
	await client.connect();
	try {
		await client.query(
			`INSERT INTO admins (email, is_superadmin) VALUES ($1, true)
			 ON CONFLICT (email) DO NOTHING`,
			[LOGOUT_TEST_ADMIN_EMAIL]
		);
	} finally {
		await client.end();
	}
}

test('an admin logs out through the topbar and loses access to admin screens', async ({
	page
}) => {
	await seedAdmin();

	await page.request.post('/api/auth/request-code', {
		data: { email: LOGOUT_TEST_ADMIN_EMAIL }
	});
	const code = await readLatestOtpCode(page.request, LOGOUT_TEST_ADMIN_EMAIL);
	await page.request.post('/api/auth/verify-code', {
		data: { email: LOGOUT_TEST_ADMIN_EMAIL, code }
	});

	await page.goto('/evenements');
	await expect(page.getByText(LOGOUT_TEST_ADMIN_EMAIL)).toBeVisible();

	await page.getByRole('button', { name: 'Déconnexion' }).click();

	await expect(page).toHaveURL(/\/connexion$/);

	// The session is gone server-side too, not just the client-side cookie.
	await page.goto('/evenements');
	await expect(page).toHaveURL(/\/connexion$/);
});
