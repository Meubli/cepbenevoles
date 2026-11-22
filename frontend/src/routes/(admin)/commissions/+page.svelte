<script lang="ts">
	import { onMount } from 'svelte';
	import { api, ApiError } from '#lib/api';
	import { onLive } from '#lib/live';

	type Poste = { id: string; nom: string; prerequis_libre_texte: string | null };
	type Commission = {
		id: string;
		nom: string;
		prerequis_libre_texte: string | null;
		a_des_postes: boolean;
		grille_creneaux: 'generale' | 'propre';
		besoin_chiffre: boolean;
		couleur: string;
		postes: Poste[];
	};

	type FormState = {
		nom: string;
		prerequis_libre_texte: string;
		a_des_postes: boolean;
		grille_creneaux: 'generale' | 'propre';
		besoin_chiffre: boolean;
		couleur: string;
	};

	// Pleasant-looking random hex color (fixed saturation/lightness, random
	// hue) — specs.md §5: "couleur tirée aléatoirement à sa création,
	// modifiable [...] dès l'écran de création".
	function randomColor(): string {
		const hue = Math.floor(Math.random() * 360);
		const s = 0.55;
		const l = 0.6;
		const k = (n: number) => (n + hue / 30) % 12;
		const a = s * Math.min(l, 1 - l);
		const f = (n: number) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
		const toHex = (n: number) =>
			Math.round(f(n) * 255)
				.toString(16)
				.padStart(2, '0');
		return `#${toHex(0)}${toHex(8)}${toHex(4)}`;
	}

	function emptyForm(): FormState {
		return {
			nom: '',
			prerequis_libre_texte: '',
			a_des_postes: false,
			grille_creneaux: 'generale',
			besoin_chiffre: true,
			couleur: randomColor()
		};
	}

	let commissions = $state<Commission[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let search = $state('');

	let creating = $state(false);
	let createForm = $state<FormState>(emptyForm());

	let editingId = $state<string | null>(null);
	let editForm = $state<FormState>(emptyForm());

	let managingPostesId = $state<string | null>(null);
	let newPosteNom = $state('');

	// Refetches without touching `loading`/`editingId`/`creating` — used
	// both for the first load and for a live-refresh tick (technical-specs.md
	// §10): a background refetch must never unmount an in-progress form. Also
	// never clears `error` on success — a live tick from another admin's
	// change must not wipe out an unrelated validation message this page's
	// own form just set (e.g. two e2e workers hitting the same backend).
	async function refresh() {
		try {
			commissions = await api.get<Commission[]>('/commissions');
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
		return onLive(['commissions'], () => void refresh());
	});

	function visibleCommissions(): Commission[] {
		const query = search.trim().toLowerCase();
		if (!query) return commissions;
		return commissions.filter((c) => c.nom.toLowerCase().includes(query));
	}

	function toForm(c: Commission): FormState {
		return {
			nom: c.nom,
			prerequis_libre_texte: c.prerequis_libre_texte ?? '',
			a_des_postes: c.a_des_postes,
			grille_creneaux: c.grille_creneaux,
			besoin_chiffre: c.besoin_chiffre,
			couleur: c.couleur
		};
	}

	function toBody(f: FormState) {
		return {
			nom: f.nom,
			prerequis_libre_texte: f.prerequis_libre_texte || null,
			a_des_postes: f.a_des_postes,
			grille_creneaux: f.grille_creneaux,
			besoin_chiffre: f.besoin_chiffre,
			couleur: f.couleur
		};
	}

	function startEditing(c: Commission) {
		managingPostesId = null;
		editingId = c.id;
		editForm = toForm(c);
	}

	async function saveEdit(id: string) {
		error = null;
		try {
			await api.patch(`/commissions/${id}`, toBody(editForm));
			editingId = null;
			await load();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	async function createCommission() {
		error = null;
		if (!createForm.nom.trim()) {
			error = 'Le nom est obligatoire.';
			return;
		}
		try {
			await api.post('/commissions', toBody(createForm));
			creating = false;
			createForm = emptyForm();
			await load();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	function manageExistingPostes(c: Commission) {
		editingId = null;
		managingPostesId = c.id;
		newPosteNom = '';
	}

	async function addPoste(commissionId: string) {
		error = null;
		if (!newPosteNom.trim()) return;
		try {
			await api.post(`/commissions/${commissionId}/postes`, { nom: newPosteNom });
			newPosteNom = '';
			await load();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	async function removePoste(commissionId: string, posteId: string) {
		error = null;
		try {
			await api.delete(`/commissions/${commissionId}/postes/${posteId}`);
			await load();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	function managingCommission(): Commission | undefined {
		return commissions.find((c) => c.id === managingPostesId);
	}
</script>

<div class="toolbar-row" style="margin-top:0">
	<input
		class="input-wf wf"
		style="flex:1; min-width:160px"
		type="search"
		placeholder="🔍 Rechercher une commission…"
		bind:value={search}
	/>
	{#if !creating}
		<button
			class="btn-wf primary"
			onclick={() => {
				editingId = null;
				managingPostesId = null;
				createForm = emptyForm();
				creating = true;
			}}
		>
			+ Nouvelle commission
		</button>
	{/if}
</div>
<p class="note">
	Liste des commissions disponibles ; une nouvelle peut être ajoutée à tout moment.
</p>

{#if error}
	<p role="alert" class="error-note">{error}</p>
{/if}

{#if loading}
	<p>Chargement…</p>
{:else}
	{#if creating}
		<div style="margin-bottom:22px; padding-bottom:16px; border-bottom:1px solid var(--line)">
			<h3>Nouvelle commission</h3>
			<div class="field-grid">
				<div class="field wf">
					<label>
						Nom
						<input class="input-wf" bind:value={createForm.nom} required />
					</label>
				</div>
				<div class="field wf">
					<label>
						Prérequis (texte libre)
						<input class="input-wf" bind:value={createForm.prerequis_libre_texte} />
					</label>
				</div>
				<div class="field wf">
					<span class="wf-label">Grille de créneaux</span>
					<div class="btn-group">
						<button
							class="btn-wf"
							class:primary={createForm.grille_creneaux === 'generale'}
							onclick={() => (createForm.grille_creneaux = 'generale')}
						>
							Générale
						</button>
						<button
							class="btn-wf"
							class:primary={createForm.grille_creneaux === 'propre'}
							onclick={() => (createForm.grille_creneaux = 'propre')}
						>
							Propre
						</button>
					</div>
				</div>
				<div class="field wf">
					<span class="wf-label">A des postes</span>
					<div class="btn-group">
						<button
							class="btn-wf"
							class:primary={!createForm.a_des_postes}
							onclick={() => (createForm.a_des_postes = false)}
						>
							Non
						</button>
						<button
							class="btn-wf"
							class:primary={createForm.a_des_postes}
							onclick={() => (createForm.a_des_postes = true)}
						>
							Oui
						</button>
					</div>
				</div>
				<div class="field wf">
					<span class="wf-label">
						Besoin chiffré <span class="optional-tag">— défaut : oui</span>
					</span>
					<div class="btn-group">
						<button
							class="btn-wf"
							class:primary={createForm.besoin_chiffre}
							onclick={() => (createForm.besoin_chiffre = true)}
						>
							Oui
						</button>
						<button
							class="btn-wf"
							class:primary={!createForm.besoin_chiffre}
							onclick={() => (createForm.besoin_chiffre = false)}
						>
							Non
						</button>
					</div>
				</div>
				<div class="field wf">
					<span class="wf-label">
						Couleur <span class="optional-tag">— tirée au hasard, modifiable dès maintenant</span>
					</span>
					<div class="toolbar-row" style="margin:4px 0 0">
						<span
							class="color-swatch"
							style="background:{createForm.couleur}; width:22px; height:22px"
						></span>
						<button class="btn-wf" onclick={() => (createForm.couleur = randomColor())}>
							Changer
						</button>
					</div>
				</div>
			</div>
			<button class="btn-wf primary" style="margin-top:14px" onclick={createCommission}>
				Créer la commission
			</button>
			<button class="btn-wf" style="margin-top:14px" onclick={() => (creating = false)}>
				Annuler
			</button>
		</div>
	{/if}

	{#each visibleCommissions() as c (c.id)}
		<div class="comm-row" data-testid="commission-row">
			<div class="comm-main">
				<span>
					<span class="color-swatch" style="background:{c.couleur}"></span>
					{c.nom}
				</span>
				{#if c.postes.length > 0}
					<div class="poste-list">
						{#each c.postes as p (p.id)}
							<span class="poste-item">· {p.nom}</span>
						{/each}
					</div>
				{/if}
			</div>
			<div class="comm-meta">
				<span class="grille-badge" class:propre={c.grille_creneaux === 'propre'}>
					grille {c.grille_creneaux === 'propre' ? 'propre' : 'générale'}
				</span>
				{#if !c.besoin_chiffre}
					<span class="grille-badge propre">sans besoin chiffré</span>
				{/if}
				{#if c.a_des_postes}
					<span class="pill">{c.postes.length} poste{c.postes.length > 1 ? 's' : ''}</span>
					<button class="btn-wf" onclick={() => manageExistingPostes(c)}>Gérer les postes</button>
				{:else}
					<span class="pill">active</span>
				{/if}
				<button class="btn-wf" onclick={() => startEditing(c)}>Modifier</button>
			</div>
		</div>

		{#if editingId === c.id}
			<div style="margin-top:12px; padding-top:16px; border-top:1px solid var(--line)">
				<h3>Modifier — commission <em>{c.nom}</em></h3>
				<div class="field-grid">
					<div class="field wf">
						<label>
							Nom
							<input class="input-wf" bind:value={editForm.nom} required />
						</label>
					</div>
					<div class="field wf">
						<label>
							Prérequis (texte libre)
							<input class="input-wf" bind:value={editForm.prerequis_libre_texte} />
						</label>
					</div>
					<div class="field wf">
						<span class="wf-label">Grille de créneaux</span>
						<div class="btn-group">
							<button
								class="btn-wf"
								class:primary={editForm.grille_creneaux === 'generale'}
								onclick={() => (editForm.grille_creneaux = 'generale')}
							>
								Générale
							</button>
							<button
								class="btn-wf"
								class:primary={editForm.grille_creneaux === 'propre'}
								onclick={() => (editForm.grille_creneaux = 'propre')}
							>
								Propre
							</button>
						</div>
					</div>
					<div class="field wf">
						<span class="wf-label">A des postes</span>
						<div class="btn-group">
							<button
								class="btn-wf"
								class:primary={!editForm.a_des_postes}
								onclick={() => (editForm.a_des_postes = false)}
							>
								Non
							</button>
							<button
								class="btn-wf"
								class:primary={editForm.a_des_postes}
								onclick={() => (editForm.a_des_postes = true)}
							>
								Oui
							</button>
						</div>
					</div>
					<div class="field wf">
						<span class="wf-label">
							Besoin chiffré
							<span class="optional-tag">
								— off = pas de nombre requis, écran de liste uniquement
							</span>
						</span>
						<div class="btn-group">
							<button
								class="btn-wf"
								class:primary={editForm.besoin_chiffre}
								onclick={() => (editForm.besoin_chiffre = true)}
							>
								Oui
							</button>
							<button
								class="btn-wf"
								class:primary={!editForm.besoin_chiffre}
								onclick={() => (editForm.besoin_chiffre = false)}
							>
								Non
							</button>
						</div>
					</div>
					<div class="field wf">
						<span class="wf-label">
							Couleur <span class="optional-tag">— modifiable ici</span>
						</span>
						<div class="toolbar-row" style="margin:4px 0 0">
							<span
								class="color-swatch"
								style="background:{editForm.couleur}; width:22px; height:22px"
							></span>
							<button class="btn-wf" onclick={() => (editForm.couleur = randomColor())}>
								Changer
							</button>
						</div>
					</div>
				</div>
				<button class="btn-wf primary" style="margin-top:14px" onclick={() => saveEdit(c.id)}>
					Enregistrer
				</button>
				<button class="btn-wf" style="margin-top:14px" onclick={() => (editingId = null)}>
					Annuler
				</button>
			</div>
		{/if}

		{#if managingPostesId === c.id}
			<div style="margin-top:12px; padding-top:16px; border-top:1px solid var(--line)">
				<h3>Gestion des postes — commission <em>{c.nom}</em></h3>
				<p class="note" style="margin:4px 0 12px">
					Le besoin de chaque poste par créneau se saisit dans l'écran Besoins ; la somme doit
					égaler le besoin de la commission.
				</p>

				{#each managingCommission()?.postes ?? [] as p (p.id)}
					<div class="poste-edit-row">
						<span class="input-wf wf" style="display:flex; align-items:center; padding:0 8px">
							{p.nom}
						</span>
						<button class="btn-wf danger" onclick={() => removePoste(c.id, p.id)}>
							Supprimer
						</button>
					</div>
				{/each}

				<div class="poste-edit-row">
					<input
						class="input-wf wf"
						placeholder="+ Nom du nouveau poste…"
						bind:value={newPosteNom}
					/>
					<button class="btn-wf" onclick={() => addPoste(c.id)}>Ajouter</button>
				</div>

				<div class="toolbar-row" style="justify-content:flex-end">
					<button class="btn-wf" onclick={() => (managingPostesId = null)}>Fermer</button>
				</div>
			</div>
		{/if}
	{/each}
{/if}
