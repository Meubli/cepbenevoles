# Handoff — reprise de session

Dernière mise à jour : 2026-11-22. Dernier commit : `ec34e8f`.

## État général

Repo propre (`git status` vide), commits séparés par lot (demandé par
l'utilisateur, un par morceau de travail indépendant plutôt qu'un seul
commit fourre-tout) :
`66ed12a` (dev compose + bootstrap), `c1f36cc` (stub Sheets), `677f2f0`
(écran connexion + cookie SameSite/Secure), `612b957` (restyle connexion),
`ef8a182` (`SHEET_VALIDATION` + `validate_bloc` + e2e Événements + specs),
`9ae7478` (restyle Événements + shell admin partagé), `fd409d1` (restyle
Mapping), `b665773` (déconnexion + expiration glissante), `e145d2e`
(écran Admins), `ec34e8f` (fix overflow inputs/box-sizing + largeur
shell). Chaque commit a été vérifié individuellement (build + e2e
complet) avant le suivant. Stack :
Rust/Axum/sqlx backend, SvelteKit SPA frontend, Postgres, tests Rust (unitaires +
intégration testcontainers) + Playwright e2e. Voir `docs/specs.md` (fonctionnel) et
`docs/technical-specs.md` (technique) pour les specs à jour — ce fichier ne les
duplique pas, il dit où on en est par rapport à elles.

## Fait (backend + frontend + e2e, testé)

- **Squelette** : Axum sert l'API sous `/api/*` + la SPA buildée en fallback
  (`STATIC_DIR`), `/health` à part. `AuthAdmin` (extractor) donne l'id admin
  connecté pour `created_by`/`updated_by`.
- **Sync Google Sheets** : OAuth2 service account réel (JWT RS256 signé
  maison, `backend/src/google_auth.rs`), `SheetsClient` (encodage URL correct,
  erreurs typées 404/403), parser pur par formulaire (`backend/src/sync/form_2027.rs`,
  hardcodé par année de formulaire — un nouveau form = un nouveau module, pas un
  écran de config), orchestration multi-sheets (`orchestrate.rs`, isole les pannes
  par Sheet), persistance idempotente (`persist.rs`), historique `sync_run`/
  `sync_row_error`. Endpoint manuel `POST /api/sync` + scheduler périodique
  (`SYNC_INTERVAL_SECONDS`, défaut 600s). Vérifié en vrai contre le vrai Google
  (`backend/tests/manual_real_sheet.rs`, `#[ignore]`, besoin `REAL_KEY_PATH`/
  `REAL_SHEET_ID`).
- **Écran Mapping** (`/mapping`) : référentiel Commission/Poste seedé par
  migration (`20260101000004_commissions.sql`), résolution des souhaits
  (`backend/src/mapping/`) en une seule requête SQL (CTEs) pour éviter une race
  condition découverte en review. Libellé "joueur polyvalent" pré-mappé.
- **Écran Événements** (`/evenements`) : CRUD (`backend/src/evenements/`),
  validation `sheet_id` contre la vraie API Sheets (`SheetsClient::check_access`),
  normalisation URL→ID, rejet des URL de Google Forms, gestion `bloc_formulaire`
  (requis si `sheet_id` posé, unique par sheet parmi événements actifs, re-vérifié
  au désarchivage).
- **Middleware correlation-id** (`backend/src/correlation.rs`) : UUID par requête,
  présent dans le span tracing + header + body JSON des 5xx.
- **`docker-compose.dev.yml`** (Postgres/Mailpit/stub Sheets pour dev local,
  ports standard 5432/1025/8025/8089, volume Postgres persistant, `name:
  benefrance-dev` pour ne pas entrer en collision avec `docker-compose.test.yml`
  — `name: benefrance-test` — quand les deux tournent en même temps) +
  **bootstrap premier superadmin** (`BOOTSTRAP_SUPERADMIN_EMAIL`, insert
  idempotent `ON CONFLICT DO NOTHING` au démarrage dans `main.rs`). Voir
  `.env.dev.example`.
