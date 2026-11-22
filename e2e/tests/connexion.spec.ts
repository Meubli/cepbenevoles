import { expect, test } from '@playwright/test';
import { Client } from 'pg';
import { readLatestOtpCode } from './helpers/mailpit';

const DATABASE_URL =
	process.env.E2E_DATABASE_URL ?? 'postgres://postgres:postgres@127.0.0.1:55433/benefrance_e2e';

// Distinct from every other spec's admin email (cf. mapping.spec.ts comment):
// readLatestOtpCode picks the newest email to a given address, a real race
// across specs running in parallel. The two tests below also need distinct
// emails from each other for the same reason: `verify_code` checks the code
// against the latest unused challenge for that email, so one test
// requesting a code invalidates (as "latest") the other's in-flight one if
// they share an address.
const CONNEXION_TEST_ADMIN_EMAIL = 'connexion-e2e-admin@example.org';
const WRONG_CODE_TEST_ADMIN_EMAIL = 'connexion-wrong-code-e2e-admin@example.org';

async function seedAdmin(email: string): Promise<void> {
	const client = new Client({ connectionString: DATABASE_URL });
	await client.connect();
	try {
		await client.query(
			`INSERT INTO admins (email, is_superadmin) VALUES ($1, true)
			 ON CONFLICT (email) DO NOTHING`,
			[email]
		);
	} finally {
		await client.end();
	}
}

test('an admin logs in through the /connexion page and reaches an admin screen', async ({
	page,
	request
}) => {
	await seedAdmin(CONNEXION_TEST_ADMIN_EMAIL);

	await page.goto('/connexion');
	await page.getByLabel('Email').fill(CONNEXION_TEST_ADMIN_EMAIL);
	await page.getByRole('button', { name: 'Recevoir un code' }).click();

	await expect(page.getByLabel('Code reçu par email')).toBeVisible();

	const code = await readLatestOtpCode(request, CONNEXION_TEST_ADMIN_EMAIL);
	await page.getByLabel('Code reçu par email').fill(code);
	await page.getByRole('button', { name: 'Se connecter' }).click();

	await expect(page).toHaveURL(/\/mapping$/);
	await expect(page.getByRole('heading', { name: 'Admin — mapping form' })).toBeVisible();
});

test('a wrong code shows an inline error instead of redirecting to /connexion', async ({
	page
}) => {
	await seedAdmin(WRONG_CODE_TEST_ADMIN_EMAIL);

	await page.goto('/connexion');
	await page.getByLabel('Email').fill(WRONG_CODE_TEST_ADMIN_EMAIL);
	await page.getByRole('button', { name: 'Recevoir un code' }).click();

	await page.getByLabel('Code reçu par email').fill('000000');
	await page.getByRole('button', { name: 'Se connecter' }).click();

	await expect(page.getByRole('alert')).toHaveText('Code invalide ou expiré.');
	await expect(page).toHaveURL(/\/connexion$/);
});
