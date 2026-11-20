# Spécification fonctionnelle — Application de planning des bénévoles

Nov 20, 2026 — Projet : CEP Poitiers Gymnastique, Championnats de France 2027

## 1. Contexte

L'association organise deux événements en 2027 :

- **France Indiv Perf GAM/GAF** — 29 avril au 4 mai 2027, Parc des Expositions de Poitiers
- **France TeamGym Performance** — 11 au 15 juin 2027, Arena Futuroscope

Les bénévoles s'inscrivent via un Google Form (identité, disponibilités par jour/créneau pour chaque événement, poste(s) souhaité(s)). L'association doit ensuite construire un planning qui affecte chaque bénévole à des créneaux et des commissions, en respectant les effectifs requis par commission et par créneau, et des règles d'enchaînement des créneaux.

Ce document couvre le fonctionnel uniquement ; la technique (stack, hébergement, auth) sera traitée séparément.

## 2. Acteurs

- **Bénévole** : remplit le Google Form. N'a pas de compte dans l'application (sauf décision contraire — cf. question ouverte 10.4).
- **Admin** : saisit les besoins (nb bénévoles par commission/créneau), configure les créneaux applicatifs, lance la génération du planning, modifie le planning à la main, gère la liste des commissions.

## 3. Collecte des données — Google Form

### 3.1 Structure du formulaire (relevée sur le form actuel)

**Identité / contact** (section 1) : adresse e-mail, nom, prénom, tranche d'âge (10-12, 13-15 *(sic, incohérent avec section 6 qui utilise 13-14)*, 15-17, 18+), mail (doublon de l'adresse e-mail), téléphone, permis B (oui/non), taille de tee-shirt.

**Disponibilités**, par événement, avec une question filtre ("Serez-vous de la partie ?" oui/non) avant le détail jour par jour :

| Événement | Jour | Créneaux proposés dans le form |
| --- | --- | --- |
| France Indiv | Jeudi 29 avril | matin, aprem, soir |
| France Indiv | Vendredi 30 avril | matin, midi, aprem, soir |
| France Indiv | Samedi 1er mai | matin, midi, aprem, soir |
| France Indiv | Dimanche 2 mai | matin, midi, aprem, soir |
| France Indiv | Lundi 3 mai | matin, aprem |
| France Indiv | Mardi 4 mai | matin |
| TeamGym | Vendredi 11 juin | matin, aprem, soir |
| TeamGym | Samedi 12 juin | matin, midi, aprem, soir |
| TeamGym | Dimanche 13 juin | matin, midi, aprem, soir |
| TeamGym | Lundi 14 juin | matin, aprem |
| TeamGym | Mardi 15 juin | matin |

**Poste(s) souhaité(s)** (section 6), liste dépendant de la tranche d'âge :

- *10-12 ans* : Remise des récompenses lors des palmarès (option unique)
- *13-14 ans* : Secrétaire de table pour la saisie des notes (option unique)
- *15 ans et + / adultes* : Je suis joueur, peu m'importe · Accueil · Transport · Restauration · Friture / cuisson / grillade · Boutique · Contrôle des accès · Hébergement ISAAC de l'ÉTOILE (nuit possible sur place) · Déroulement (connaissance du déroulement d'une compétition nécessaire) · Sonorisation (connaissance du déroulement d'une compétition nécessaire) · Secrétaire de table pour la saisie des notes · Juge de ligne sur le plateau de compétition (N2 requis)

"Je suis joueur, peu m'importe" = bénévole polyvalent, affectable à n'importe quelle commission.

Le form contient aussi des questions hors périmètre planning (compétences, matériel prêté, allergies...) — non reprises dans l'app sauf décision contraire.

### 3.2 Synchronisation automatique

- Le Google Form est lié à une Google Sheet de réponses.
- Un service account accède à cette Sheet via l'API Google Sheets.
- Synchronisation : job planifié (ex. toutes les 10-15 min) **+** bouton "Synchroniser maintenant" côté admin.
- Chaque ligne de la Sheet = une **soumission**. Clé d'import = (row index Sheet + horodatage de réponse) pour ne jamais réimporter deux fois la même ligne.
- Décision actée : **pas de dédoublonnage automatique** entre soumissions (une personne qui soumet deux fois crée deux soumissions distinctes). L'admin pourra marquer manuellement deux soumissions comme doublon/fusionnées dans le planning (cf. 9).

