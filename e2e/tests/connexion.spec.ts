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

// The code form is 6 single-digit boxes (mockups/wireframes.html screen s0),
// not one text input — fills each one in turn.
async function fillCode(page: import('@playwright/test').Page, code: string) {
	for (let i = 0; i < code.length; i++) {
		await page.getByLabel(`Chiffre ${i + 1} du code`).fill(code[i]);
	}
}

test('an admin logs in through the /connexion page and reaches an admin screen', async ({
	page,
	request
}) => {
	await seedAdmin(CONNEXION_TEST_ADMIN_EMAIL);

	await page.goto('/connexion');
	await page.getByPlaceholder('prenom.nom@exemple.fr').fill(CONNEXION_TEST_ADMIN_EMAIL);
	await page.getByRole('button', { name: 'Recevoir le code' }).click();

	await expect(page.getByLabel('Chiffre 1 du code')).toBeVisible();

	const code = await readLatestOtpCode(request, CONNEXION_TEST_ADMIN_EMAIL);
	await fillCode(page, code);
	await page.getByRole('button', { name: 'Se connecter' }).click();

	await expect(page).toHaveURL(/\/mapping$/);
	await expect(page.getByRole('heading', { name: 'Admin — mapping form' })).toBeVisible();
});

test('a wrong code shows an inline error instead of redirecting to /connexion', async ({
	page
}) => {
	await seedAdmin(WRONG_CODE_TEST_ADMIN_EMAIL);

	await page.goto('/connexion');
	await page.getByPlaceholder('prenom.nom@exemple.fr').fill(WRONG_CODE_TEST_ADMIN_EMAIL);
	await page.getByRole('button', { name: 'Recevoir le code' }).click();

	await fillCode(page, '000000');
	await page.getByRole('button', { name: 'Se connecter' }).click();

	await expect(page.getByRole('alert')).toHaveText('Code invalide ou expiré.');
	await expect(page).toHaveURL(/\/connexion$/);
});
