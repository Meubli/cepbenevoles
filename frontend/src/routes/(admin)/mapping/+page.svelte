<script lang="ts">
	import { onMount } from 'svelte';
	import { api, ApiError } from '#lib/api';
	import { onLive } from '#lib/live';

	type MappingRow = {
		libelle: string;
		kind: string | null;
		commission_nom: string | null;
		poste_nom: string | null;
		count: number;
	};

	type Poste = { id: string; nom: string };
	type Commission = {
		id: string;
		nom: string;
		a_des_postes: boolean;
		couleur: string;
		postes: Poste[];
	};

	let rows = $state<MappingRow[]>([]);
	let commissions = $state<Commission[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	let search = $state('');
	let filter = $state<'all' | 'unmapped'>('all');

	// The label currently being associated, and the picker's selection.
	let editing = $state<string | null>(null);
	let selectedCommissionId = $state('');
	let selectedPosteId = $state('');

	// See commissions/+page.svelte's refresh() for why this is split from
	// load(): a live-refresh tick must not toggle `loading` (which would
	// unmount the Associer/Modifier form an admin has open).
	async function refresh() {
		try {
			[rows, commissions] = await Promise.all([
				api.get<MappingRow[]>('/mapping'),
				api.get<Commission[]>('/commissions')
			]);
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
		// "commissions": a commission/poste renamed or added changes the
		// picker and the "→ Commission · Poste" label already mapped rows
		// show. "sync": a new sync can surface a brand new unmapped label.
		return onLive(['mapping', 'commissions', 'sync'], () => void refresh());
	});

	function unmappedCount(): number {
		return rows.filter((r) => !r.kind).length;
	}

	function visibleRows(): MappingRow[] {
		const query = search.trim().toLowerCase();
		return rows.filter((r) => {
			if (filter === 'unmapped' && r.kind) return false;
			if (query && !r.libelle.toLowerCase().includes(query)) return false;
			return true;
		});
	}

	function startAssociating(libelle: string) {
		editing = libelle;
		selectedCommissionId = '';
		selectedPosteId = '';
	}

	function selectedCommission(): Commission | undefined {
		return commissions.find((c) => c.id === selectedCommissionId);
	}

	const POLYVALENT_VALUE = '__polyvalent__';

	function selectCommission(id: string) {
		selectedCommissionId = id;
		selectedPosteId = '';
	}

	async function saveMapping(libelle: string) {
		error = null;
		try {
			const isPolyvalent = selectedCommissionId === POLYVALENT_VALUE;
			await api.post('/mapping', {
				libelle,
				kind: isPolyvalent ? 'polyvalent' : 'commission',
				commission_id: isPolyvalent ? null : selectedCommissionId || null,
				poste_id: isPolyvalent ? null : selectedPosteId || null
			});
			editing = null;
			await load();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}
</script>

<div class="screen-head">
	<h2>Admin — mapping form</h2>
	<span class="screen-tag">Libellés "poste(s) souhaité(s)" → commissions/postes</span>
</div>
<p class="note">
	Libellés détectés dans les soumissions importées (événements non archivés), à associer à une
	commission (et éventuellement un poste précis). Une soumission avec un libellé non mappé est
	importée normalement ; seul ce souhait reste de côté jusqu'au mapping.
</p>

{#if error}
	<p role="alert" class="error-note">{error}</p>
{/if}

{#if loading}
	<p>Chargement…</p>
{:else}
	<div class="toolbar-row" style="margin-top:0; justify-content:flex-start">
		<input
			class="input-wf wf"
			style="flex:1; min-width:160px"
			type="search"
			placeholder="🔍 Rechercher un libellé…"
			bind:value={search}
		/>
		<div class="btn-group">
			<button class="btn-wf" class:primary={filter === 'all'} onclick={() => (filter = 'all')}>
				Tous
			</button>
			<button
				class="btn-wf"
				class:primary={filter === 'unmapped'}
				onclick={() => (filter = 'unmapped')}
			>
				Non mappés ({unmappedCount()})
			</button>
		</div>
	</div>

	{#each visibleRows() as row (row.libelle)}
		<div data-testid="mapping-row">
			<div class="comm-row" class:unmapped={!row.kind}>
				<span>"{row.libelle}"</span>
				<div class="comm-meta">
					{#if row.kind === 'polyvalent'}
						<span class="pill">spécial</span>
						<span class="wf-label">bénévole polyvalent — pas de mapping commission</span>
					{:else if row.kind}
						<span class="pill">mappé</span>
						<span class="wf-label">
							→ {row.commission_nom}{row.poste_nom ? ` · ${row.poste_nom}` : ''}
						</span>
					{:else}
						<span class="pill unmapped">non mappé</span>
					{/if}
					{#if row.kind !== 'polyvalent'}
						<button
							class="btn-wf"
							class:primary={!row.kind}
							onclick={() => startAssociating(row.libelle)}
						>
							{row.kind ? 'Modifier' : 'Associer'}
						</button>
					{/if}
				</div>
			</div>

			{#if editing === row.libelle}
				<div style="margin-top:12px; padding-top:16px; border-top:1px solid var(--line)">
					<h3>Associer — "{row.libelle}"</h3>
					<div class="field-grid">
						<div class="field wf">
							<span class="wf-label">Commission</span>
							<div class="btn-group">
								<button
									class="btn-wf"
									class:primary={selectedCommissionId === POLYVALENT_VALUE}
									onclick={() => selectCommission(POLYVALENT_VALUE)}
								>
									Bénévole polyvalent
								</button>
								{#each commissions as c (c.id)}
									<button
										class="btn-wf"
										class:primary={selectedCommissionId === c.id}
										style="border-left:4px solid {c.couleur}"
										onclick={() => selectCommission(c.id)}
									>
										{c.nom}
									</button>
								{/each}
							</div>
						</div>
						{#if selectedCommission()?.a_des_postes}
							<div class="field wf">
								<span class="wf-label">
									Poste <span class="optional-tag">— uniquement si la commission choisie en a</span>
								</span>
								<div class="btn-group">
									<button
										class="btn-wf"
										class:primary={selectedPosteId === ''}
										onclick={() => (selectedPosteId = '')}
									>
										Aucun poste précis
									</button>
									{#each selectedCommission()?.postes ?? [] as p (p.id)}
										<button
											class="btn-wf"
											class:primary={selectedPosteId === p.id}
											onclick={() => (selectedPosteId = p.id)}
										>
											{p.nom}
										</button>
									{/each}
								</div>
							</div>
						{/if}
					</div>
					<button
						class="btn-wf primary"
						style="margin-top:14px"
						onclick={() => saveMapping(row.libelle)}
					>
						Enregistrer le mapping
					</button>
					<button class="btn-wf" style="margin-top:14px" onclick={() => (editing = null)}>
						Annuler
					</button>
					<p class="note" style="margin-top:8px">
						S'applique aux prochaines synchronisations ; les soumissions déjà importées avec ce
						libellé seront mises à jour rétroactivement.
					</p>
				</div>
			{/if}
		</div>
	{/each}
{/if}
