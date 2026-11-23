<script lang="ts">
	import { api, ApiError } from '#lib/api';

	type Step = 'email' | 'code';

	const CODE_LENGTH = 6;

	function emptyDigits(): string[] {
		return Array.from({ length: CODE_LENGTH }, () => '');
	}

	let step = $state<Step>('email');
	let email = $state('');
	let digits = $state<string[]>(emptyDigits());
	let digitInputs: HTMLInputElement[] = [];
	let sending = $state(false);
	let verifying = $state(false);
	let error = $state<string | null>(null);

	function code(): string {
		return digits.join('');
	}

	async function requestCode() {
		error = null;
		sending = true;
		try {
			await api.post('/auth/request-code', { email });
			digits = emptyDigits();
			step = 'code';
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		} finally {
			sending = false;
		}
	}

	async function verifyCode() {
		error = null;
		verifying = true;
		try {
			await api.post('/auth/verify-code', { email, code: code() }, { skipAuthRedirect: true });
			window.location.href = '/mapping';
		} catch (e) {
			error =
				e instanceof ApiError && e.status === 401
					? 'Code invalide ou expiré.'
					: 'Erreur inattendue';
		} finally {
			verifying = false;
		}
	}

	function onDigitInput(index: number, raw: string) {
		const value = raw.replace(/\D/g, '').slice(-1);
		digits[index] = value;
		if (value && index < CODE_LENGTH - 1) {
			digitInputs[index + 1]?.focus();
		}
	}

	function onDigitKeydown(index: number, e: KeyboardEvent) {
		if (e.key === 'Backspace' && !digits[index] && index > 0) {
			digitInputs[index - 1]?.focus();
		}
	}

	function onDigitPaste(e: ClipboardEvent) {
		const pasted = e.clipboardData?.getData('text').replace(/\D/g, '').slice(0, CODE_LENGTH);
		if (!pasted) return;
		e.preventDefault();
		digits = Array.from({ length: CODE_LENGTH }, (_, i) => pasted[i] ?? '');
		digitInputs[Math.min(pasted.length, CODE_LENGTH - 1)]?.focus();
	}

	function backToEmail() {
		step = 'email';
		digits = emptyDigits();
		error = null;
	}
</script>

