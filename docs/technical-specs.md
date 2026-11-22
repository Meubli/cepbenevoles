# Spécification technique — Application de planning des bénévoles

Nov 2026 — Projet : CEP Poitiers Gymnastique, Championnats de France 2027

Ce document couvre la technique (stack, hébergement, déploiement). Le fonctionnel est couvert dans `specs.md`.

## 1. Hébergement

- Auto-hébergé sur le NAS personnel du porteur du projet, **exposé sur internet** (pas de réseau local uniquement).
- Base de données : instance **Postgres** partagée (sert aussi d'autres projets du NAS), conteneurisée, avec son propre volume de données monté sur le NAS. Cette application utilise une base dédiée (`benefrance`) sur cette instance.
- Configuration réseau/NAS (reverse proxy, TLS, exposition de ports, durcissement) : traitée séparément, plus tard.

## 2. Stack technique

- **Backend** : Rust, framework **Axum**, accès base via **sqlx** (driver Postgres). Client HTTP sortant (`reqwest`, appels Google Sheets/OAuth2) en **rustls** plutôt que native-tls/OpenSSL — cohérent avec `lettre` (SMTP) déjà en rustls, et évite d'avoir à installer `libssl` dans l'image finale (§3), qui n'a que `ca-certificates`.
- **Frontend** : **SvelteKit**, buildé en SPA statique.
- **Base de données** : Postgres (instance partagée, cf. 1).
- **Migrations** : gérées côté Rust (`sqlx migrate` ou `refinery`), exécutées au démarrage du container applicatif — jamais manuellement sur le NAS.

### 2.1 Pourquoi ces choix

- Rust/Axum : binaire statique, faible empreinte mémoire/CPU, adapté au matériel NAS.
- Postgres plutôt que SQLite : l'instance est mutualisée entre plusieurs projets du NAS, un serveur réseau partagé est donc plus adapté qu'un fichier SQLite par projet.
- SvelteKit plutôt qu'une lib plus légère (htmx, vanilla JS) : l'app est exposée sur internet (pas de LAN), chaque interaction htmx impliquerait un aller-retour réseau réel ; une SPA récupère les données une fois puis interagit en mémoire, plus fluide pour les tableaux filtrés/dispositions de l'écran planning. Le gain de légèreté d'une solution encore plus minimale (vanilla, Alpine) serait marginal face à la perte de confort de dev pour les interactions déjà spécifiées (filtres, bascule de disposition, menus contextuels, color picker).

## 3. Déploiement

- **Une seule image Docker** pour backend + frontend : le binaire Axum sert à la fois l'API et les fichiers statiques buildés par SvelteKit (route fallback `/` → `index.html`, assets via `tower-http::ServeDir`).
- **Dockerfile multi-stage** :
  1. Stage build front (node) : `npm run build` du SPA SvelteKit.
  2. Stage build back (rust) : compilation du binaire Axum.
  3. Stage final : copie uniquement le binaire compilé + les assets statiques du front — pas de node/npm dans l'image finale.
- Le container applicatif est **stateless** : pas de volume propre, connexion réseau vers le container Postgres (même réseau Docker ou IP du NAS selon où tourne l'instance Postgres).
- Secrets (clé de service account Google, identifiants SMTP, URL de connexion à la base) injectés via variables d'environnement ou fichier monté en lecture seule — jamais intégrés à l'image.

## 4. Dépôt de code et CI/CD

- Code hébergé sur **GitHub** (dépôt privé) — accès aux runners **GitHub Actions** gratuits (minutes incluses sur dépôt privé) pour la CI/CD.
- Pipeline envisagé :
  1. Build de l'image Docker (étapes du Dockerfile multi-stage, cf. 3) à chaque push/merge sur la branche principale.
  2. Publication de l'image sur le **GitHub Container Registry (GHCR)**, gratuit et intégré nativement à GitHub Actions (pas de registry tiers à gérer).
  3. Le NAS récupère (`docker pull`) la nouvelle image depuis GHCR — déclenchement manuel dans un premier temps ; automatisation (webhook, polling, Watchtower...) à voir plus tard.
- Tests (cf. §9) exécutés dans le même pipeline avant build de l'image, pour bloquer la publication si echec.

## 5. Rate limiting & cache

- Pas de service de cache externe (Redis, Memcached) : volume d'usage trop faible pour le justifier (petite association, quelques admins). Un cache in-process côté Rust (ex. crate `moka`) reste possible plus tard si un besoin de performance précis apparaît, sans dépendance externe.
- Rate limiting (specs §11.2) implémenté via **table Postgres dédiée** : compteur par email et par IP sur fenêtre glissante (requête `COUNT(*) WHERE created_at > now() - interval '...'`, indexée sur `(email, created_at)` / `(ip, created_at)`). Cohérent avec le conteneur applicatif stateless (§2.1) — aucun état en mémoire à perdre entre redémarrages.

## 6. Email (SMTP)

- Boîte mail de service **Google (Gmail)**, dédiée à l'association — utilisée à la fois pour l'envoi des codes OTP (specs §11.1) et l'envoi manuel du planning aux bénévoles (specs §7.3).
- Distincte du compte de **service account** Google utilisé pour l'accès à la Sheet de réponses (§1 ci-dessus, OAuth2 JSON) : le SMTP utilise des identifiants de boîte mail classique (adresse + mot de passe d'application), pas le même mécanisme d'auth.
- Identifiants SMTP injectés via variable d'environnement / fichier monté, jamais dans l'image (cf. §3).

## 7. Fuseau horaire

- Stockage en base : **UTC** (type `timestamptz` Postgres).
- Affichage : converti en **Europe/Paris** côté frontend (ou au rendu backend pour l'export PDF/impression). Évite toute ambiguïté liée aux changements d'heure (DST) sur les créneaux d'événements.

## 8. Logs

- Niveau **debug** par défaut.
- Sortie **stdout/stderr** du conteneur applicatif, consultée via `docker logs` — pas de stack d'agrégation/log dédiée (ex. Loki, ELK) à ce stade.

## 9. Tests

Stratégie à trois niveaux, tests d'intégration et e2e priorisés sur les tests unitaires (jugés moins représentatifs ici) ; unitaires ajoutés seulement sur logique isolée pertinente (ex. règles d'enchaînement de créneaux, calculs de génération).

- **Intégration (backend, Rust)** : `cargo test`, contre une instance Postgres **testcontainers** (un seul conteneur partagé pour tout le run, une base fraîche + migrations par test). Appels Google Sheets API mockés via `wiremock` (fixtures JSON), jamais d'appel réseau réel en test.
- **E2E (Playwright)** : `docker-compose.test.yml` dédié (Postgres, **Mailpit** pour lire les codes OTP envoyés par email, stub HTTP pour l'API Sheets). Playwright démarre le **binaire Axum réel** servant la SPA buildée (fidèle au déploiement prod, image unique), piloté en multi-navigateur et multi-contexte (utile pour les scénarios multi-admin, ex. rafraîchissement temps réel §10 ci-dessous).
- **CI** (GitHub Actions, `.github/workflows/ci.yml`) : trois jobs séquentiels bloquants — guardrails (fmt, clippy, `cargo sqlx prepare --check`, svelte-check, eslint/prettier) → intégration → e2e. Rapport Playwright (trace) remonté en artifact en cas d'échec.

## 10. Temps réel (websocket)

**Résolu (implémentation) — élargi à tous les écrans admin, pas seulement Planning** : à l'origine ce chapitre ne couvrait que la mise à jour live du planning entre admins (specs.md §7.1). Demandé explicitement par l'utilisateur : chaque écran admin (Commissions, Mapping form, Événements, Synchro form, Admins — Planning suivra le même mécanisme une fois construit) doit refléter en direct les changements faits par un autre admin, sans recharger la page. Niveau minimal, inchangé : **notification de rafraîchissement par tag de ressource**, pas d'édition collaborative (pas de curseurs, pas de résolution de conflit type CRDT) — en cas d'édition concurrente du même élément, dernier enregistrement gagne.

- **Support** : websocket intégré d'Axum (`axum::extract::ws`), pas de dépendance ni d'infra supplémentaire. Un seul endpoint `GET /api/ws` pour tous les écrans (pas un par écran).
- **Flux push-only, sans donnée** : toute modification passe par les routes HTTP authentifiées habituelles (jamais d'écriture via le websocket), et le message diffusé ne contient **jamais les données elles-mêmes** — juste un tag de ressource générique (ex. `"commissions"`, `"mapping"`, `"evenements"`, `"sync"`, `"admins"`), tiré du premier segment du chemin de la requête qui vient de réussir (`backend/src/live.rs::emit_live_event`, middleware sur `api_router()` : non-GET + statut 2xx seulement, `/auth/*` exclu). Diffusion via un `tokio::sync::broadcast::Sender<String>` partagé (`AppState.live`), sans scoping par (événement, jour) — plus simple que le scoping Planning initialement envisagé, suffisant pour une appli de cette taille ; **le scheduler de sync** (`main.rs::spawn_scheduler`, qui écrit en base hors requête HTTP) émet aussi `"sync"` après chaque exécution, puisqu'il contourne le middleware.
- **Client** (`frontend/src/lib/live.ts`) : un seul websocket partagé par onglet, ouvert à la première page qui s'y abonne (`onLive(tags, cb)`). À réception d'un tag, seules les pages abonnées à ce tag (ou à un full-resync `"*"`, envoyé après une reconnexion — les événements manqués pendant la coupure ne sont pas rejoués) rappellent leur fonction de rafraîchissement ; reconnexion automatique avec backoff exponentiel (max 15s). **Un rafraîchissement déclenché par le live ne doit jamais** faire disparaître un formulaire en cours de saisie (pas de toggle de `loading`) **ni** effacer un message d'erreur de validation qu'une action locale vient de poser (le rafraîchissement live ne touche l'état d'erreur qu'en cas d'échec de son propre fetch, jamais en l'effaçant sur un succès) — motif direct d'une régression e2e observée (deux workers Playwright sur le même backend) et corrigée en conséquence.
- **Sécurité** :
  - Handshake authentifié avec la **même session cookie** (httpOnly) que le reste de l'appli ; upgrade refusé sans session valide (`AuthAdmin`, comme toute autre route).
  - `wss://` uniquement (TLS via le reverse proxy, cf. 1) — **point ouvert, non vérifié en prod** : la configuration du reverse proxy doit transmettre l'en-tête `Upgrade` (comme pour tout websocket), pas encore documentée/testée ici.
  - Messages sans donnée sensible (juste un tag de ressource), ce qui limite encore l'impact d'une éventuelle fuite.
  - **Connu non géré** : un websocket ouvert n'est pas fermé à la déconnexion ou à la suppression de l'admin — risque faible, il ne porte aucune donnée.

## 11. Corrélation des erreurs (correlation ID)

- Chaque requête HTTP entrante se voit attribuer un **ID de corrélation** (UUID v4), généré par un middleware Axum (ex. `tower-http::request_id`), dès son entrée.
- Cet ID est injecté dans le **span `tracing`** de la requête : toutes les lignes de log émises pendant son traitement (niveau debug, cf. §8) le portent automatiquement, sans avoir à le passer manuellement à chaque fonction.
- **Erreur technique (5xx)** : la réponse JSON renvoyée au frontend inclut l'ID de corrélation (champ `correlation_id`), en plus du header `X-Correlation-Id`. Le frontend affiche un message générique ("Une erreur technique est survenue") accompagné de cet ID, à communiquer à un admin/développeur pour retrouver les logs correspondants (`docker logs`, recherche du UUID).
- **Erreur fonctionnelle/validation (4xx)** : pas besoin de corrélation — le message retourné est déjà explicite et actionnable par l'utilisateur (cf. règles de validation métier, `specs.md`), inutile de complexifier l'affichage.
- Pas de stockage de l'ID côté base ; uniquement présent dans les logs du conteneur (stdout) le temps de leur rétention (gérée par Docker, pas de rotation applicative dédiée à ce stade).
