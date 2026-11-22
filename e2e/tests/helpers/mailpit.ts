import type { APIRequestContext } from '@playwright/test';

const MAILPIT_HTTP_URL = process.env.E2E_MAILPIT_URL ?? 'http://127.0.0.1:18025';

/**
 * Polls Mailpit's HTTP API for the most recent email to `toEmail` and
 * extracts the 6-digit OTP from its body (specs.md §11.1). Real SMTP
 * delivery, not a shortcut: the backend actually sends through SMTP to
 * Mailpit, and this reads it back the same way a human would check inbox.
 */
export async function readLatestOtpCode(
	request: APIRequestContext,
	toEmail: string
): Promise<string> {
	const deadline = Date.now() + 10_000;

	while (Date.now() < deadline) {
		const response = await request.get(`${MAILPIT_HTTP_URL}/api/v1/messages`);
		const body = await response.json();

		const message = body.messages?.find((m: { To: { Address: string }[] }) =>
			m.To?.some((recipient) => recipient.Address === toEmail)
		);

		if (message) {
			const detail = await request.get(`${MAILPIT_HTTP_URL}/api/v1/message/${message.ID}`);
			const detailBody = await detail.json();
			const match = /\b(\d{6})\b/.exec(detailBody.Text ?? '');
			if (match) {
				return match[1];
			}
		}

		await new Promise((resolve) => setTimeout(resolve, 300));
	}

	throw new Error(`timed out waiting for an OTP email to ${toEmail}`);
}
