import { expect, Page, test } from '@playwright/test';
import { Client } from 'pg';
import { readLatestOtpCode } from './helpers/mailpit';

const DATABASE_URL =
	process.env.E2E_DATABASE_URL ?? 'postgres://postgres:postgres@127.0.0.1:55433/benefrance_e2e';

async function seedSuperadmin(email: string): Promise<void> {
	const client = new Client({ connectionString: DATABASE_URL });
	await client.connect();
	try {
		await client.query(
			`INSERT INTO admins (nom, prenom, email, is_superadmin) VALUES ('Chevalier', 'Inès', $1, true)
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

test('a superadmin creates an admin, promotes it, then removes it', async ({ page }) => {
	const selfEmail = 'admins-e2e-superadmin@example.org';
	await seedSuperadmin(selfEmail);
	await loginAs(page, selfEmail);

	await page.goto('/admins');

	// The logged-in admin's own row shows no actions, just "connecté".
	const ownRow = page.locator('[data-testid="admin-row"]', { hasText: selfEmail });
	await expect(ownRow).toContainText('connecté');
	await expect(ownRow.getByRole('button')).toHaveCount(0);

	const newEmail = `admins-e2e-created-${Date.now()}@example.org`;
	await page.getByRole('button', { name: '+ Ajouter un admin' }).click();
	await page.getByLabel('Nom', { exact: true }).fill('Bernard');
	await page.getByLabel('Prénom').fill('Luc');
	await page.getByLabel('Email').fill(newEmail);
	await page.getByRole('button', { name: "Créer l'admin" }).click();

	const newRow = page.locator('[data-testid="admin-row"]', { hasText: newEmail });
	await expect(newRow).toContainText('admin');

	await newRow.getByRole('button', { name: 'Promouvoir superadmin' }).click();
	await expect(newRow).toContainText('superadmin');
	await expect(newRow.getByRole('button', { name: 'Promouvoir superadmin' })).toHaveCount(0);

	await newRow.getByRole('button', { name: 'Retirer' }).click();
	await expect(page.locator('[data-testid="admin-row"]', { hasText: newEmail })).toHaveCount(0);
});

test('a plain admin cannot reach the Admins screen', async ({ page }) => {
	const email = 'admins-e2e-plain-admin@example.org';
	const client = new Client({ connectionString: DATABASE_URL });
	await client.connect();
	try {
		await client.query(
			`INSERT INTO admins (nom, prenom, email, is_superadmin) VALUES ('X', 'Y', $1, false)
			 ON CONFLICT (email) DO NOTHING`,
			[email]
		);
	} finally {
		await client.end();
	}
	await loginAs(page, email);

	const response = await page.request.get('/api/admins');
	expect(response.status()).toBe(403);
});
