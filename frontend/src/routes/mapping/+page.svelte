<script lang="ts">
	import { onMount } from 'svelte';
	import { api, ApiError } from '#lib/api';

	type MappingRow = {
		libelle: string;
		kind: string | null;
		commission_nom: string | null;
		poste_nom: string | null;
		count: number;
	};

	type Poste = { id: string; nom: string };
	type Commission = { id: string; nom: string; a_des_postes: boolean; postes: Poste[] };

	let rows = $state<MappingRow[]>([]);
	let commissions = $state<Commission[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	// The label currently being associated, and the picker's selection.
	let editing = $state<string | null>(null);
	let selectedCommissionId = $state('');
	let selectedPosteId = $state('');

	async function load() {
		loading = true;
		error = null;
		try {
			[rows, commissions] = await Promise.all([
				api.get<MappingRow[]>('/mapping'),
				api.get<Commission[]>('/commissions')
			]);
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		} finally {
			loading = false;
		}
	}

	onMount(load);

	function startAssociating(libelle: string) {
		editing = libelle;
		selectedCommissionId = '';
		selectedPosteId = '';
	}

	function selectedCommission(): Commission | undefined {
		return commissions.find((c) => c.id === selectedCommissionId);
	}

	async function saveMapping(libelle: string) {
		error = null;
		try {
			await api.post('/mapping', {
				libelle,
				kind: 'commission',
				commission_id: selectedCommissionId || null,
				poste_id: selectedPosteId || null
			});
			editing = null;
			await load();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}
</script>

<h2>Admin — mapping form</h2>
<p>
	Libellés "poste(s) souhaité(s)" détectés dans les soumissions importées (événements non archivés),
	à associer à une commission (et éventuellement un poste précis).
</p>

{#if error}
	<p role="alert">{error}</p>
{/if}

{#if loading}
	<p>Chargement…</p>
{:else}
	<table>
		<thead>
			<tr>
				<th>Libellé</th>
				<th>Statut</th>
				<th>Associé à</th>
				<th>Soumissions</th>
				<th></th>
			</tr>
		</thead>
		<tbody>
			{#each rows as row (row.libelle)}
				<tr>
					<td>{row.libelle}</td>
					<td>{row.kind ? 'mappé' : 'non mappé'}</td>
					<td>
						{#if row.commission_nom}
							{row.commission_nom}{row.poste_nom ? ` · ${row.poste_nom}` : ''}
						{:else if row.kind === 'polyvalent'}
							Bénévole polyvalent
						{/if}
					</td>
					<td>{row.count}</td>
					<td>
						{#if editing === row.libelle}
							<select bind:value={selectedCommissionId}>
								<option value="">— commission —</option>
								{#each commissions as c (c.id)}
									<option value={c.id}>{c.nom}</option>
								{/each}
							</select>
							{#if selectedCommission()?.a_des_postes}
								<select bind:value={selectedPosteId}>
									<option value="">Aucun poste précis</option>
									{#each selectedCommission()?.postes ?? [] as p (p.id)}
										<option value={p.id}>{p.nom}</option>
									{/each}
								</select>
							{/if}
							<button onclick={() => saveMapping(row.libelle)}>Enregistrer</button>
							<button onclick={() => (editing = null)}>Annuler</button>
						{:else}
							<button onclick={() => startAssociating(row.libelle)}>
								{row.kind ? 'Modifier' : 'Associer'}
							</button>
						{/if}
					</td>
				</tr>
			{/each}
		</tbody>
	</table>
{/if}
