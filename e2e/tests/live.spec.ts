import { expect, Page, test } from '@playwright/test';
import { Client } from 'pg';
import { readLatestOtpCode } from './helpers/mailpit';
import { openTwoAdminContexts } from './helpers/multi-admin';

const DATABASE_URL =
	process.env.E2E_DATABASE_URL ?? 'postgres://postgres:postgres@127.0.0.1:55433/benefrance_e2e';

async function seedAdmin(email: string): Promise<void> {
	const client = new Client({ connectionString: DATABASE_URL });
	await client.connect();
	try {
		await client.query(
			`INSERT INTO admins (nom, prenom, email) VALUES ('Live', 'E2E', $1)
			 ON CONFLICT (email) DO NOTHING`,
			[email]
		);
	} finally {
		await client.end();
	}
}

async function loginAs(page: Page, email: string): Promise<void> {
	await page.request.post('/api/auth/request-code', { data: { email } });
	const code = await readLatestOtpCode(page.request, email);
	await page.request.post('/api/auth/verify-code', { data: { email, code } });
}

// Exercises the live-refresh websocket (technical-specs.md §10): a change
// made in one browser context must appear in another, already-open context
// without a reload — the scenario `openTwoAdminContexts` was written for.
test('a commission created by one admin appears live for another, already-open admin', async ({
	browser
}) => {
	const emailA = `live-e2e-admin-a-${Date.now()}@example.org`;
	const emailB = `live-e2e-admin-b-${Date.now()}@example.org`;
	await seedAdmin(emailA);
	await seedAdmin(emailB);

	const [contextA, contextB] = await openTwoAdminContexts(browser);
	const pageA = await contextA.newPage();
	const pageB = await contextB.newPage();

	await loginAs(pageA, emailA);
	await loginAs(pageB, emailB);

	await pageA.goto('/commissions');
	await pageB.goto('/commissions');

	const commissionName = `Live e2e ${Date.now()}`;

	await pageA.getByRole('button', { name: '+ Nouvelle commission' }).click();
	await pageA.getByLabel('Nom', { exact: true }).fill(commissionName);
	await pageA.getByRole('button', { name: 'Créer la commission' }).click();

	// pageB never reloads or re-navigates — only the live-refresh websocket
	// can make this row appear.
	await expect(
		pageB.locator('[data-testid="commission-row"]', { hasText: commissionName })
	).toBeVisible({ timeout: 5000 });

	await Promise.all([contextA.close(), contextB.close()]);
});