## 4. Créneaux applicatifs

Les créneaux du formulaire (matin/midi/aprem/soir) ne sont pas les créneaux de travail réels. Exemple donné : "vendredi matin" (form) = 2 créneaux applicatifs 7h-9h et 9h-11h.

### 4.1 Paramétrage

- Un **gabarit de créneaux applicatifs par défaut** est fourni par l'app (modifiable).
- Pour chaque événement, l'admin peut définir/adapter, **par jour**, la liste des créneaux applicatifs (heure début, heure fin). Le nombre de créneaux par jour est variable.
- Un **mapping** relie chaque créneau du formulaire (jour + matin/midi/aprem/soir) à un ou plusieurs créneaux applicatifs de ce jour. C'est ce mapping qui permet de déduire la disponibilité réelle d'un bénévole à partir de sa réponse au form.

### 4.2 Règles d'enchaînement (paramétrables, avec valeurs par défaut)

1. **Pause obligatoire** : après 2 créneaux applicatifs consécutifs travaillés, au moins 1 créneau non travaillé est requis avant de pouvoir réaffecter le bénévole. Seuil "2" paramétrable.
2. **Non-enchaînement midi** : un bénévole ne peut pas être affecté aux deux créneaux du midi à la suite (par défaut 11h-13h et 13h-15h).
3. **Non-enchaînement soir** : un bénévole ne peut pas être affecté aux deux créneaux du soir à la suite (par défaut 17h-19h30 et 19h30h-fin).
4. Les plages "matin"/"midi"/"soir" (bornes horaires qui définissent ces catégories) sont paramétrables par événement.
5. Le nombre de créneaux applicatifs par jour est variable d'un jour à l'autre (déjà couvert en 4.1).
6. Une commission configurée avec une grille de créneaux propre (cf. 5.2) n'est soumise à aucune des règles 1 à 3 ci-dessus : ses créneaux s'enchaînent librement.

Par défaut, le créneau "aprem" est **15h-17h**.

### 4.3 Grille de créneaux par commission

Par défaut, une commission utilise la grille de créneaux applicatifs générale de l'événement (section 4.1). L'admin peut configurer une commission pour qu'elle utilise à la place une **grille de créneaux qui lui est propre** (ses propres heures de début/fin, son propre nombre de créneaux par jour) — par exemple pour les commissions Déroulement, Sonorisation, Juge de ligne et Secrétariat de table, qui partagent aujourd'hui la même grille spécifique. Les règles d'enchaînement de 4.2 ne s'appliquent qu'aux commissions utilisant la grille générale.

