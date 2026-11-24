<script lang="ts">
	import { onMount } from 'svelte';
	import { api, ApiError } from '#lib/api';
	import { onLive } from '#lib/live';
	import { createSaveFlash } from '#lib/saveFeedback.svelte';

	type Evenement = {
		id: string;
		nom: string;
		date_debut: string;
		date_fin: string;
		archive: boolean;
	};
	type Poste = { id: string; nom: string };
	type Commission = {
		id: string;
		nom: string;
		couleur: string;
		a_des_postes: boolean;
		postes: Poste[];
	};
	type CreneauMapping = { jour: string; slot_form: string };

	const SLOT_ORDER = ['matin', 'midi', 'aprem', 'soir'] as const;
	const SLOT_LABEL: Record<string, string> = {
		matin: 'matin',
		midi: 'midi',
		aprem: 'aprem',
		soir: 'soir'
	};
	const TRANCHE_AGE_OPTIONS = [
		{ label: '10-12', value: '10 - 12 ans' },
		{ label: '13-14', value: '13 - 14 ans' },
		{ label: '15-17', value: '15 - 17 ans' },
		{ label: '18+', value: '18 ans et +' }
	];
	const TAILLE_TSHIRT_OPTIONS = ['S', 'M', 'L', 'XL'];
	const POLYVALENT_LABEL = "Je suis joueur, peu m'importe";

	// Mêmes règles et mêmes limites que le backend (benevoles/mod.rs,
	// admins/mod.rs pour l'email) — validées ici en plus pour un retour
	// immédiat, le backend reste la source de vérité (revalidé à l'envoi).
	const NOM_MAX_LEN = 100;
	const EMAIL_MAX_LEN = 255;
	const TELEPHONE_MAX_LEN = 30;

	function isValidEmail(email: string): boolean {
		const at = email.indexOf('@');
		if (at === -1) return false;
		const local = email.slice(0, at);
		const domain = email.slice(at + 1);
		if (local.length === 0) return false;
		if (/\s/.test(email)) return false;
		if (!domain.includes('.')) return false;
		if (domain.startsWith('.') || domain.endsWith('.')) return false;
		const tld = domain.slice(domain.lastIndexOf('.') + 1);
		return tld.length >= 2;
	}

	function validateNom(v: string): string | undefined {
		if (v.trim().length === 0) return 'Le nom ne peut pas être vide.';
		if (v.trim().length > NOM_MAX_LEN)
			return `Le nom ne peut pas dépasser ${NOM_MAX_LEN} caractères.`;
	}
	function validatePrenom(v: string): string | undefined {
		if (v.trim().length === 0) return 'Le prénom ne peut pas être vide.';
		if (v.trim().length > NOM_MAX_LEN)
			return `Le prénom ne peut pas dépasser ${NOM_MAX_LEN} caractères.`;
	}
	function validateEmail(v: string): string | undefined {
		if (v.trim().length === 0) return "L'email ne peut pas être vide.";
		if (v.trim().length > EMAIL_MAX_LEN)
			return `L'email ne peut pas dépasser ${EMAIL_MAX_LEN} caractères.`;
		if (!isValidEmail(v.trim())) return 'Adresse email invalide.';
	}
	function isValidTelephone(telephone: string): boolean {
		const t = telephone.trim();
		if (t.length === 0) return false;
		const rest = t.startsWith('+') ? t.slice(1) : t;
		if (rest.includes('+')) return false;
		const digitCount = (rest.match(/\d/g) ?? []).length;
		const onlyAllowedChars = /^[\d .()-]*$/.test(rest);
		return onlyAllowedChars && digitCount >= 10;
	}
	function validateTelephone(v: string): string | undefined {
		if (v.trim().length === 0) return 'Le téléphone ne peut pas être vide.';
		if (v.trim().length > TELEPHONE_MAX_LEN)
			return `Le téléphone ne peut pas dépasser ${TELEPHONE_MAX_LEN} caractères.`;
		if (!isValidTelephone(v)) return 'Numéro de téléphone invalide.';
	}

	const FIELD_VALIDATORS: Record<string, (v: string) => string | undefined> = {
		nom: validateNom,
		prenom: validatePrenom,
		email: validateEmail,
		telephone: validateTelephone
	};
	const FIELD_VALUES: Record<string, () => string> = {
		nom: () => nom,
		prenom: () => prenom,
		email: () => email,
		telephone: () => telephone
	};

	// Dates manipulées en UTC uniquement (même piège off-by-one que les
	// écrans Créneaux/Besoins avec new Date(iso) en heure locale).
	function parseIsoDateUtc(s: string): Date {
		const [y, m, d] = s.split('-').map(Number);
		return new Date(Date.UTC(y, m - 1, d));
	}
	function formatJourShort(iso: string): string {
		const d = parseIsoDateUtc(iso);
		const weekday = d
			.toLocaleDateString('fr-FR', { weekday: 'short', timeZone: 'UTC' })
			.replace('.', '');
		const dm = d.toLocaleDateString('fr-FR', { day: '2-digit', month: '2-digit', timeZone: 'UTC' });
		return `${weekday.charAt(0).toUpperCase()}${weekday.slice(1)} ${dm}`;
	}

	type EventFormState = {
		mapping: CreneauMapping[];
		loadingMapping: boolean;
		// clé = "<jour>|<slot_form>"
		dispos: Record<string, boolean>;
		polyvalent: boolean;
		// clé = "<commissionId>" (souhait sans poste) ou "<commissionId>|<posteId>"
		souhaits: Record<string, boolean>;
	};

	let evenements = $state<Evenement[]>([]);
	let commissions = $state<Commission[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let fieldErrors = $state<Record<string, string>>({});
	let saving = $state(false);

	let nom = $state('');
	let prenom = $state('');
	let email = $state('');
	let telephone = $state('');
	let trancheAge = $state(TRANCHE_AGE_OPTIONS[3].value);
	let permisB = $state(false);
	let tailleTshirt = $state('M');

	// clé = evenementId
	let eventForms = $state<Record<string, EventFormState>>({});

	const saved = createSaveFlash();

	async function refresh() {
		try {
			const [evs, comms] = await Promise.all([
				api.get<Evenement[]>('/evenements'),
				api.get<Commission[]>('/commissions')
			]);
			evenements = evs;
			commissions = comms;
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
		return onLive(['evenements', 'commissions'], () => void refresh());
	});

	function evenementsActifs(): Evenement[] {
		return evenements.filter((e) => !e.archive);
	}

	async function toggleEvenement(evenementId: string) {
		if (eventForms[evenementId]) {
			const next = { ...eventForms };
			delete next[evenementId];
			eventForms = next;
			return;
		}

		const state: EventFormState = {
			mapping: [],
			loadingMapping: true,
			dispos: {},
			polyvalent: false,
			souhaits: {}
		};
		eventForms = { ...eventForms, [evenementId]: state };

		try {
			const data = await api.get<{ mapping: CreneauMapping[] }>(`/creneaux/${evenementId}`);
			const current = eventForms[evenementId];
			if (!current) return; // retiré pendant le chargement
			eventForms = {
				...eventForms,
				[evenementId]: { ...current, mapping: data.mapping, loadingMapping: false }
			};
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
			const current = eventForms[evenementId];
			if (current) {
				eventForms = { ...eventForms, [evenementId]: { ...current, loadingMapping: false } };
			}
		}
	}

	function joursMappes(mapping: CreneauMapping[]): string[] {
		return Array.from(new Set(mapping.map((m) => m.jour))).sort();
	}
	function slotsDuJour(mapping: CreneauMapping[], jour: string): string[] {
		const present = new Set(mapping.filter((m) => m.jour === jour).map((m) => m.slot_form));
		return SLOT_ORDER.filter((s) => present.has(s));
	}

	function disporKey(jour: string, slot: string): string {
		return `${jour}|${slot}`;
	}
	function souhaitKey(commissionId: string, posteId?: string): string {
		return posteId ? `${commissionId}|${posteId}` : commissionId;
	}

	function toggleDispo(evenementId: string, jour: string, slot: string) {
		const s = eventForms[evenementId];
		if (!s) return;
		const key = disporKey(jour, slot);
		eventForms = {
			...eventForms,
			[evenementId]: { ...s, dispos: { ...s.dispos, [key]: !s.dispos[key] } }
		};
	}
	function togglePolyvalent(evenementId: string) {
		const s = eventForms[evenementId];
		if (!s) return;
		eventForms = { ...eventForms, [evenementId]: { ...s, polyvalent: !s.polyvalent } };
	}
	function toggleSouhait(evenementId: string, key: string) {
		const s = eventForms[evenementId];
		if (!s) return;
		eventForms = {
			...eventForms,
			[evenementId]: { ...s, souhaits: { ...s.souhaits, [key]: !s.souhaits[key] } }
		};
	}

	function validateField(name: keyof typeof FIELD_VALIDATORS) {
		const message = FIELD_VALIDATORS[name](FIELD_VALUES[name]());
		const next = { ...fieldErrors };
		if (message) next[name] = message;
		else delete next[name];
		fieldErrors = next;
	}

	function clearFieldErrorIfFixed(name: keyof typeof FIELD_VALIDATORS) {
		if (fieldErrors[name]) validateField(name);
	}

	function validateAll(): boolean {
		for (const name of Object.keys(FIELD_VALIDATORS)) {
			validateField(name);
		}
		return Object.keys(fieldErrors).length === 0;
	}

	function resetForm() {
		nom = '';
		prenom = '';
		email = '';
		telephone = '';
		trancheAge = TRANCHE_AGE_OPTIONS[3].value;
		permisB = false;
		tailleTshirt = 'M';
		eventForms = {};
		fieldErrors = {};
	}

	async function submit() {
		error = null;
		if (!validateAll()) return;
		const evenementIds = Object.keys(eventForms);
		if (evenementIds.length === 0) {
			error = 'Sélectionnez au moins un événement.';
			return;
		}

		const soumissions = evenementIds.map((evenementId) => {
			const s = eventForms[evenementId];
			const disponibilites = Object.entries(s.dispos)
				.filter(([, checked]) => checked)
				.map(([key]) => {
					const [jour, slot] = key.split('|');
					return { jour, slot };
				});
			const souhaits = Object.entries(s.souhaits)
				.filter(([, checked]) => checked)
				.map(([key]) => {
					const [commission_id, poste_id] = key.split('|');
					return poste_id ? { commission_id, poste_id } : { commission_id };
				});
			return { evenement_id: evenementId, disponibilites, polyvalent: s.polyvalent, souhaits };
		});

		if (
			soumissions.some(
				(s) => s.disponibilites.length === 0 && !s.polyvalent && s.souhaits.length === 0
			)
		) {
			error =
				'Indiquez au moins un créneau de disponibilité ou un poste souhaité pour chaque événement.';
			return;
		}

		saving = true;
		try {
			await api.post('/benevoles', {
				nom,
				prenom,
				email,
				telephone,
				tranche_age: trancheAge,
				permis_b: permisB,
				taille_tshirt: tailleTshirt,
				soumissions
			});
			resetForm();
			saved.trigger();
		} catch (e) {
			if (e instanceof ApiError && e.field) {
				fieldErrors = { [e.field]: e.message };
			} else {
				error = e instanceof ApiError ? e.message : 'Erreur inattendue';
			}
		} finally {
			saving = false;
		}
	}
</script>

<div class="screen-head">
	<h2>Admin — nouveau bénévole</h2>
	<span class="screen-tag">Saisie manuelle</span>
</div>
<p class="note">
	Même structure que le Google Form, pour un bénévole arrivé hors formulaire. Crée une soumission
	par événement coché, avec les mêmes règles que les soumissions importées (specs.md §3.4/§9).
</p>

{#if error}
	<p role="alert" class="error-note">{error}</p>
{/if}

{#if loading}
	<p>Chargement…</p>
{:else}
	<div class="panel wf" style="margin-bottom:14px">
		<h3>Identité / contact</h3>
		<div class="field-grid">
			<div class="field wf">
				<label>
					Nom
					<input
						class="input-wf"
						type="text"
						bind:value={nom}
						maxlength="100"
						required
						onblur={() => validateField('nom')}
						oninput={() => clearFieldErrorIfFixed('nom')}
					/>
				</label>
				{#if fieldErrors.nom}
					<p class="error-note" style="margin:4px 0 0">{fieldErrors.nom}</p>
				{/if}
			</div>
			<div class="field wf">
				<label>
					Prénom
					<input
						class="input-wf"
						type="text"
						bind:value={prenom}
						maxlength="100"
						required
						onblur={() => validateField('prenom')}
						oninput={() => clearFieldErrorIfFixed('prenom')}
					/>
				</label>
				{#if fieldErrors.prenom}
					<p class="error-note" style="margin:4px 0 0">{fieldErrors.prenom}</p>
				{/if}
			</div>
			<div class="field wf">
				<label>
					Email
					<input
						class="input-wf"
						type="email"
						bind:value={email}
						maxlength="255"
						required
						onblur={() => validateField('email')}
						oninput={() => clearFieldErrorIfFixed('email')}
					/>
				</label>
				{#if fieldErrors.email}
					<p class="error-note" style="margin:4px 0 0">{fieldErrors.email}</p>
				{/if}
			</div>
			<div class="field wf">
				<label>
					Téléphone
					<input
						class="input-wf"
						type="text"
						bind:value={telephone}
						maxlength="30"
						required
						onblur={() => validateField('telephone')}
						oninput={() => clearFieldErrorIfFixed('telephone')}
					/>
				</label>
				{#if fieldErrors.telephone}
					<p class="error-note" style="margin:4px 0 0">{fieldErrors.telephone}</p>
				{/if}
			</div>
			<div class="field wf">
				<span class="wf-label">Tranche d'âge</span>
				<div class="btn-group">
					{#each TRANCHE_AGE_OPTIONS as opt (opt.value)}
						<button
							type="button"
							class="btn-wf"
							class:primary={trancheAge === opt.value}
							onclick={() => (trancheAge = opt.value)}
						>
							{opt.label}
						</button>
					{/each}
				</div>
				{#if fieldErrors.tranche_age}
					<p class="error-note" style="margin:4px 0 0">{fieldErrors.tranche_age}</p>
				{/if}
			</div>
			<div class="field wf">
				<span class="wf-label">Permis B</span>
				<div class="btn-group">
					<button
						type="button"
						class="btn-wf"
						class:primary={permisB}
						onclick={() => (permisB = true)}
					>
						Oui
					</button>
					<button
						type="button"
						class="btn-wf"
						class:primary={!permisB}
						onclick={() => (permisB = false)}
					>
						Non
					</button>
				</div>
			</div>
			<div class="field wf">
				<span class="wf-label">Taille tee-shirt</span>
				<div class="btn-group">
					{#each TAILLE_TSHIRT_OPTIONS as size (size)}
						<button
							type="button"
							class="btn-wf"
							class:primary={tailleTshirt === size}
							onclick={() => (tailleTshirt = size)}
						>
							{size}
						</button>
					{/each}
				</div>
				{#if fieldErrors.taille_tshirt}
					<p class="error-note" style="margin:4px 0 0">{fieldErrors.taille_tshirt}</p>
				{/if}
			</div>
		</div>
	</div>

	<div class="panel wf" style="margin-bottom:14px">
		<h3>Événements</h3>
		<p class="note" style="margin:4px 0 10px">
			Cocher un ou plusieurs événements : un bloc disponibilités + souhaits apparaît pour chacun, à
			remplir en une seule saisie (une soumission créée par événement coché).
		</p>
		{#if evenementsActifs().length === 0}
			<p class="note">Aucun événement actif — créez-en un dans l'écran Événements d'abord.</p>
		{:else}
			<div class="btn-group">
				{#each evenementsActifs() as ev (ev.id)}
					<button
						type="button"
						class="btn-wf"
						class:primary={!!eventForms[ev.id]}
						onclick={() => toggleEvenement(ev.id)}
					>
						{ev.nom}
					</button>
				{/each}
			</div>
		{/if}
	</div>

	{#each Object.entries(eventForms) as [evenementId, s] (evenementId)}
		{@const ev = evenements.find((e) => e.id === evenementId)}
		<div class="panel wf" style="margin-bottom:14px">
			<div class="screen-head" style="margin-bottom:0">
				<h3>{ev?.nom ?? evenementId}</h3>
				<button
					type="button"
					class="btn-wf danger"
					style="border-style:dashed"
					onclick={() => toggleEvenement(evenementId)}
				>
					Retirer cet événement
				</button>
			</div>

			{#if s.loadingMapping}
				<p class="note" style="margin-top:10px">Chargement des créneaux…</p>
			{:else}
				{@const jours = joursMappes(s.mapping)}
				<h3 style="margin-top:16px; font-size:12.5px">Disponibilités</h3>
				{#if jours.length === 0}
					<p class="note" style="margin:4px 0 0">
						Aucun créneau mappé au formulaire pour cet événement (écran Créneaux, section mapping).
					</p>
				{:else}
					<p class="note" style="margin:4px 0 10px">
						Mêmes cases que le form (matin/midi/aprem/soir selon le jour) ; converties ensuite en
						créneaux applicatifs via le mapping existant.
					</p>
					<div style="overflow-x:auto">
						<table class="req-table">
							<thead>
								<tr>
									<th>Jour</th>
									{#each SLOT_ORDER as slot (slot)}
										<th>{SLOT_LABEL[slot]}</th>
									{/each}
								</tr>
							</thead>
							<tbody>
								{#each jours as jour (jour)}
									{@const slots = slotsDuJour(s.mapping, jour)}
									<tr>
										<td style="text-align:left">{formatJourShort(jour)}</td>
										{#each SLOT_ORDER as slot (slot)}
											<td>
												{#if slots.includes(slot)}
													<input
														class="check-wf"
														type="checkbox"
														checked={!!s.dispos[disporKey(jour, slot)]}
														onchange={() => toggleDispo(evenementId, jour, slot)}
													/>
												{:else}
													<span class="wf-label">—</span>
												{/if}
											</td>
										{/each}
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				{/if}

				<h3 style="margin-top:16px; font-size:12.5px">
					Poste(s) souhaité(s)
					<span class="optional-tag">
						— facultatif, affectable ensuite à la main dans le planning
					</span>
				</h3>
				<div class="field-grid">
					<label class="field check-field wf">
						<input
							class="check-wf"
							type="checkbox"
							checked={s.polyvalent}
							onchange={() => togglePolyvalent(evenementId)}
						/>
						{POLYVALENT_LABEL}
					</label>
					{#each commissions as comm (comm.id)}
						<label class="field check-field wf">
							<input
								class="check-wf"
								type="checkbox"
								checked={!!s.souhaits[souhaitKey(comm.id)]}
								onchange={() => toggleSouhait(evenementId, souhaitKey(comm.id))}
							/>
							<span>
								<span class="color-swatch" style="background:{comm.couleur}"></span>
								{comm.nom}
							</span>
						</label>
						{#each comm.postes as poste (poste.id)}
							<label class="field check-field wf">
								<input
									class="check-wf"
									type="checkbox"
									checked={!!s.souhaits[souhaitKey(comm.id, poste.id)]}
									onchange={() => toggleSouhait(evenementId, souhaitKey(comm.id, poste.id))}
								/>
								{comm.nom} · {poste.nom}
							</label>
						{/each}
					{/each}
				</div>
				<p class="note" style="margin-top:10px">
					Liste = référentiel des commissions actives et de leurs postes (specs.md §3.1/§5.1).
				</p>
			{/if}
		</div>
	{/each}

	{#if error}
		<p role="alert" class="error-note">{error}</p>
	{/if}

	<button
		type="button"
		class="btn-wf primary"
		class:success={saved.active}
		disabled={saving || Object.keys(eventForms).length === 0}
		onclick={submit}
	>
		{saved.active ? '✓ Enregistré' : 'Créer la/les soumission(s)'}
	</button>
{/if}
