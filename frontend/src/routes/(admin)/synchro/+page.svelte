<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { api, ApiError } from '#lib/api';
	import { onLive } from '#lib/live';

	type SheetStatus = {
		sheet_id: string;
		event_names: string;
		soumission_count: number;
		last_started_at: string | null;
		last_finished_at: string | null;
		last_inserted: number | null;
		last_already_present: number | null;
		last_unrouted: number | null;
		last_error: string | null;
	};

	type MappingRow = { libelle: string; kind: string | null };

	let sheets = $state<SheetStatus[]>([]);
	let unmappedCount = $state(0);
	let loading = $state(true);
	let syncing = $state(false);
	let error = $state<string | null>(null);
	let lastSyncMessage = $state<string | null>(null);

	const mappingHref = resolve('/(admin)/mapping');

	// See commissions/+page.svelte's refresh() for why this is split from
	// load(): a live-refresh tick (e.g. the scheduler syncing in the
	// background, or another admin editing Mapping/Événements) must not
	// toggle `loading`.
	async function refresh() {
		try {
			const [statuses, mappingRows] = await Promise.all([
				api.get<SheetStatus[]>('/sync/status'),
				api.get<MappingRow[]>('/mapping')
			]);
			sheets = statuses;
			unmappedCount = mappingRows.filter((r) => !r.kind).length;
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	async function load() {
		loading = true;
		error = null;
		await refresh();
		loading = false;
	}

	onMount(() => {
		void load();
		// "evenements": a new sheet_id configured changes which sheets show
		// up here.
		return onLive(['sync', 'mapping', 'evenements'], () => void refresh());
	});

	function formatDateTime(value: string | null): string {
		if (!value) return 'jamais';
		return new Date(value).toLocaleString('fr-FR');
	}

	async function syncNow() {
		syncing = true;
		error = null;
		lastSyncMessage = null;
		try {
			const result = await api.post<{
				sheets: Array<{ sheet_id: string; status: string; inserted?: number; error?: string }>;
			}>('/sync');
			const failed = result.sheets.filter((s) => s.status === 'error');
			lastSyncMessage =
				failed.length > 0
					? `Synchronisation terminée, ${failed.length} Sheet(s) en échec (voir ci-dessous).`
					: 'Synchronisation terminée.';
			await load();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		} finally {
			syncing = false;
		}
	}
</script>

<div class="screen-head">
	<h2>Admin — synchro Google Form</h2>
	<span class="screen-tag">Import des soumissions</span>
</div>
<p class="note">
	Suivi de la synchronisation, par Google Sheet source (plusieurs événements actifs peuvent partager
	la même Sheet, lue une seule fois).
</p>

{#if error}
	<p role="alert" class="error-note">{error}</p>
{/if}

{#if loading}
	<p>Chargement…</p>
{:else}
	<div class="panel wf" style="margin-bottom:16px">
		<div class="toolbar-row" style="margin-top:0; justify-content:space-between">
			<h3 style="margin:0">Sheets configurées</h3>
			<button class="btn-wf primary" disabled={syncing} onclick={syncNow}>
				{syncing ? 'Synchronisation…' : 'Synchroniser maintenant'}
			</button>
		</div>
		{#if lastSyncMessage}
			<p class="note" style="margin:10px 0 0">{lastSyncMessage}</p>
		{/if}

		{#if sheets.length === 0}
			<p class="note" style="margin:10px 0 0">
				Aucun événement actif n'a de Sheet configurée (écran Événements).
			</p>
		{/if}

		{#each sheets as sheet (sheet.sheet_id)}
			<div
				data-testid="sync-sheet-row"
				style="margin-top:14px; padding-top:14px; border-top:1px solid var(--line)"
			>
				<h3 style="margin:0 0 10px">
					Sheet {sheet.sheet_id} <span class="wf-label">— {sheet.event_names}</span>
				</h3>
				<div class="sync-status">
					<div class="stat-wf wf">
						<div class="stat-num">{sheet.soumission_count}</div>
						<div class="stat-lbl">soumissions importées</div>
					</div>
					<div class="stat-wf wf">
						<div class="stat-num">{sheet.last_inserted ?? '—'}</div>
						<div class="stat-lbl">depuis la dernière synchro</div>
					</div>
					<div class="stat-wf wf">
						<div class="stat-num">{formatDateTime(sheet.last_finished_at)}</div>
						<div class="stat-lbl">dernière synchro</div>
					</div>
				</div>
				{#if sheet.last_error}
					<p role="alert" class="error-note" style="margin-top:10px">
						Dernière synchro en échec : {sheet.last_error}
					</p>
				{/if}
			</div>
		{/each}
	</div>

	<div
		class="panel wf"
		style="margin-bottom:16px"
		style:border-color={unmappedCount > 0 ? 'var(--accent)' : undefined}
	>
		<div class="toolbar-row" style="margin-top:0; justify-content:space-between">
			<h3 style="margin:0">{unmappedCount} libellé(s) de souhait non mappé(s)</h3>
			<a class="btn-wf primary" href={mappingHref}>Aller au mapping</a>
		</div>
		<p class="note">
			Soumissions importées normalement (identité, disponibilités) ; ces souhaits précis restent de
			côté tant qu'ils ne sont pas associés à une commission/poste.
		</p>
	</div>
{/if}
