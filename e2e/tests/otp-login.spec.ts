import { expect, test } from '@playwright/test';
import { TEST_ADMIN_EMAIL } from '../global-setup';
import { readLatestOtpCode } from './helpers/mailpit';
import { openTwoAdminContexts } from './helpers/multi-admin';

test('an admin logs in with an emailed OTP code and reaches the authenticated route', async ({
	page,
	request,
	baseURL
}) => {
	const requestResponse = await request.post(`${baseURL}/api/auth/request-code`, {
		data: { email: TEST_ADMIN_EMAIL }
	});
	expect(requestResponse.status()).toBe(202);

	const code = await readLatestOtpCode(request, TEST_ADMIN_EMAIL);

	const verifyResponse = await page.request.post(`${baseURL}/api/auth/verify-code`, {
		data: { email: TEST_ADMIN_EMAIL, code }
	});
	expect(verifyResponse.status()).toBe(200);

	// The session cookie set by verify-code is now in this page's context.
	const sessionResponse = await page.request.get(`${baseURL}/api/session`);
	expect(sessionResponse.status()).toBe(200);
});

test('two admins get independent browser contexts, each able to reach the app', async ({
	browser,
	baseURL
}) => {
	const [contextA, contextB] = await openTwoAdminContexts(browser);

	const [responseA, responseB] = await Promise.all([
		contextA.request.get(`${baseURL}/health`),
		contextB.request.get(`${baseURL}/health`)
	]);

	expect(responseA.status()).toBe(200);
	expect(responseB.status()).toBe(200);

	await Promise.all([contextA.close(), contextB.close()]);
});
