import { expect, test } from '@playwright/test';
import { Client } from 'pg';
import { readLatestOtpCode } from './helpers/mailpit';

const DATABASE_URL =
	process.env.E2E_DATABASE_URL ?? 'postgres://postgres:postgres@127.0.0.1:55433/benefrance_e2e';

// Distinct admin email — sharing one with otp-login/mapping specs running
// in parallel races on Mailpit's "latest message" lookup (cf. mapping.spec.ts).
const EVENEMENTS_TEST_ADMIN_EMAIL = 'evenements-e2e-admin@example.org';

async function seedAdmin(): Promise<void> {
	const client = new Client({ connectionString: DATABASE_URL });
	await client.connect();
	try {
		await client.query(
			`INSERT INTO admins (email, is_superadmin) VALUES ($1, true)
			 ON CONFLICT (email) DO NOTHING`,
			[EVENEMENTS_TEST_ADMIN_EMAIL]
		);
	} finally {
		await client.end();
	}
}

test('an admin creates an event, then archives and unarchives it', async ({ page }) => {
	await seedAdmin();

	await page.request.post('/api/auth/request-code', {
		data: { email: EVENEMENTS_TEST_ADMIN_EMAIL }
	});
	const code = await readLatestOtpCode(page.request, EVENEMENTS_TEST_ADMIN_EMAIL);
	await page.request.post('/api/auth/verify-code', {
		data: { email: EVENEMENTS_TEST_ADMIN_EMAIL, code }
	});

	await page.goto('/evenements');

	const nom = `E2E event ${Date.now()}`;
	await page.getByRole('button', { name: '+ Nouvel événement' }).click();
	await page.getByLabel('Nom').fill(nom);
	await page.getByLabel('Date début').fill('2027-04-29');
	await page.getByLabel('Date fin').fill('2027-05-04');
	await page.getByRole('button', { name: "Créer l'événement" }).click();

	const row = page.locator('tr', { hasText: nom });
	await expect(row).toContainText('actif');

	await row.getByRole('button', { name: 'Archiver' }).click();
	await expect(row).toContainText('archivé');

	await row.getByRole('button', { name: 'Désarchiver' }).click();
	await expect(row).toContainText('actif');
});
