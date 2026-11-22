// Stand-in for the Google Sheets + OAuth2 token APIs, used by e2e and by
// `docker-compose.dev.yml` for local dev, so neither needs real Google
// credentials/quota (cf. specs.md §3.3).
//
// - POST /token            — fake JWT-bearer token exchange
//   (`backend/src/google_auth.rs`); any request succeeds, no signature
//   check.
// - GET /v4/spreadsheets/:id?fields=... — metadata check used by
//   `SheetsClient::check_access`; any id succeeds.
// - GET /v4/spreadsheets/:id/values/:range — returns the 2027 form
//   response fixture (`fixtures/sheets_responses_form1.json`, a copy of
//   `backend/tests/fixtures/sheets_responses_form1.json` kept here so it
//   ships inside this directory's Docker volume mount) for every id/range,
//   so a dev sync against a stub-configured sheet_id has real-looking rows
//   to parse instead of an empty sheet.
import { createServer } from 'node:http';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const formResponses = JSON.parse(
	readFileSync(join(here, 'fixtures', 'sheets_responses_form1.json'), 'utf8')
);

const server = createServer((req, res) => {
	if (req.method === 'POST' && req.url === '/token') {
		res.writeHead(200, { 'Content-Type': 'application/json' });
		res.end(JSON.stringify({ access_token: 'dev-stub-token', expires_in: 3600 }));
		return;
	}

	if (req.url?.startsWith('/v4/spreadsheets/')) {
		const isValuesRequest = req.url.includes('/values/');
		res.writeHead(200, { 'Content-Type': 'application/json' });
		res.end(JSON.stringify(isValuesRequest ? formResponses : { spreadsheetId: 'dev-stub' }));
		return;
	}

	res.writeHead(404);
	res.end();
});

const port = Number(process.env.PORT ?? 8089);
server.listen(port, () => {
	console.log(`sheets-stub listening on :${port}`);
});