<main class="connexion-main">
	<div class="screen-head">
		<h2>Connexion admin</h2>
		<span class="screen-tag">Sans mot de passe</span>
	</div>
	<p class="note">
		Email vérifié contre la liste des admins, puis code à usage unique envoyé par email. Pas de mot
		de passe stocké.
	</p>

	{#if error}
		<p role="alert" class="error-note">{error}</p>
	{/if}

	<div class="login-wrap">
		{#if step === 'email'}
			<div class="panel wf">
				<h3>Étape 1 — email</h3>
				<form
					onsubmit={(e) => {
						e.preventDefault();
						void requestCode();
					}}
				>
					<input
						class="input-wf wf"
						type="email"
						name="email"
						placeholder="prenom.nom@exemple.fr"
						bind:value={email}
						required
						autocomplete="email"
					/>
					<button class="btn-wf primary" type="submit" disabled={sending || !email}>
						{sending ? 'Envoi…' : 'Recevoir le code'}
					</button>
				</form>
			</div>
		{:else}
			<div class="panel wf">
				<h3>Étape 2 — code reçu par email</h3>
				<p class="note" style="margin:4px 0 0">Valable 10 minutes.</p>
				<form
					onsubmit={(e) => {
						e.preventDefault();
						void verifyCode();
					}}
				>
					<div class="code-boxes">
						{#each digits as digit, i (i)}
							<input
								class="code-box wf"
								type="text"
								inputmode="numeric"
								maxlength="1"
								value={digit}
								aria-label={`Chiffre ${i + 1} du code`}
								bind:this={digitInputs[i]}
								oninput={(e) => onDigitInput(i, e.currentTarget.value)}
								onkeydown={(e) => onDigitKeydown(i, e)}
								onpaste={onDigitPaste}
							/>
						{/each}
					</div>
					<button
						class="btn-wf primary"
						type="submit"
						disabled={verifying || code().length < CODE_LENGTH}
					>
						{verifying ? 'Vérification…' : 'Se connecter'}
					</button>
					<button type="button" class="link-btn" onclick={backToEmail}>Renvoyer le code</button>
				</form>
			</div>
		{/if}
	</div>
	<p class="note" style="margin-top:4px">
		Après connexion, la session reste active plusieurs mois sans ressaisie, tant qu'il y a de
		l'activité régulière (données traitées peu sensibles). Déconnexion manuelle possible à tout
		moment.
	</p>
</main>

<style>
	:root {
		--bg: #f3f1ec;
		--surface: #ffffff;
		--ink: #2a2824;
		--muted: #8a8578;
		--line: #c9c4b8;
		--accent: #7a5c3e;
		--accent-weak: #ece4d8;
		--font-display: 'Space Mono', ui-monospace, 'SF Mono', monospace;
		--font-body: 'Inter', system-ui, -apple-system, sans-serif;
	}
	@media (prefers-color-scheme: dark) {
		:root:not([data-theme='light']) {
			--bg: #1c1a17;
			--surface: #262320;
			--ink: #e9e5dc;
			--muted: #8f897a;
			--line: #45403a;
			--accent: #c79a6e;
			--accent-weak: #352c22;
			color-scheme: dark;
		}
	}
	:global(body) {
		margin: 0;
		background: var(--bg);
		color: var(--ink);
		font-family: var(--font-body);
		font-size: 14px;
		line-height: 1.5;
		box-sizing: border-box;
	}
	:global(body *),
	:global(body *::before),
	:global(body *::after) {
		box-sizing: inherit;
	}
	.connexion-main {
		max-width: 1100px;
		margin: 0 auto;
		padding: 24px 16px 60px;
	}
	h2,
	h3 {
		font-family: var(--font-display);
		font-weight: 700;
		margin: 0;
	}
	.screen-head {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		flex-wrap: wrap;
		gap: 8px;
		margin-bottom: 4px;
	}
	.screen-head h2 {
		font-size: 19px;
	}
	.screen-tag {
		font-size: 11.5px;
		color: var(--muted);
		letter-spacing: 0.03em;
	}
	.note {
		color: var(--muted);
		font-size: 12.5px;
		margin: 4px 0 18px;
		max-width: 65ch;
	}
	.error-note {
		color: #9c4a3c;
		font-size: 12.5px;
		margin: 0 0 14px;
	}
	.wf {
		border: 1px dashed var(--line);
		border-radius: 6px;
		background: var(--surface);
	}
	.btn-wf {
		border: 1px solid var(--ink);
		border-radius: 5px;
		padding: 7px 13px;
		font-size: 12.5px;
		font-weight: 600;
		background: var(--surface);
		color: var(--ink);
		white-space: nowrap;
		cursor: pointer;
	}
	.btn-wf.primary {
		background: var(--ink);
		color: var(--bg);
		border-color: var(--ink);
	}
	.btn-wf:disabled {
		opacity: 0.45;
		cursor: not-allowed;
	}
	.link-btn {
		background: none;
		border: none;
		color: var(--muted);
		font-size: 12.5px;
		margin-top: 10px;
		cursor: pointer;
		text-decoration: underline;
		text-underline-offset: 2px;
		padding: 0;
	}
	.login-wrap {
		max-width: 360px;
		margin: 40px auto 0;
		display: flex;
		flex-direction: column;
		gap: 14px;
	}
	.login-wrap .panel {
		text-align: center;
	}
	.panel {
		padding: 16px;
		min-width: 0;
	}
	.panel h3 {
		font-size: 14px;
		margin-bottom: 3px;
	}
	.panel form {
		display: flex;
		flex-direction: column;
	}
	.input-wf {
		height: 28px;
		margin-top: 10px;
		padding: 0 10px;
		font-size: 13px;
		font-family: var(--font-body);
		color: var(--ink);
	}
	.input-wf::placeholder {
		color: var(--muted);
	}
	.panel .btn-wf.primary {
		width: 100%;
		margin-top: 12px;
	}
	.code-boxes {
		display: flex;
		gap: 8px;
		justify-content: center;
		margin: 14px 0;
	}
	.code-box {
		width: 38px;
		height: 44px;
		text-align: center;
		font-family: var(--font-display);
		font-size: 18px;
		color: var(--ink);
	}
</style>
