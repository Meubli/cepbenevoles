<script lang="ts">
	import { api, ApiError } from '#lib/api';

	type Step = 'email' | 'code';

	let step = $state<Step>('email');
	let email = $state('');
	let code = $state('');
	let sending = $state(false);
	let verifying = $state(false);
	let error = $state<string | null>(null);
	let info = $state<string | null>(null);

	async function requestCode() {
		error = null;
		sending = true;
		try {
			await api.post('/auth/request-code', { email });
			// Always the same message whether the email is a known admin or
			// not (backend never reveals which — specs.md §11.1).
			info = 'Si cette adresse correspond à un compte admin, un code vient de lui être envoyé.';
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
			await api.post('/auth/verify-code', { email, code }, { skipAuthRedirect: true });
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

	function backToEmail() {
		step = 'email';
		code = '';
		error = null;
		info = null;
	}
</script>

<h2>Connexion</h2>

{#if error}
	<p role="alert">{error}</p>
{/if}

{#if step === 'email'}
	<form
		onsubmit={(e) => {
			e.preventDefault();
			void requestCode();
		}}
	>
		<label>
			Email
			<input type="email" bind:value={email} required autocomplete="email" />
		</label>
		<button type="submit" disabled={sending || !email}>
			{sending ? 'Envoi…' : 'Recevoir un code'}
		</button>
	</form>
{:else}
	{#if info}
		<p>{info}</p>
	{/if}
	<form
		onsubmit={(e) => {
			e.preventDefault();
			void verifyCode();
		}}
	>
		<label>
			Code reçu par email
			<input
				type="text"
				inputmode="numeric"
				pattern="[0-9]*"
				bind:value={code}
				required
				autocomplete="one-time-code"
			/>
		</label>
		<button type="submit" disabled={verifying || !code}>
			{verifying ? 'Vérification…' : 'Se connecter'}
		</button>
		<button type="button" onclick={backToEmail}>Changer d'email</button>
	</form>
{/if}
