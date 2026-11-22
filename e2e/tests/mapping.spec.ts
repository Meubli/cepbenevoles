import { expect, test } from '@playwright/test';
import { Client } from 'pg';
import { readLatestOtpCode } from './helpers/mailpit';

const DATABASE_URL =
	process.env.E2E_DATABASE_URL ?? 'postgres://postgres:postgres@127.0.0.1:55433/benefrance_e2e';

// A distinct admin email from otp-login.spec.ts's TEST_ADMIN_EMAIL: both
// specs run in parallel (fullyParallel), and readLatestOtpCode picks the
// newest email to a given address — sharing one address across parallel
// logins is a real race, not a theoretical one (it flaked immediately).
const MAPPING_TEST_ADMIN_EMAIL = 'mapping-e2e-admin@example.org';

async function seedUnmappedLabel(libelle: string): Promise<void> {
	const client = new Client({ connectionString: DATABASE_URL });
	await client.connect();
	try {
		await client.query(
			`INSERT INTO admins (email, is_superadmin) VALUES ($1, true)
			 ON CONFLICT (email) DO NOTHING`,
			[MAPPING_TEST_ADMIN_EMAIL]
		);

		const { rows } = await client.query(
			`INSERT INTO evenement (nom, date_debut, date_fin)
			 VALUES ('E2E mapping event', '2027-01-01', '2027-01-02')
			 RETURNING id`
		);
		const evenementId = rows[0].id;

		const { rows: soumissionRows } = await client.query(
			`INSERT INTO soumission
				(evenement_id, nom, prenom, email, telephone, tranche_age, permis_b, taille_tshirt, date_soumission)
			 VALUES ($1, 'Nom', 'Prenom', 'e2e@example.com', '0600000000', '18 ans et +', true, 'M', now())
			 RETURNING id`,
			[evenementId]
		);
		const soumissionId = soumissionRows[0].id;

		await client.query(
			`INSERT INTO soumission_libelle (soumission_id, libelle_brut) VALUES ($1, $2)`,
			[soumissionId, libelle]
		);
	} finally {
		await client.end();
	}
}

test('an admin maps a pending label to a commission from the Mapping screen', async ({ page }) => {
	const libelle = `E2E libellé ${Date.now()}`;
	await seedUnmappedLabel(libelle);

	// Log in (same OTP flow as otp-login.spec.ts) so the session cookie is
	// in this test's browser context before visiting the admin page.
	await page.request.post('/api/auth/request-code', {
		data: { email: MAPPING_TEST_ADMIN_EMAIL }
	});
	const code = await readLatestOtpCode(page.request, MAPPING_TEST_ADMIN_EMAIL);
	await page.request.post('/api/auth/verify-code', {
		data: { email: MAPPING_TEST_ADMIN_EMAIL, code }
	});

	await page.goto('/mapping');

	const row = page.locator('[data-testid="mapping-row"]', { hasText: libelle });
	await expect(row).toContainText('non mappé');

	await row.getByRole('button', { name: 'Associer' }).click();
	await row.getByRole('combobox').first().selectOption({ label: 'Accueil' });
	await row.getByRole('button', { name: 'Enregistrer' }).click();

	await expect(row).not.toContainText('non mappé');
	await expect(row).toContainText('Accueil');
});