Il n'y a pas d'objet "grille" partagé entre commissions : chaque commission à grille propre définit ses propres créneaux, indépendamment des autres. Pour créer une grille propre sur une nouvelle commission, l'admin bascule cette commission sur "grille propre" (écran Commissions) puis saisit ses créneaux dans l'écran Créneaux, en la sélectionnant dans la liste des commissions à grille propre. Si deux commissions ont les mêmes horaires (ex. Déroulement, Sonorisation, Juge de ligne, Secrétariat de table aujourd'hui), c'est une coïncidence de saisie, pas un partage structurel.

Pour gagner du temps, l'écran Créneaux propose de **dupliquer la grille d'une commission existante** comme point de départ d'une nouvelle grille propre (copie des créneaux jour par jour, ensuite modifiable librement). C'est un raccourci de saisie : une fois dupliquée, la grille reste indépendante de celle d'origine, sans lien structurel entre les deux.

## 5. Commissions

- Référentiel global de commissions, commun aux événements, avec possibilité pour l'admin d'**ajouter de nouvelles commissions** à tout moment.
- Liste initiale = commissions relevées en section 3.1 (Accueil, Transport, Restauration, Friture/cuisson/grillade, Boutique, Contrôle des accès, Hébergement, Déroulement, Sonorisation, Secrétariat de table, Juge de ligne, Remise des récompenses...).
- Certaines commissions ont des **prérequis** (ex. "Juge de ligne" = N2 requis, "Déroulement"/"Sonorisation" = connaissance du déroulement d'une compétition). Ces prérequis sont déclaratifs côté form (texte dans le libellé) ; l'app n'a pas aujourd'hui de champ de compétence structuré distinct — à statuer (10.5).
- **Besoins** : pour chaque événement, l'admin saisit le nombre de bénévoles nécessaires par **commission × créneau** (créneau de la grille générale, ou de la grille propre à la commission si elle en a une — cf. 4.3). Pour une commission qui a des postes (cf. 5.1), ce total est ensuite réparti manuellement par poste.

### 5.1 Postes (sous-découpage d'une commission)

Une commission peut être découpée en **postes**. Exemple : la commission *Animation* a les postes *Mascotte*, *Tombola gradins*, *Micro-trottoir*, *Stand tombola*, *Photo booth*. D'autres commissions n'ont pas de poste (ex. *Friture/cuisson/grillade*) et restent affectées globalement à la commission.

- Les postes sont créés par l'admin, propres à une commission.
- Le besoin est saisi au niveau **commission × créneau**. Quand la commission a des postes, l'admin répartit ensuite manuellement ce total par **poste × créneau** ; la somme des besoins par poste sur un créneau doit égaler le besoin de la commission sur ce même créneau (contrôle de cohérence à l'écran).
- Une affectation sur une commission à postes précise le poste ; sur une commission sans poste, seule la commission est renseignée.
- Le champ "Poste(s) souhaité(s)" du Google Form désigne le souhait de **commission** du bénévole (vocabulaire historique du form) ; il ne correspond pas au concept de poste introduit ici.

### 5.2 Commissions à grille de créneaux propre

Quatre commissions — **Déroulement, Sonorisation, Juge de ligne, Secrétariat de table** — suivent des règles différentes des autres :

- Elles utilisent une **grille de créneaux qui leur est propre**, distincte de la grille applicative générale de l'événement, paramétrable par l'admin (cf. 4.3).
- Les bénévoles qui leur sont affectés peuvent **enchaîner tous les créneaux de cette grille** : aucune des règles de pause / non-enchaînement midi / non-enchaînement soir (section 4.2) ne s'applique à ces affectations.
- Plus largement, **toute commission** peut être configurée par l'admin pour utiliser soit la grille applicative générale (comportement par défaut), soit une grille de créneaux qui lui est propre ; dans ce second cas les règles d'enchaînement de 4.2 ne s'appliquent pas aux affectations de cette commission.

## 6. Génération du planning

### 6.1 Entrées

- Bénévoles (soumissions) avec leurs disponibilités (déduites via le mapping form → créneaux applicatifs) et leurs souhaits de commission.
- Besoins par commission × créneau (section 5).
- Règles d'enchaînement (section 4.2).

### 6.2 Algorithme — contraintes actées

1. Respecter autant que possible le(s) souhait(s) de commission du bénévole (un "joueur" est affectable à toute commission en tension).
2. Équilibrer la charge totale (nombre de créneaux travaillés) entre bénévoles.
3. Ne jamais violer les règles de pause / non-enchaînement midi / non-enchaînement soir.

### 6.3 Non-régression sur modifications manuelles

- Toute affectation modifiée à la main par un admin est marquée **verrouillée**.
- Une nouvelle génération recalcule uniquement les affectations non verrouillées ; elle ne touche jamais une affectation verrouillée, et prend les affectations verrouillées comme contraintes fixes (un verrouillé compte dans la charge du bénévole et dans l'effectif déjà couvert de sa commission/créneau).

## 7. Dashboards & écrans

### 7.1 Vue planning événement / jour (vue principale)

- Sélection d'un événement, puis d'une date (jour) de cet événement.
- Tableau : lignes = bénévoles triés par nom de famille puis prénom ; colonnes = groupe "identité" bénévole + une colonne par créneau applicatif du jour ; cellule = commission(s) affectée(s) à ce bénévole sur ce créneau.
- Clic sur un bénévole → détail de son planning individuel sur **tout l'événement** (tous les jours).
- Filtre/recherche de bénévole, pour accéder rapidement à son planning individuel.

### 7.2 Détail individuel

- Planning du bénévole sur l'événement en cours.
- Bouton de switch pour afficher son planning sur un **autre événement** (facilement accessible, sans ressaisir de recherche).

### 7.3 Écrans admin

- Saisie des besoins par commission × créneau (événement).
- Configuration des créneaux applicatifs (gabarit par défaut + override par jour/événement), des bornes matin/midi/soir, et des grilles de créneaux propres à certaines commissions (cf. 4.3).
- Gestion du référentiel de commissions (ajout/désactivation, postes par commission, choix grille générale ou grille propre).
- Lancement de la génération du planning + visualisation des affectations verrouillées vs générées.
- Suivi de synchronisation Google Form (dernière synchro, nombre de nouvelles soumissions, bouton synchroniser).
- Envoi (manuel) du planning individuel par email aux bénévoles, événement par événement, depuis une boîte mail dédiée à l'association.

## 8. Modèle de données (entités pressenties)

- `Evenement` (nom, dates)
- `CreneauApplicatif` (evenement, commission nullable — renseigné seulement pour la grille propre d'une commission, jour, heure\_debut, heure\_fin, ordre)
- `MappingCreneauForm` (evenement, jour, libelle\_form \[matin/midi/aprem/soir\] → liste de CreneauApplicatif)
- `Commission` (nom, actif, prerequis\_libre\_texte, a\_des\_postes: bool, grille\_creneaux \[generale|propre\])
- `BesoinCommission` (evenement, creneau\_applicatif, commission, nb\_requis) — nb\_requis total de la commission sur ce créneau
- `Soumission` (evenement, nom, prenom, email, telephone, tranche\_age, permis\_b, taille\_tshirt, date\_soumission, row\_source\_id, benevole\_id nullable)
- `DisponibiliteSoumission` (soumission, jour, libelle\_form)
- `SouhaitCommission` (soumission, commission)
- `Affectation` (evenement, soumission, creneau\_applicatif, commission, poste nullable, origine \[genere|manuel\], verrouille: bool)
- `Benevole` (nom, prenom, email, profil canonique issu d'une fusion de soumissions)
- `Poste` (commission, nom)
- `BesoinPoste` (evenement, creneau\_applicatif, commission, poste, nb\_requis) — somme par créneau = nb\_requis de BesoinCommission

## 9. Doublons de soumission

Clé d'identification d'une soumission : nom + prénom + email.

- **Même clé, même événement** : doublon. La **dernière soumission** (la plus récente) fait foi pour les disponibilités et les souhaits de commission de cet événement ; l'ancienne reste conservée en base mais n'est plus prise en compte par défaut.
- **Même clé, événements différents** : ce n'est pas un doublon, c'est le même bénévole inscrit aux deux événements (cas normal, ex. une soumission par événement). L'admin peut alors **fusionner** ces soumissions en un seul profil bénévole, pour que son planning individuel agrège les deux événements (cf. 7.2). La fusion est toujours une action manuelle de l'admin, jamais automatique.

## 10. Questions ouvertes

1. Résolu : "aprem" = 15h-17h par défaut (cf. section 4.2).
2. Gabarit de créneaux applicatifs par défaut (nombre et horaires précis) à valider avec l'association.
3. **Résolu : les compétences/prérequis (N2, connaissance du déroulement) ne sont pas pris en compte dans la génération ; ils restent déclaratifs (texte libre sur la commission).**
4. Résolu : l'application est réservée aux admins ; les bénévoles n'ont pas de compte.
5. Résolu : notification par email, envoyée manuellement par un admin, événement par événement, depuis une boîte mail dédiée à l'association (pas d'envoi automatique depuis l'application).
6. Gestion multi-saison : les événements/commissions doivent-ils être archivés et réutilisables d'une année sur l'autre (ex. dupliquer la config de créneaux) ? Point mis de côté, à retraiter plus tard.
7. Résolu : la tranche retenue est "13-14 ans" (section 6) ; l'association corrige elle-même le formulaire (section 1) pour harmoniser les tranches d'âge.
