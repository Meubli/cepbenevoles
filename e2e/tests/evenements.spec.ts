import { expect, Page, test } from '@playwright/test';
import { Client } from 'pg';
import { readLatestOtpCode } from './helpers/mailpit';

const DATABASE_URL =
	process.env.E2E_DATABASE_URL ?? 'postgres://postgres:postgres@127.0.0.1:55433/benefrance_e2e';

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

// Every test needs its own admin email: tests in this file run in parallel
// (fullyParallel) and readLatestOtpCode picks the newest email to a given
// address, a real race if two tests share one (cf. connexion.spec.ts).
async function loginAs(page: Page, email: string): Promise<void> {
	await seedAdmin(email);
	await page.request.post('/api/auth/request-code', { data: { email } });
	const code = await readLatestOtpCode(page.request, email);
	await page.request.post('/api/auth/verify-code', { data: { email, code } });
}

test('an admin creates an event, then archives and unarchives it', async ({ page }) => {
	await loginAs(page, 'evenements-e2e-admin@example.org');

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

test('entering a sheet_id reveals the bloc selector, and the created event keeps it', async ({
	page
}) => {
	await loginAs(page, 'evenements-bloc-e2e-admin@example.org');

	await page.goto('/evenements');

	const nom = `E2E bloc event ${Date.now()}`;
	const sheetId = `e2e-sheet-${Date.now()}`;
	await page.getByRole('button', { name: '+ Nouvel événement' }).click();
	await page.getByLabel('Nom').fill(nom);
	await page.getByLabel('Date début').fill('2027-04-29');
	await page.getByLabel('Date fin').fill('2027-05-04');

	// No sheet_id yet: the bloc selector isn't shown at all.
	await expect(page.getByLabel('Bloc du formulaire')).toBeHidden();

	await page.getByLabel('ID Google Sheet source').fill(sheetId);
	await expect(page.getByLabel('Bloc du formulaire')).toBeVisible();
	await page
		.getByLabel('Bloc du formulaire')
		.selectOption({ label: 'France Indiv Perf GAM/GAF' });

	await page.getByRole('button', { name: "Créer l'événement" }).click();

	const row = page.locator('tr', { hasText: nom });
	await expect(row).toContainText(sheetId);
});

test('a sheet_id without a chosen bloc is rejected with an inline error', async ({ page }) => {
	await loginAs(page, 'evenements-no-bloc-e2e-admin@example.org');

	await page.goto('/evenements');

	const nom = `E2E no-bloc event ${Date.now()}`;
	await page.getByRole('button', { name: '+ Nouvel événement' }).click();
	await page.getByLabel('Nom').fill(nom);
	await page.getByLabel('Date début').fill('2027-04-29');
	await page.getByLabel('Date fin').fill('2027-05-04');
	await page.getByLabel('ID Google Sheet source').fill(`e2e-sheet-${Date.now()}`);
	// Bloc selector is shown but deliberately left on its placeholder option.
	await page.getByRole('button', { name: "Créer l'événement" }).click();

	await expect(page.getByRole('alert')).toContainText(
		'un sheet_id est renseigné : choisissez le bloc du formulaire'
	);
	// The form must still be open with its values, not silently discarded.
	await expect(page.getByLabel('Nom')).toHaveValue(nom);
});
