# Handoff — reprise de session

Dernière mise à jour : 2026-11-23. Dernier commit : `bbcb4b9` — travail
décrit ci-dessous (écrans Créneaux + Besoins + Responsables) pas encore
commité.

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
  d'association en `field-grid` sous la ligne concernée. `e2e/tests/mapping.spec.ts`
  adapté (`<tr>` → `[data-testid="mapping-row"]`, même raison que
  Événements). Vérifié en e2e (8/8) ; vérifié dans le navigateur à vide (DB
  dev sans libellé synchronisé pour l'instant).
  - **Boutons commission colorés** (demandé par l'utilisateur après coup) :
    l'écart ci-dessus (plus de couleur propre, `/commissions` ne la
    renvoyait pas) est corrigé — `couleur` (déjà en base depuis la
    migration seed, jamais exposée) ajoutée à `CommissionRow`/`CommissionDto`
    (`backend/src/mapping/endpoint.rs`), assertion ajoutée dans
    `mapping_endpoint.rs`. Le formulaire Associer/Modifier
    (`(admin)/mapping/+page.svelte`) remplace ses deux `<select>`
    Commission/Poste par des groupes de boutons (`.btn-group`, pas encore
    porté dans `admin.css` malgré son usage déjà existant pour le filtre
    Tous/Non mappés — ajouté au passage), chaque bouton commission avec
    `border-left: 4px solid {couleur}` comme la maquette s10 (pas de
    classes `.c-accueil` etc. codées en dur : la couleur vient de l'API,
    pas d'un jeu de classes fixe). **Écart assumé** : "Bénévole polyvalent"
    reste un bouton simple sans couleur en tête de la liste commission —
    la maquette ne le montre pas du tout dans ce formulaire (un libellé
    polyvalent y est déjà pré-mappé sans bouton Modifier), mais l'app
    permet d'en mapper un nouveau si besoin, capacité gardée.
    `e2e/tests/mapping.spec.ts` adapté (`getByRole('combobox')` →
    `getByRole('button', { name: 'Accueil', exact: true })`). Vérifié en
    e2e (11/11) et dans le navigateur (bordures colorées, sélection
    Plateau fait bien apparaître ses postes).
- **Écran "Synchro form"** (specs.md §7.3, wireframe s6, `/synchro`) :
  périmètre volontairement réduit par rapport à la maquette complète
  — décidé avec l'utilisateur plutôt que de construire doublons/permis
  B/envoi email d'un coup (aucune détection de doublon, aucun envoi email
  n'existe encore côté backend).
  - **Nouveau** `GET /api/sync/status` (`backend/src/sync/endpoint.rs`) :
    une ligne par `sheet_id` distinct utilisé par un événement actif (CTE
    SQL : événements/noms groupés par sheet, compteur de soumissions par
    sheet, dernier `sync_run` par sheet via `DISTINCT ON`). Un sheet
    jamais synchronisé apparaît quand même, tous les `last_*` à null.
  - UI (`(admin)/synchro/+page.svelte`) : un bloc par Sheet (compteur de
    soumissions, +N depuis la dernière synchro, date de dernière synchro,
    erreur de dernière synchro le cas échéant) + bouton global
    "Synchroniser maintenant" (`POST /api/sync`, existant — synchronise
    toutes les Sheets configurées d'un coup, pas par Sheet contrairement à
    la maquette qui a un bouton par ligne) + bloc compteur de libellés non
    mappés avec lien vers `/mapping` (réutilise `GET /api/mapping` déjà
    existant, pas de nouvel endpoint pour ce compteur). Classes CSS
    `.sync-status`/`.stat-wf`/`.stat-num`/`.stat-lbl` portées de la
    maquette dans `admin.css` (n'existaient pas encore). `AdminNav`:
    "Synchro form" devient un vrai lien (resolve `/(admin)/synchro`),
    retiré de la liste des entrées désactivées.
  - Testé : 2 tests Rust (`backend/tests/sync_endpoint.rs` — session
    requise, liste une Sheet configurée avec son dernier run) + vérifié à
    la main dans le navigateur (événement existant pointé vers la Sheet
    stub, synchro déclenchée depuis l'écran, compteurs mis à jour, lien
    vers Mapping suivi). Pas de test e2e Playwright dédié pour ce lot.
  - **Non fait** (hors scope choisi pour ce lot) : détection de doublons
    potentiels, avertissement "Transport sans permis B", envoi des
    plannings par email — les trois blocs du bas de la maquette s6.
- **Logs de debug** (demandé par l'utilisateur, pas lié à un écran) :
  - `tracing_subscriber` initialisé avec un `EnvFilter` par défaut
    `benefrance_backend=debug,tower_http=debug,info` (`backend/src/main.rs`)
    au lieu d'aucun filtre — `RUST_LOG`, s'il est positionné, prend
    toujours le dessus.
  - `TraceLayer::new_for_http()` ajouté au routeur (`backend/src/lib.rs`),
    imbriqué **dans** le span `correlation_id` (ordre des `.layer()`
    volontaire — sinon les logs de `TraceLayer` n'auraient pas porté le
    `correlation_id`) : log debug méthode/URI/statut/latence par requête,
    gratuit, sans toucher aux handlers.
  - Quelques `tracing::debug!` ciblés ajoutés à la main sur les chemins
    chauds : sync (`orchestrate.rs` — compte de sheets, lignes fetchées,
    parsées, par sheet ; `main.rs` — tick du scheduler) et auth
    (`auth.rs` — email connu/inconnu sur `request_code`, raison d'un 401
    sur `verify_code`, session créée, cookie absent/invalide sur
    `renew_session`). Pas un log par ligne de code — ciblé sur les points
    où "pourquoi ça n'a pas marché" se pose en pratique.
  - `tracing-subscriber` gagne la feature `env-filter` (`backend/Cargo.toml`,
    répercuté sur `Cargo.lock`) — nécessaire pour `EnvFilter`.
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

- **Écran "Commissions"** (specs.md §5, wireframe s5, `/commissions`) :
  référentiel jusque-là modifiable uniquement par migration SQL. Périmètre
  choisi avec l'utilisateur : CRUD complet (créer/modifier une commission,
  ajouter/retirer des postes), **sans** réordonnancement des postes (la
  table `poste` n'a pas de colonne d'ordre) et **sans** toggle
  actif/inactif (la colonne `actif` reste migration-only, comme avant).
  - **Nouveau module** `backend/src/commissions/` (`mod.rs` logique +
    `endpoint.rs` HTTP) — la liste `GET /commissions` (déjà utilisée par
    l'écran Mapping) a été **déplacée** depuis `mapping/endpoint.rs` ici,
    enrichie de `prerequis_libre_texte`/`grille_creneaux`/`besoin_chiffre`
    (en plus de `couleur`, ajoutée au lot précédent) : source unique pour
    les deux écrans. `POST /commissions` (création), `PATCH
    /commissions/:id` (modification), `POST /commissions/:id/postes`
    (ajout d'un poste), `DELETE /commissions/:id/postes/:poste_id`
    (suppression).
  - **Validations** : nom non vide, `grille_creneaux` ∈
    {generale,propre}, `couleur` au format `#rrggbb`, nom de commission
    unique (422 explicite sur la contrainte unique, même pattern que
    l'email admin dupliqué). **Désactiver "a des postes"** sur une
    commission qui a encore des postes est rejeté (422) — pas de bouton
    "retirer tous les postes en un clic" comme la maquette (admin retire
    les postes un par un, puis bascule le toggle) : simplification assumée
    avec l'utilisateur pour ce lot. **Supprimer un poste déjà référencé**
    (mapping ou souhait existant) est rejeté (422, "ce poste est déjà
    utilisé...") plutôt que de gérer une cascade — la contrainte FK
    (`ON DELETE` par défaut = `NO ACTION`) fait le travail, juste
    traduite en message explicite côté API.
  - UI (`(admin)/commissions/+page.svelte`) : liste en `comm-row` (pastille
    couleur, liste des postes le cas échéant, badges grille
    générale/propre et sans besoin chiffré, cf. maquette), formulaires
    Modifier/Nouvelle commission en `field-grid` avec boutons
    Oui/Non·Générale/Propre (pas de `<select>`, même esprit que les
    boutons commission du Mapping) et un swatch + bouton "Changer" pour la
    couleur (couleur aléatoire générée **côté client** — HSL→hex, pas
    d'appel serveur — dès l'ouverture du formulaire de création, modifiable
    avant tout enregistrement, cf. specs.md §5). Panneau "Gérer les
    postes" séparé (ajout/suppression, pas de drag-and-drop). Classes CSS
    `.grille-badge`/`.color-swatch`/`.poste-list`/`.poste-item`/
    `.poste-edit-row`/`.poste-drag` portées de la maquette dans
    `admin.css` (aucune n'existait encore). Nouveau lien réel dans
    `AdminNav` (`resolve('/(admin)/commissions')`), retiré de la liste des
    entrées désactivées.
  - Testé : 10 tests Rust (`backend/tests/commissions_endpoint.rs` —
    session requise, création+liste, nom vide/dupliqué/couleur invalide
    rejetés, modification d'un id inconnu = 404, modification change bien
    les champs, désactivation de "a des postes" avec postes existants
    rejetée, ajout+suppression d'un poste, suppression d'un poste
    référencé par un mapping rejetée) + vérifié dans le navigateur
    (création d'une commission avec couleur aléatoire, ajout puis
    suppression d'un poste). e2e existants (mapping, 11/11) toujours
    verts après le déplacement de `GET /commissions`.
  - **Formulaire "Nouvelle commission" déplacé au-dessus de la liste** (au
    lieu d'après, où il fallait scroller toute la liste pour le voir —
    signalé par l'utilisateur) : même correction répercutée dans
    `mockups/wireframes.html` (écran s5) et dans l'artifact Claude.ai qui
    en est une copie publiée (republié après édition directe du fichier
    source sauvegardé de l'artifact).

- **Websocket de rafraîchissement live, généralisé à tous les écrans
  admin** (technical-specs.md §10, demandé explicitement par l'utilisateur
  — la spec d'origine ne couvrait que l'écran Planning, pas encore
  construit) :
  - **Nouveau module `backend/src/live.rs`** : `AppState` gagne un champ
    `live: tokio::sync::broadcast::Sender<String>`. Un middleware
    (`emit_live_event`), posé sur `api_router()` dans `app()` (pas besoin
    d'y toucher route par route), diffuse le premier segment du chemin
    (`commissions`, `mapping`, `evenements`, `sync`, `admins`) après
    **toute requête non-GET réussie (2xx)** — `/auth/*` exclu. `GET
    /api/ws` (nouvelle route) upgrade une connexion **authentifiée**
    (`AuthAdmin`, comme toute autre route) et stream les tags reçus ;
    ping toutes les 30s pour détecter une socket morte / éviter qu'un
    reverse proxy la coupe pour inactivité. Flux **push-only, sans
    donnée** : jamais l'objet modifié lui-même, juste son tag de
    ressource — le client refait l'appel REST habituel. Le **scheduler de
    sync** (`main.rs::spawn_scheduler`, qui écrit en base hors requête
    HTTP donc hors middleware) émet aussi `"sync"` lui-même après chaque
    exécution.
  - `AppState` gagnant un champ obligatoire, les 6 `base_state()` des
    fichiers de test ont dû être mis à jour (mécanique, aucun changement
    de comportement testé).
  - **Client** `frontend/src/lib/live.ts` : un seul websocket partagé par
    onglet (ouvert à la première page qui s'y abonne via `onLive(tags,
    cb)`), reconnexion avec backoff exponentiel (max 15s), full-resync
    (`"*"`) après une reconnexion (les événements manqués pendant la
    coupure ne sont pas rejoués). Chaque écran (`(admin)/commissions`,
    `mapping`, `synchro`, `evenements`, `admins`) s'abonne aux tags qui le
    concernent (ex. Mapping écoute aussi `commissions`/`sync` : un
    renommage de commission ou une nouvelle synchro changent ce qu'il
    affiche) et rappelle son `refresh()`.
  - **Refonte nécessaire de chaque écran** : la fonction de chargement
    existante (`load()`) a dû être scindée en `load()` (premier affichage
    — bascule `loading`, remet `error` à zéro) et `refresh()` (rappelée
    par un tick live **et** par `load()`) qui ne touche **ni** `loading`
    (démonterait un formulaire Modifier/Nouveau en cours de saisie), **ni**
    `error` sur un succès (effacerait un message de validation qu'une
    action locale venait de poser). Cette deuxième règle n'était pas
    évidente a priori : trouvée via un **vrai flake e2e** (6 workers
    Playwright tapant le même backend en parallèle — le tick live déclenché
    par la mutation d'un autre test effaçait le message d'erreur du test en
    cours).
  - `vite.config.ts` : le proxy `/api` passe de la forme raccourcie
    (string) à `{ target, ws: true }` — la forme raccourcie ne proxie que
    le HTTP classique, pas l'upgrade websocket ; sans ça `/api/ws`
    échouait silencieusement en dev (`:5173`) tout en fonctionnant en
    prod/e2e (le même binaire Axum sert tout, pas de proxy).
  - Testé : 4 tests Rust (`backend/tests/live_endpoint.rs` — `/ws` exige
    une session, une écriture réussie diffuse son tag, une écriture 422
    n'émet rien, un GET n'émet rien) + nouveau e2e
    (`e2e/tests/live.spec.ts`, utilisant `openTwoAdminContexts`,
    déjà présent dans `e2e/tests/helpers/multi-admin.ts` avec un
    commentaire anticipant exactement ce scénario) : deux contextes
    authentifiés distincts, l'un crée une commission, l'autre (jamais
    rechargé) la voit apparaître via le seul websocket. Suite e2e complète
    (12/12) rejouée 3 fois de suite pour écarter un flake. Vérifié aussi à
    la main dans deux onglets Chrome via le vrai proxy Vite dev (`:5173`,
    pas seulement le binaire de prod/e2e).
  - **Connu non géré** : édition concurrente du même élément = dernier
    enregistrement gagne (pas de fusion) ; une socket ouverte n'est pas
    fermée à la déconnexion/suppression de l'admin (risque faible, elle ne
    porte aucune donnée) ; configuration du reverse proxy pour transmettre
    l'en-tête `Upgrade` en prod — jamais vérifiée ici, à faire avant un
    vrai déploiement derrière un reverse proxy.

- **Bug trouvé en live par l'utilisateur** : un nom de commission très long
  (chaîne continue, sans espace) cassait la mise en page de l'écran
  Commissions — débordait de `.comm-row` au lieu de passer à la ligne,
  chevauchant le bouton "Modifier". Même risque sur Admins/Événements
  (classes `.comm-row`/`.comm-main` partagées) et sur les titres "Modifier
  — commission …"/"Nouvelle commission" (`<h3>` sans contrainte de
  largeur).
  - **Corrigé des deux côtés** :
    - CSS (`admin.css`) : `overflow-wrap: anywhere` sur `.comm-main` et sur
      `.admin-shell h3` — un mot sans espace passe désormais à la ligne au
      lieu de déborder.
    - **Limites de longueur ajoutées au back** (nouvelles constantes
      publiques `NOM_MAX_LEN`/`PREREQUIS_MAX_LEN`/`LIEU_MAX_LEN`/
      `EMAIL_MAX_LEN`, un nouveau variant d'erreur par champ, 422 comme les
      autres validations) : `Commission.nom` 100, `prerequis_libre_texte`
      500, `Poste.nom` 100 (`backend/src/commissions/mod.rs`) ;
      `Evenement.nom`/`lieu` 200 (`backend/src/evenements/mod.rs`) ;
      `Admin.nom`/`prenom` 100, `email` 255 (`backend/src/admins/mod.rs`).
      La CSS seule aurait suffi pour l'affichage, mais la limite doit
      exister côté serveur (pas que `maxlength` HTML, contournable) —
      demandé explicitement par l'utilisateur ("dans le back").
    - `maxlength` HTML ajouté aux champs correspondants (Commissions,
      Admins, Événements) — confort UI, la vraie limite reste le 422
      serveur.
  - Testé : 6 nouveaux tests Rust (3 dans `commissions_endpoint.rs` — nom/
    prérequis/poste trop longs —, 1 dans `admins_endpoint.rs`, 2 dans
    `evenements.rs`) + vérifié dans le navigateur (le nom très long déjà
    créé par l'utilisateur passe maintenant à la ligne proprement).

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
9. ~~Écran Commissions (créer/éditer commissions et postes à la main)~~ —
   **fait** (voir plus bas).
10. ~~`/evenements` et `/mapping` ne reprenaient pas la maquette~~ — **fait**
    (voir plus bas) : shell admin partagé + sidebar nav, les deux écrans
    restylés (s11/s10). Écart assumé sur Mapping : pas de boutons
    commission colorés par couleur propre (couleur pas encore renvoyée par
    l'API, cf. "Fait" ci-dessus).

Toutes les pages existantes (connexion, événements, mapping, admins,
synchro, commissions, créneaux, besoins, responsables) sont restylées sur
la maquette ou construites dessus. Reste, côté maquette : les écrans qui
n'existent pas encore dans l'appli du tout (Bénévoles, Générer planning) —
pas un restyle, une construction d'écran complète, hors scope ici.

- **Écran Créneaux** (`/creneaux`, specs.md §4.1/§4.2/§4.3) : périmètre
  choisi avec l'utilisateur, les volets proposés — gabarit de créneaux
  applicatifs par jour/événement (grille générale), grilles propres par
  commission (+ duplication d'une grille existante comme point de
  départ), mapping créneau form → créneaux applicatifs — tous construits.
  Les bornes matin/midi/aprem/soir d'origine (§4.2 point 4) ont été
  retirées par la suite (colonnes `born_fin_matin/midi/aprem` sur
  `evenement`, migration `20260101000011_drop_bornes.sql`) : elles ne
  servaient qu'aux règles "non-enchaînement midi/soir", devenues caduques
  avec les créneaux repas (§4.4 ci-dessous), et n'étaient utilisées par
  rien d'autre.
  - **Backend** : nouveau module `backend/src/creneaux/` (domaine +
    `endpoint.rs`), migration `20260101000007_creneaux.sql` (tables
    `creneau_applicatif`, `creneau_form_mapping`). Routes sous
    `/api/creneaux/*` (même préfixe partout → un seul tag live-refresh
    `"creneaux"`). Pas de macro `sqlx::query!`/`query_as!` utilisée ici
    (requêtes runtime) : aucun impact sur `backend/.sqlx/`.
  - **Frontend** (`frontend/src/routes/(admin)/creneaux/+page.svelte`) :
    sélecteur d'événement, les quatre sections en une seule page. Nav
    (`AdminNav.svelte`) : lien activé, retiré de la liste des écrans
    désactivés.
  - **Vérifié** : build + clippy + `cargo test --lib` propres ; smoke
    test manuel complet de chaque endpoint (create/patch/delete créneau,
    bornes, duplication — y compris les deux rejets attendus
    "commission cible non vide"/"commission non propre", mapping — y
    compris rejet slot invalide et créneau hors grille générale, avec
    vérification que l'échec ne vide pas le mapping existant grâce au
    rollback de transaction) via curl authentifié, puis aperçu visuel
    dans le navigateur pour les quatre sections.
  - **Pas construit ici** (pas demandé, hors du périmètre discuté) :
    aucun test d'intégration Rust dédié (`backend/tests/creneaux_*.rs`)
    ni spec e2e Playwright pour cet écran — à ajouter si un futur lot y
    revient, sur le modèle de `commissions_endpoint.rs`/
    `commissions.spec.ts`. Les règles d'enchaînement elles-mêmes (§4.2
    points 1-3, pause/non-enchaînement midi-soir) ne sont pas
    implémentées : cet écran ne fait qu'en paramétrer les bornes, les
    règles proprement dites arriveront avec la génération du planning
    (§6).
  - **Créneaux repas (§4.4, ajouté après coup)** : remplace les règles
    "non-enchaînement midi/soir" (4.2.2/4.2.3, devenues caduques) par des
    groupes nommés de créneaux applicatifs de la grille générale, définis
    par événement et par jour, décorrélés du mapping form. Migration
    `20260101000010_creneaux_repas.sql` (tables `creneau_repas`,
    `creneau_repas_membre`). Domaine : `create_creneau_repas`/
    `update_creneau_repas`/`delete_creneau_repas` dans
    `creneaux/mod.rs` — validations : nom non vide, min. 2 créneaux
    (`CRENEAU_REPAS_MIN_MEMBRES`), jour dans les bornes de l'événement,
    chaque créneau référencé appartient à la grille générale de cet
    événement/jour. Routes `POST/PATCH/DELETE /api/creneaux/:evenement_id/
    repas[/:id]`, incluses dans la réponse `GET /api/creneaux/:id`
    (`creneaux_repas: [{id, jour, nom, creneau_ids}]`). Frontend : nouveau
    panneau "Créneaux repas" dans l'écran Créneaux, même pattern
    d'édition inline que les créneaux (ajout/édition avec nom + toggle
    des créneaux du jour, suppression). **Contrainte de génération
    elle-même (ne jamais affecter un bénévole à tous les créneaux d'un
    même créneau repas) non implémentée** — ce lot ne couvre que le
    paramétrage ; la contrainte arrivera avec l'algorithme de génération
    (§6.2), comme les règles 4.2 ci-dessus.
  - **Duplication jour → jour (§4.1, ajouté après coup)** : raccourci pour
    copier les créneaux de grille générale d'un jour source vers un jour
    cible vide, dans le même événement — les créneaux repas (ci-dessus)
    **et le mapping form → créneaux applicatifs** du jour source sont
    dupliqués avec, reconstruits contre les nouveaux créneaux copiés
    (même nom/`slot_form`). Domaine : `dupliquer_jour` dans
    `creneaux/mod.rs` (transaction unique, map ancien id → nouvel id pour
    relier créneaux repas et mapping copiés) — refuse jour source = jour
    cible, jour cible hors bornes événement, jour source sans créneau
    générale, jour cible déjà non vide. Route `POST /api/creneaux/
    :evenement_id/dupliquer-jour`. Ne touche pas les grilles propres (cf.
    duplication par commission, déjà existante). Frontend : bouton
    "Dupliquer les créneaux d'un autre jour" dans le panneau "créneaux du
    jour",
    affiché seulement quand ce jour n'a encore aucun créneau générale.

- **Écran Besoins** (`/besoins`, specs.md §5/§5.1, wireframe s3) :
  saisie du nombre de bénévoles requis par commission × créneau
  applicatif, avec répartition par poste pour les commissions qui en ont.
  - **Backend** : nouveau module `backend/src/besoins/` (domaine +
    `endpoint.rs`), migration `20260101000008_besoins.sql` (tables
    `besoin_commission`, `besoin_poste`). Upsert (`ON CONFLICT ... DO
    UPDATE`) par créneau × commission (ou × poste), pas de suppression de
    ligne — un besoin à 0 est une valeur normale (§5.3). Validations :
    commission `besoin_chiffre = false` rejetée, créneau hors grille de la
    commission (générale vs propre, même règle que Créneaux §4.3)
    rejeté, somme des besoins par poste sur un créneau ne doit jamais
    dépasser le besoin de la commission sur ce créneau. Routes sous
    `/api/besoins/*`. Pas de macro `sqlx::query!`/`query_as!` (requêtes
    runtime) : aucun impact sur `backend/.sqlx/`.
  - **Frontend** (`frontend/src/routes/(admin)/besoins/+page.svelte`) :
    sélecteur événement/jour (même `btn-group` que Créneaux), un
    `req-table` (nouvelles classes dans `admin.css`, portées de la
    maquette) pour la grille générale — commissions en lignes, créneaux du
    jour en colonnes — puis un sous-tableau par poste pour chaque
    commission qui en a (avec ligne de total calculé en direct côté
    client, avant même l'enregistrement). Panneau séparé pour les
    commissions à grille propre (sélecteur par commission, même
    disposition). Un bouton "Enregistrer les besoins" par tableau envoie
    en parallèle une requête par cellule renseignée. Nav (`AdminNav.svelte`) :
    lien activé, retiré de la liste des écrans désactivés.
  - **Vérifié** : build + clippy propres côté backend, lint + svelte-check
    propres côté frontend ; smoke test manuel (créneaux de test insérés
    puis retirés en base) de la saisie commission, de l'enregistrement, et
    du rejet attendu "la somme des besoins par poste... dépasserait le
    besoin de la commission" avec message affiché dans l'UI.
  - **Pas construit ici** (pas demandé) : aucun test d'intégration Rust
    dédié ni spec e2e Playwright pour cet écran.

- **Écran Responsables** (`/responsables`, specs.md §5.4, wireframe s9) :
  désigner/retirer un ou plusieurs bénévoles responsables d'une
  commission, par événement. Pas d'écran pour les responsables de poste
  (§5.5) — purement informatif, désigné uniquement depuis l'écran Planning
  (pas encore construit), hors scope.
  - **Backend** : nouveau module `backend/src/responsables/` (domaine +
    `endpoint.rs`), migration `20260101000009_responsables.sql` (table
    `responsable_commission`, sans `updated_by`/`updated_at` — la ligne
    n'est jamais modifiée, seulement créée/supprimée). Règle de
    chevauchement (§5.4) implémentée en comparant en mémoire les
    intervalles jour/heure de la grille de la commission visée et de
    chaque autre commission dont ce bénévole est déjà responsable sur cet
    événement (générale ou propre, même logique de périmètre que
    Créneaux/Besoins) — rejeté si au moins un chevauchement, sauf s'il
    s'agit de la même commission. Un endpoint de recherche de bénévoles
    (`GET /responsables/:id/soumissions?q=`, sur la table `soumission` —
    pas d'entité `Benevole` séparée, pas d'écran "Bénévoles" dédié non
    plus pour fournir ça autrement) alimente le champ "Ajouter un
    responsable". Routes sous `/api/responsables/*`. Pas de macro
    `sqlx::query!`/`query_as!` (requêtes runtime).
  - **Frontend** (`frontend/src/routes/(admin)/responsables/+page.svelte`) :
    panneau "Couverture par commission" (compteur par commission, pill
    accentuée si besoin chiffré et 0 responsable), sélecteur de commission
    en `btn-group` (bordure couleur comme Créneaux/Besoins), liste des
    responsables de la commission sélectionnée (`comm-row`, badge
    "R", note de cumul si responsable d'une autre commission), recherche +
    ajout d'un bénévole. Nav (`AdminNav.svelte`) : lien activé, retiré de
    la liste des écrans désactivés.
  - **Vérifié** : build + clippy propres côté backend, lint + svelte-check
    propres côté frontend ; smoke test manuel (soumission et créneaux de
    test insérés puis retirés en base) de l'ajout, du retrait, et du rejet
    de chevauchement attendu ("... est déjà responsable de \"Accueil\",
    dont au moins un créneau chevauche cette commission") avec message
    affiché dans l'UI.
  - **Pas construit ici** (pas demandé) : aucun test d'intégration Rust
    dédié ni spec e2e Playwright pour cet écran ; pas de calcul de blocage
    de la génération (§6.1) à partir de ce panneau de couverture — purement
    informatif pour l'instant, la génération elle-même n'existe pas.

- **Écran Bénévoles** (`/benevoles`, specs.md §3.4, wireframe s7) : saisie
  manuelle d'un bénévole arrivé hors Google Form — un formulaire reprenant
  la structure du form (identité/contact, puis un bloc disponibilités +
  souhaits par événement coché), remplissable en une seule saisie. Premier
  écran de ce lot qui n'existait pas du tout dans l'appli (contrairement à
  Créneaux/Besoins/Responsables, qui restylaient ou complétaient une page
  déjà présente).
  - **Backend** : nouveau module `backend/src/benevoles/` (domaine +
    `endpoint.rs`), route unique `POST /api/benevoles`. Aucune nouvelle
    migration : réutilise les tables déjà posées pour le sync
    (`soumission`, `disponibilite_soumission`, `souhait_commission`) —
    contrairement à une ligne importée du form, une saisie manuelle choisit
    directement une commission/poste du référentiel (pas de libellé brut à
    passer par `soumission_libelle`/le Mapping form, cf. §3.2). Une
    soumission est créée par événement coché, dans **une seule
    transaction** (tout ou rien). Validations : identité obligatoire
    (email vérifié avec le même `is_valid_email`, rendu `pub(crate)` dans
    `admins/mod.rs`, que l'écran Admins), `tranche_age` comparée aux
    labels exacts de `sync::TrancheAge::label()`, au moins un événement et
    pas de doublon d'événement dans la requête, chaque jour de
    disponibilité dans les dates de l'événement visé, chaque poste souhaité
    rattaché à la bonne commission. "Je suis joueur, peu m'importe" pose
    directement `soumission.polyvalent = true` plutôt qu'un souhait. Pas
    de macro `sqlx::query!`/`query_as!` (requêtes runtime) : aucun impact
    sur `backend/.sqlx/`.
  - **Frontend** (`frontend/src/routes/(admin)/benevoles/+page.svelte`) :
    panneau identité/contact, panneau de sélection d'événements, puis un
    panneau par événement coché avec le tableau de disponibilités
    (jours/créneaux du form déduits des `(jour, slot_form)` déjà posés sur
    l'écran Créneaux pour cet événement — `GET /creneaux/:id`, champ
    `mapping` — jamais codé en dur par événement) et la liste de souhaits
    (une case par commission active du référentiel, + une case par poste
    quand elle en a, + la case polyvalent). Nouvelle classe
    `.field-check` dans `admin.css` pour les lignes case à cocher + libellé
    (le reste réutilise `field`/`field-grid`/`req-table`/`btn-group`
    existants). Nav (`AdminNav.svelte`) : lien activé, retiré de la liste
    des écrans désactivés.
  - **Vérifié** : `cargo fmt`/`clippy --all-targets`/`cargo test --lib`
    propres côté backend ; `svelte-check`/eslint/prettier/`npm run build`
    propres côté frontend. Smoke test manuel via curl authentifié contre
    `docker-compose.dev.yml` (login OTP réel via Mailpit) : création
    réussie (soumission + dispos + souhaits vérifiés en base), puis les
    rejets attendus — email invalide, poste n'appartenant pas à la
    commission choisie, événement inconnu (404), aucun événement
    sélectionné — tous avec le bon message. Créneau/mapping de test
    insérés puis retirés en base pour vérifier que le tableau de
    disponibilités affiche bien les cases issues du mapping réel. **Pas
    vérifié visuellement dans un navigateur** (extension Claude in Chrome
    indisponible dans cette session) — juste `svelte-check`/build/un
    `curl` sur la page buildée.
  - **Pas construit ici** (pas demandé) : aucun test d'intégration Rust
    dédié ni spec e2e Playwright pour cet écran. Pas de filtrage des
    commissions/postes proposés par tranche d'âge (§3.1 — le form réel
    varie la liste selon l'âge) : la saisie manuelle montre tout le
    référentiel actif, choix délibéré pour rester simple (cf. note du
    wireframe s7 : "liste = référentiel des commissions actives").

- **Planning — affectation manuelle, backend seul** (specs.md §6.3/§7.1) :
  premier lot de l'écran Planning, décidé avec l'utilisateur en deux
  temps — schéma + CRUD backend d'abord, écran `/planning` ensuite (pas
  encore construit). Ce lot ne couvre que poser/retirer une affectation à
  la main ; la génération automatique (§6.2) n'existe pas.
  - **Backend** : nouveau module `backend/src/planning/` (domaine +
    `endpoint.rs`), migration `20260101000012_affectations.sql` (table
    `affectation` : `soumission_id`, `creneau_applicatif_id`,
    `commission_id`, `poste_id` optionnel, `verrouille` — toujours `true`
    ici, posé en prévision de la génération future, cf. §6.3 : une
    affectation générée ne doit jamais être écrasée par une affectation
    manuelle ultérieure déjà verrouillée). Contraintes DB : `UNIQUE
    (soumission_id, creneau_applicatif_id)` (un bénévole, une seule
    commission/poste par créneau), FK composite `(poste_id,
    commission_id) → poste(id, commission_id)` (même garde-fou que
    `mapping_souhait_form`, §3.2). Validations applicatives (`mod.rs`) :
    soumission/créneau appartiennent à l'événement de l'URL, le créneau
    appartient bien à la grille de la commission choisie (générale si
    `commission_id IS NULL` sur le créneau, propre sinon — même logique
    que `creneaux::check_commission_propre`), poste appartient à la
    commission, pas de double affectation sur le même créneau. Routes
    `GET/POST /api/planning/:evenement_id/affectations` (`GET` filtré par
    `?jour=`), `DELETE /api/planning/:evenement_id/affectations/:id`.
  - **Vérifié** : build + clippy + `cargo test --lib` propres ; smoke test
    manuel complet via curl authentifié (create, list par jour, rejet
    doublon sur même créneau, rejet créneau hors grille de la commission,
    rejet poste hors commission, 404 soumission inconnue, delete).
  - **Pas de `PATCH`** : changer une affectation se fait par delete +
    create, suffisant pour l'écran construit ci-dessous. Aucun test
    d'intégration Rust dédié ni spec e2e pour ce module.
  - **Écran `/planning` (lot suivant, même conversation)** : vue
    événement/jour, disposition "Bénévoles en lignes" uniquement
    (choisi avec l'utilisateur — "Créneaux en lignes" et les filtres
    commission/poste/recherche de §7.1 restent à construire). Colonnes =
    créneaux de la **grille générale** du jour uniquement (`commission_id
    IS NULL`) : une commission à grille propre (Plateau) n'est pas
    assignable depuis cet écran pour l'instant — limitation délibérée,
    confirmée avec l'utilisateur, à lever dans un lot futur (union des
    grilles ou vue dédiée). Lignes = tous les bénévoles ayant déclaré une
    disponibilité ce jour (nouvel endpoint `GET
    /api/planning/:evenement_id/benevoles?jour=`, dans
    `planning::endpoint`, domaine directement en SQL runtime dans
    l'endpoint — pas de mutation, donc pas besoin de `mod.rs` — déduit la
    disponibilité via `disponibilite_soumission` + `creneau_form_mapping`,
    comme le fait le mapping form→créneaux, cf. écran Créneaux), même
    quand ils n'ont encore aucune affectation ce jour. Clic sur une
    cellule disponible et vide → mini-panneau inline (commission puis
    poste si la commission en a, souhaits de ce bénévole affichés en
    rappel) → `POST .../affectations` ; clic sur ✕ d'une cellule déjà
    affectée → `DELETE`. Frontend :
    `frontend/src/routes/(admin)/planning/+page.svelte`, nouveau lien
    "Planning" dans `AdminNav.svelte` (premier élément — écran principal,
    absent de la liste `s3-s10` d'origine, comme "Bénévoles" l'a été).
  - **Vérifié** : build + clippy + `cargo test --lib`/svelte-check/eslint/
    prettier propres ; endpoint `benevoles` et flux complet vérifiés via
    curl authentifié (OTP réel via Mailpit) contre le stub Sheets.
    Vérification visuelle dans le navigateur non refaite dans cette
    session (pas d'extension Claude in Chrome connectée) — le code suit
    strictement le pattern déjà validé visuellement des autres écrans
    (Besoins/Créneaux), donc risque jugé faible, mais à confirmer à
    l'œil à la prochaine occasion.
  - **Pas construit ici** (hors périmètre discuté pour ce lot) :
    disposition "Créneaux en lignes", filtres commission/poste/recherche
    bénévole, totaux par créneau, badges responsable/responsable de
    poste, version imprimable, détail individuel (écran s2), affectation
    sur grille propre, et bien sûr la génération automatique (§6.2) —
    tout §7.1 au-delà de la vue minimale "Bénévoles en lignes, grille
    générale, affectation manuelle".

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
7. ~~Écran "Synchro form" (périmètre réduit, cf. "Fait")~~ — fait (voir
   plus bas). Doublons/permis B/envoi email restent à construire, pas
   décidé quand.
8. ~~Écran "Commissions" (périmètre réduit, cf. "Fait")~~ — fait (voir
   plus bas). Réordonnancement des postes et toggle actif/inactif
   restent à construire, pas décidé quand.
9. ~~Websocket de rafraîchissement live, tous écrans admin~~ — fait (voir
   plus bas). Scoping Planning (par événement/jour) différé jusqu'à la
   construction de cet écran.
10. ~~Écran "Créneaux" (gabarit applicatif, grilles propres, bornes,
    mapping form)~~ — fait (voir plus bas).
11. ~~Écran "Besoins" (commission × créneau, répartition par poste)~~ —
    fait (voir plus bas).
12. ~~Écran "Responsables" (responsables de commission par événement)~~ —
    fait (voir plus bas).
13. Rate limiting (DB, décider header IP) — seul point ouvert restant de
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
  --test mapping --test mapping_endpoint --test evenements --test evenements_endpoint \
  --test auth_endpoint --test admins_endpoint --test commissions_endpoint \
  --test live_endpoint

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
- `backend/src/sync/` — parsing form + orchestration + persistance +
  `endpoint.rs` (`POST /sync` déclenchement manuel, `GET /sync/status`
  écran Synchro).
- `backend/src/mapping/` — résolution souhaits + écran Mapping (le
  référentiel Commission/Poste lui-même vit dans `commissions/`).
- `backend/src/commissions/` — CRUD Commission/Poste (écran Commissions),
  `GET /commissions` partagé avec l'écran Mapping.
- `backend/src/evenements/` — CRUD Événements.
- `backend/src/live.rs` — websocket de rafraîchissement live (`GET /api/ws`
  + middleware `emit_live_event` posé sur `api_router()`), `frontend/src/lib/live.ts`
  son pendant côté client.
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
