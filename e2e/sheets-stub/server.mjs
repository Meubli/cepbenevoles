// Trivial stand-in for the Google Sheets API used only by e2e tests, so
// e2e doesn't depend on real Google credentials/quota (cf. specs.md §3.3).
// Returns an empty values response for any spreadsheet/range — enough for
// the smoke test, which doesn't exercise the sync feature itself yet.
import { createServer } from 'node:http';

const server = createServer((req, res) => {
	if (req.url?.startsWith('/v4/spreadsheets/')) {
		res.writeHead(200, { 'Content-Type': 'application/json' });
		res.end(JSON.stringify({ values: [] }));
		return;
	}
	res.writeHead(404);
	res.end();
});

const port = Number(process.env.PORT ?? 8089);
server.listen(port, () => {
	console.log(`sheets-stub listening on :${port}`);
});