- **Stub Sheets branché en dev** (`e2e/sheets-stub/server.mjs`) : sert
  maintenant `/token` (faux échange JWT, sans vérifier la signature) et
  `/v4/spreadsheets/:id/values/:range` avec les vraies données de la fixture
  2027 (`e2e/sheets-stub/fixtures/sheets_responses_form1.json`, copie de
  `backend/tests/fixtures/sheets_responses_form1.json`), peu importe le
  `sheet_id`/range demandé — `check_access` (`/v4/spreadsheets/:id`) répond
  200 pour n'importe quel id. `SHEETS_API_BASE_URL` (nouveau, `config.rs`,
  avant en dur dans `main.rs`) + `dev/service-account-key.dev.json` (réutilise
  la clé RSA de test déjà committée, pas un vrai secret, voir `dev/README.md`)
  pointent dessus. Vérifié bout en bout (JWT signé → `/token` → `check_access`
  + `fetch_values` → 4 lignes parsées) avec un test jetable, supprimé après
  coup — pas de test permanent ajouté.
- **Écran "connexion"** (`frontend/src/routes/connexion/+page.svelte`) :
  formulaire email → code OTP en deux étapes, contre les endpoints existants
  (`/api/auth/request-code`, `/api/auth/verify-code`). `frontend/src/lib/api.ts`
  gagne une option `skipAuthRedirect` pour que la page de connexion elle-même
  puisse recevoir un 401 (mauvais code) sans déclencher la redirection globale
  vers `/connexion` (qui aurait bouclé sur elle-même). **Reprend fidèlement la
  maquette `mockups/wireframes.html` écran s0** : titre/tags, cartes "wf"
  pointillées, étape 1 (email) / étape 2 (6 cases de code séparées avec
  auto-avance/backspace/paste, pas un simple input texte), palette + police
  monospace pour les titres, variante sombre automatique
  (`prefers-color-scheme`). CSS scoping volontaire au composant seul (pas de
  style partagé/topbar) : `/mapping` et `/evenements` restent non stylées pour
  l'instant, décision prise avec l'utilisateur de limiter le restyle à cet
  écran pour l'instant. La page s'enveloppe dans son propre `<main>` local
  (classe scopée `.connexion-main`, pas `:global(main)` — `+layout.svelte` ne
  contient aucun `<main>`, viser `:global(main)` n'accrochait rien et cassait
  l'alignement, bug trouvé en regardant dans le navigateur et corrigé). Seul
  `:global(body)` reste global ; ne fuite pas vers les autres écrans parce
  que toute navigation vers/depuis `/connexion` est un `window.location.href`
  (rechargement complet), jamais un routage SPA — à revisiter si ça change un
  jour. Testé en vrai dans le navigateur (saisie email → code lu dans
  Mailpit → 6 cases remplies en auto-avance → `/mapping`) + e2e
  (`e2e/tests/connexion.spec.ts`, 2 tests : succès + mauvais code reste sur
  place avec message inline).
- **Cookie de session `SameSite=Lax` + `Secure`** (`backend/src/auth.rs`,
  specs.md §11.3) : `Secure` piloté par `session_cookie_secure`
  (`AppState`/`Config`, env `SESSION_COOKIE_SECURE`, défaut `true` — `false`
  dans `.env.dev.example` et `e2e/playwright.config.ts`, le dev/e2e tournant
  en http simple).
- **`SHEET_VALIDATION=required|skip`** (`backend/src/config.rs`,
  `sheet_validation_required`) : sans `SyncContext` configuré,
  l'enregistrement d'un `sheet_id` **échoue** désormais par défaut
  (`SheetIdError::CannotVerify`, 422) au lieu d'être accepté silencieusement —
  comportement demandé explicitement. `SHEET_VALIDATION=skip` restaure
  l'ancien comportement (accepté sans vérification), utilisé uniquement par
  `e2e/playwright.config.ts` (pas de Google configuré en e2e, et les tests
  Événements n'exercent pas la vraie API Sheets).
- **`validate_bloc` ne masque plus une panne DB en "bloc inconnu" 422** :
  `BlocError` gagne un variant `Database` propre (propagé via `?` au lieu
  d'être mappé à la main), routé en 500 par `save_error_response`
  (`backend/src/evenements/endpoint.rs`) au lieu de rejoindre le 422
  générique des autres variants (`Missing`/`Unknown`/`AlreadyUsed`).
- **e2e Événements étendu** (`e2e/tests/evenements.spec.ts`) : sélecteur de
  bloc caché tant qu'aucun `sheet_id` n'est saisi puis visible une fois
  rempli, événement créé avec bloc gardant bien son `sheet_id` ; `sheet_id`
  sans bloc choisi rejeté avec message inline, formulaire conservé (pas
  réinitialisé sur erreur).
- **`docs/specs.md` §3.3 mis à jour** : règle bloc requis si sheet_id posé,
  unicité `(sheet_id, bloc_formulaire)` par événements actifs + re-vérifiée
  au désarchivage, acceptation ID nu ou URL complète + rejet des URL Forms,
  et le nouveau comportement par défaut de `SHEET_VALIDATION` (le texte
  précédent décrivait encore l'ancien "ignoré silencieusement").

- **Écran "Événements" restylé + shell admin partagé** : nouveau groupe de
  routes SvelteKit `frontend/src/routes/(admin)/` (ne change pas les URLs —
  `/mapping` et `/evenements` restent inchangées) avec un `+layout.svelte`
  commun — sidebar `AdminNav` (`frontend/src/lib/AdminNav.svelte`) + panel
  `wf` — et les tokens/classes CSS partagés extraits dans
  `frontend/src/lib/styles/admin.css` (scopés à `.admin-shell`, jamais
  `:root`/`body` global, pour ne pas affecter `/connexion`). La sidebar
  reprend les 10 entrées de la maquette (écran s11) ; seules
  "Événements"/"Mapping form" sont de vrais liens (`resolve()` de
  `$app/paths`, imposé par l'eslint rule `svelte/no-navigation-without-resolve`),
  les 8 autres s'affichent désactivées (pas de lien mort, pas un mensonge sur
  ce qui marche) — décision prise avec l'utilisateur. `/evenements` reprend
  la liste en `comm-row`/`comm-main`/`comm-meta` + formulaires en
  `field-grid` de l'écran s11. e2e Événements adapté : la ligne d'événement
  n'est plus un `<tr>` (liste en divs, pas une table) —
  `e2e/tests/evenements.spec.ts` cible maintenant
  `[data-testid="evenement-row"]`. Vérifié dans le navigateur (création
  d'événement + nav cliquée vers `/mapping`) et e2e (8/8 verts).
- **Écran Mapping restylé** (écran s10) : barre recherche + filtre
  "Tous"/"Non mappés (N)" (purement client, filtre `rows` déjà chargées, pas
  de nouvel endpoint), liste en `comm-row` (bordure teintée `--accent` +
  pill "non mappé" pour les libellés en attente, cf. mockup), formulaire
  d'association en `field-grid` sous la ligne concernée. Écart assumé par
  rapport à la maquette : pas de boutons de commission colorés par couleur
  propre (`--c-accueil` etc.) — ces couleurs sont décrites comme assignées
  depuis l'écran Commissions (§5.4, pas encore construit) et l'API
  `/commissions` ne renvoie aucune couleur aujourd'hui ; les boutons
  commission/poste restent de simples `btn-wf`. `e2e/tests/mapping.spec.ts`
  adapté (`<tr>` → `[data-testid="mapping-row"]`, même raison que
  Événements). Vérifié en e2e (8/8) ; vérifié dans le navigateur à vide (DB
  dev sans libellé synchronisé pour l'instant).
- **Déconnexion manuelle + expiration glissante de session** (specs.md
  §11.3, gaps 6/7 ci-dessous) :
  - `POST /api/auth/logout` (`backend/src/auth.rs`) — supprime la ligne
    `sessions` côté serveur (pas juste le cookie côté client) et vide le
    cookie. Idempotent (pas de cookie / session déjà morte = 204 quand
    même).
  - `renew_session` (nouvelle fonction privée partagée par `session_valid`,
    `require_session` et l'extractor `AuthAdmin`) : chaque requête
    authentifiée valide ET renouvelle `expires_at`/`last_activity_at` en une
    seule requête SQL atomique (`UPDATE ... WHERE expires_at > now()
    RETURNING admin_id`) — une session active ne se rapproche plus jamais de
    ses 180 jours, et le garde `WHERE expires_at > now()` empêche de
    ressusciter une session déjà expirée.
  - `GET /api/session` renvoie maintenant `{email, is_superadmin}` (avant :
    juste 200/401) pour que la topbar admin affiche l'identité.
  - Topbar ajoutée au shell admin (`(admin)/+layout.svelte`) : marque +
    email + badge `superadmin` + bouton "Déconnexion" (mockup s0-s11,
    `.session-box`). Clic → `POST /auth/logout` → `window.location.href =
    '/connexion'` (rechargement complet, même convention que le reste).
  - Testé : 5 tests Rust (`backend/tests/auth_endpoint.rs`, nouveau fichier
    — identité renvoyée, logout détruit bien la session en base, logout
    sans cookie ne casse rien, une session proche d'expirer est repoussée
    de ~180j, une session déjà expirée n'est jamais ressuscitée) + 1 test
    e2e (`e2e/tests/logout.spec.ts` — clic topbar, redirection, re-visite
    de `/evenements` après coup redirige bien vers `/connexion`, la session
    est bien morte côté serveur pas juste localement).
- **Écran "Admins"** (specs.md §11.4, wireframe s8, `/admins`) : ajout,
  suppression, promotion superadmin. Réservé aux superadmins côté backend
  (nouvel extractor `auth::SuperAdmin`, 403 sinon — `AuthAdmin` seul ne
  suffit pas) et côté frontend (ajouté comme vrai lien dans `AdminNav`).
  - **Schéma complété** : `Admin.nom`/`Admin.prenom` existaient dans
    specs.md §8 mais jamais dans une migration — ajoutés, nullables (migration
    `20260101000005_admin_names.sql`) puisque l'admin bootstrap
    (`BOOTSTRAP_SUPERADMIN_EMAIL`) n'en a pas.
  - **FK corrigées pour permettre un vrai `DELETE`** (migration
    `20260101000006_admin_delete_fk.sql`) : avant cette migration, retirer
    un admin qui avait ne serait-ce qu'une session active, ou qui avait un
    jour créé/modifié un événement/une commission/un poste/un mapping,
    aurait échoué sur une contrainte de clé étrangère. `sessions.admin_id`
    passe en `ON DELETE CASCADE` (une session n'a pas de sens sans son
    admin) ; les colonnes d'audit `created_by`/`updated_by` (déjà
    nullables) passent en `ON DELETE SET NULL` (la ligne métier doit
    survivre, seule l'attribution se perd).
  - **Garde-fous** (`backend/src/admins/mod.rs`) : impossible de se retirer
    soi-même (verrouillage), impossible de retirer ou rétrograder le
    dernier superadmin restant — sinon plus personne ne peut gérer les
    admins. Email dupliqué → 422 explicite (pas une 500 sur la contrainte
    unique).
  - UI (`(admin)/admins/+page.svelte`) : liste en `comm-row`/`comm-main`
    (réutilise les classes déjà posées pour Événements/Mapping, pas de CSS
    neuve), ligne de l'admin connecté marquée "connecté" sans bouton
    d'action (comme la maquette), formulaire d'ajout en `field-grid`.
    **Interprétation assumée** : la maquette ne précise pas ce que "connecté"
    veut dire précisément — lu ici comme "c'est votre propre compte" (email
    de la ligne == email de `/session`), pas "a une session active en ce
    moment" (qu'on ne pourrait de toute façon pas distinguer sans table de
    présence temps réel). À corriger si l'intention était différente.
  - Testé : 8 tests Rust (`backend/tests/admins_endpoint.rs` — autorisation
    403 pour un admin non-superadmin, création + liste, email dupliqué,
    promotion, refus de rétrograder le dernier superadmin, refus de
    s'auto-retirer, refus de retirer le dernier superadmin par quelqu'un
    d'autre, suppression détruit bien les sessions de la cible) + 2 tests
    e2e (`e2e/tests/admins.spec.ts` — cycle complet créer/promouvoir/retirer
    + un admin non-superadmin reçoit 403 sur `/api/admins`). Vérifié dans
    le navigateur (ajout + promotion + retrait réels).
- **Deux bugs CSS corrigés sur `/evenements`** (trouvés dans le navigateur
  après coup, par l'utilisateur) :
  - Formulaire d'édition inline cassé par la mise en page : il vivait dans
    le `.comm-row` flex de la ligne (`justify-content: space-between`), qui
    comprimait le `field-grid` contre les boutons Enregistrer/Annuler au
    lieu de lui laisser toute la largeur. Déplacé en bloc pleine largeur
    sous la ligne (même pattern que "Nouvel événement"), comme le fait
    réellement la maquette s11 (le bloc "Modifier — …" y est déjà séparé
    de la liste, pas incrusté dans une ligne).
  - **`admin.css` n'avait jamais posé `box-sizing: border-box`** (présent
    dans la maquette via un `*{box-sizing:border-box}` global, jamais
    repris) : `.input-wf{width:100%}` + son `padding`/`border` s'ajoutaient
    par-dessus ce 100 %, débordant visuellement de la carte `.field wf` —
    affectait **tous** les champs du shell admin (Événements, Mapping,
    Admins), pas seulement Événements. Corrigé en posant
    `box-sizing: border-box` sur `.admin-shell` et tous ses descendants ;
    même filet de sécurité ajouté à `/connexion` par précaution (son
    `.input-wf` n'a pas `width:100%` donc n'était pas cassé, mais même
    classe de bug possible).
  - `admin-main`/`topbar-inner` élargis à 1600px (l'utilisateur les
    trouvait trop étroits sur grand écran) — avant : 1100px, repris tel
    quel de la maquette sans l'adapter aux vrais écrans utilisés.

Compteurs : ~83 tests Rust (unitaires + intégration), 11 tests Playwright
e2e, tous verts au dernier run (pas de nouveau test pour ce lot — pur CSS,
déjà couvert fonctionnellement par les tests existants).

## Pas fait / connu cassé ou absent

1. ~~`docker-compose.dev.yml` + bootstrap superadmin~~ — **fait** (voir plus bas).
2. ~~`validate_sheet_id` skip silencieux sans `SyncContext`~~ — **fait**
   (`SHEET_VALIDATION=required|skip`, voir plus bas).
3. ~~`validate_bloc` masquait une erreur DB en "bloc inconnu" (422)~~ —
   **fait** (voir plus bas).
4. ~~e2e Événements incomplet (sélecteur de bloc, sheet_id sans bloc)~~ —
   **fait** (voir plus bas).
5. **Rate limiting** (specs §11.2) : toujours pas implémenté. Décision prise :
   stocké en DB (pas Redis/Memcached), mais aucune table ni code pour l'instant.
   Point en suspens signalé par l'advisor : quel header IP faire confiance
   derrière le reverse proxy (`X-Forwarded-For`), pas tranché.
6. ~~Pas de déconnexion~~ — **fait** (voir "Fait" : `POST /auth/logout` +
   bouton topbar du shell admin).
7. ~~Pas d'expiration glissante~~ — **fait** (voir "Fait" : `renew_session`,
   partagée par `session_valid`/`require_session`/`AuthAdmin`).
8. **Mapping scopé globalement** (pas par `sheet_id`) — décision actée comme
   acceptable tant qu'un seul formulaire existe ; à revoir si un 2ème formulaire/Sheet
   apparaît un jour (cf. specs.md §3.2, "point ouvert").
9. **Écran Commissions** (créer/éditer commissions et postes à la main) —
   n'existe pas, le référentiel n'est modifiable que par migration SQL pour
   l'instant.
10. ~~`/evenements` et `/mapping` ne reprenaient pas la maquette~~ — **fait**
    (voir plus bas) : shell admin partagé + sidebar nav, les deux écrans
    restylés (s11/s10). Écart assumé sur Mapping : pas de boutons
    commission colorés par couleur propre (couleur pas encore renvoyée par
    l'API, cf. "Fait" ci-dessus).

Toutes les pages existantes (connexion, événements, mapping, admins) sont
restylées sur la maquette. Reste, côté maquette : les écrans qui
n'existent pas encore dans l'appli du tout (Besoins, Créneaux,
Commissions, Bénévoles, Synchro form, Générer planning, Responsables) —
pas un restyle, une construction d'écran complète, hors scope ici.

## Prochaines étapes, dans l'ordre discuté avec l'utilisateur

1. ~~`docker-compose.dev.yml` + bootstrap premier superadmin~~ — fait.
2. ~~Brancher le stub Sheets en dev~~ — fait (voir plus bas).
3. ~~Écran "connexion" (page SvelteKit login + cookie SameSite/Secure)~~ — fait
   (voir plus bas).
4. ~~Petites corrections (`SHEET_VALIDATION`, `validate_bloc`, e2e
   Événements)~~ — fait (voir plus bas).
5. ~~Restyle écrans Événements + Mapping (+ shell admin partagé)~~ — fait
   (voir plus bas).
6. ~~Déconnexion + expiration glissante~~ — fait (voir plus bas).
7. Rate limiting (DB, décider header IP) — seul point ouvert restant de
   cette liste.

## Commandes utiles

```bash
# DB + mailpit + stub pour dev local (Postgres sur :5432, Mailpit UI :8025)
docker compose -f docker-compose.dev.yml up -d
docker compose -f docker-compose.dev.yml down -v   # reset complet (perd les données dev)

# Lancer le backend en dev (après avoir copié .env.dev.example -> .env.dev et
# adapté BOOTSTRAP_SUPERADMIN_EMAIL). SQLX_OFFLINE=true dans .env.dev.example
# est nécessaire sur un volume Postgres tout neuf : les macros sqlx::query!
# vérifient les colonnes contre une DB déjà migrée à la COMPILATION, alors
# que les migrations ne tournent qu'au démarrage du binaire (poule/oeuf sur
# un volume vide) ; avec SQLX_OFFLINE=true la compilation utilise le cache
# commité backend/.sqlx/ à la place, et sqlx::migrate! (dans main.rs) migre
# quand même la DB normalement au lancement. Sync pointe sur le stub Sheets
# par défaut (GOOGLE_SERVICE_ACCOUNT_KEY_PATH + SHEETS_API_BASE_URL dans
# .env.dev.example) ; n'importe quel sheet_id renvoie la fixture 2027.
source .env.dev && cargo run --manifest-path backend/Cargo.toml

# Frontend en dev (proxy /api -> :3000, cf. frontend/vite.config.ts)
cd frontend && npm run dev   # http://localhost:5173/connexion

# Lire les codes OTP envoyés en dev/e2e (Mailpit, pas une vraie boîte mail)
# http://127.0.0.1:8025 (dev, docker-compose.dev.yml)
# http://127.0.0.1:18025 (test/e2e, docker-compose.test.yml)

# DB + mailpit + stub pour tests/e2e
docker compose -f docker-compose.test.yml up -d
docker compose -f docker-compose.test.yml down -v   # reset complet

# Tests Rust (lib = unitaires, pas besoin de DB)
cargo test --manifest-path backend/Cargo.toml --lib

# Tests Rust intégration (besoin docker-compose.test.yml up, Postgres sur :55433)
SQLX_OFFLINE=true cargo test --manifest-path backend/Cargo.toml \
  --test smoke --test persist --test orchestrate --test sync_endpoint \
  --test mapping --test mapping_endpoint --test evenements --test evenements_endpoint

# fmt + clippy (guardrail CI)
cargo fmt --manifest-path backend/Cargo.toml
SQLX_OFFLINE=true cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings

# Frontend
cd frontend && npm run check && npm run lint && npm run build

# e2e (nécessite backend release build + frontend build + docker-compose.test.yml up)
cargo build --release --manifest-path backend/Cargo.toml
(cd frontend && npm run build)
docker compose -f docker-compose.test.yml up -d
(cd e2e && npx playwright test)

# Test manuel contre le vrai Google (jamais en CI)
REAL_KEY_PATH=/home/nico/Downloads/cepbenevoles-bb0ce64bec25.json \
REAL_SHEET_ID=1VanRtLnS2lF5XpUtUpjyv9MX2lQh7EPcAZCIPsm3jsc \
cargo test --manifest-path backend/Cargo.toml --test manual_real_sheet -- --ignored --nocapture
```

Après chaque modif backend qui ajoute un `sqlx::query!`/`query_as!` macro (pas
`sqlx::query(...)` runtime) : relancer `cargo sqlx prepare` contre une DB migrée
et commiter `backend/.sqlx/`, sinon le guardrail CI `cargo sqlx prepare --check`
casse.

## Fichiers clé pour s'orienter vite

- `backend/src/lib.rs` — assemblage du routeur, `AppState`, `SyncContext`.
- `backend/src/sync/` — parsing form + orchestration + persistance.
- `backend/src/mapping/` — résolution souhaits + écran Mapping.
- `backend/src/evenements/` — CRUD Événements.
- `backend/src/auth.rs` — OTP login + `AuthAdmin` extractor.
- `backend/migrations/` — tout le schéma, dans l'ordre chronologique des commits.
- `frontend/src/lib/api.ts` — wrapper fetch partagé par tous les écrans.
- `frontend/src/routes/(admin)/` — groupe de routes partageant le shell admin
  (`+layout.svelte` + `#lib/AdminNav.svelte` + `#lib/styles/admin.css`) ;
  contient `(admin)/mapping/+page.svelte` et `(admin)/evenements/+page.svelte`
  (le groupe `(admin)` ne change pas les URLs : toujours `/mapping` et
  `/evenements`).
- `frontend/src/routes/connexion/+page.svelte` — écran de connexion, en
  dehors du groupe `(admin)` (pas connecté → pas de sidebar), styles scopés
  au composant lui-même, pas partagés avec `admin.css`.
- `e2e/tests/` — un spec par écran, pattern : email admin dédié **par test**,
  pas seulement par spec (évite une race Mailpit "dernier message" + une race
  sur "challenge actif le plus récent" côté `verify_code` entre tests tournant
  en parallèle dans le même fichier — `connexion.spec.ts` en a deux).
- `dev/` — clé de service account factice (réutilise la clé RSA de test,
  `dev/README.md`) pour que la sync pointe sur `sheets-stub` en dev.
- `docker-compose.test.yml` — Postgres/Mailpit/stub Sheets, utilisé par les
  tests d'intégration ET les e2e (pas un vrai environnement de dev confortable).
