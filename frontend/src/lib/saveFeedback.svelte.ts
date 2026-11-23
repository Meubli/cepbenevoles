// Micro-feedback after a successful save: a button flips to a transient
// "✓ Enregistré" state for a moment, then reverts — reassures the admin
// that the click actually persisted (network round-trip, no page reload
// to confirm it otherwise). Call `.trigger()` right after the save request
// resolves; never on failure.
export function createSaveFlash(durationMs = 1500) {
	let active = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;

	return {
		get active() {
			return active;
		},
		trigger() {
			active = true;
			clearTimeout(timer);
			timer = setTimeout(() => {
				active = false;
			}, durationMs);
		}
	};
}
