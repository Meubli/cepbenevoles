import type { Browser, BrowserContext } from '@playwright/test';

/**
 * Opens two independent authenticated browser contexts — the shape needed
 * for the future "two admins see each other's websocket refresh" scenario
 * (specs.md §7.1). Each context gets its own cookie jar, so logins don't
 * leak between them.
 */
export async function openTwoAdminContexts(
	browser: Browser
): Promise<[BrowserContext, BrowserContext]> {
	const [contextA, contextB] = await Promise.all([browser.newContext(), browser.newContext()]);
	return [contextA, contextB];
}
