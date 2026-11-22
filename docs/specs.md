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
- **Admin** : saisit les besoins (nb bénévoles par commission/créneau), configure les créneaux applicatifs, lance la génération du planning, modifie le planning à la main, gère la liste des commissions, peut saisir manuellement une soumission bénévole (cf. 3.4).
- **Superadmin** : admin disposant en plus de la gestion du référentiel des admins (ajout, suppression, promotion) — cf. 11.4.

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
- *15 ans et + / adultes* : Je suis joueur, peu m'importe · Accueil · Transport · Restauration · Friture / cuisson / grillade · Boutique · Contrôle des accès · Hébergement ISAAC de l'ÉTOILE (nuit possible sur place) · Plateau (connaissance du déroulement d'une compétition nécessaire) · Sonorisation (connaissance du déroulement d'une compétition nécessaire) · Secrétaire de table pour la saisie des notes · Juge de ligne sur le plateau de compétition (N2 requis) · Animation

"Je suis joueur, peu m'importe" = bénévole polyvalent, affectable à n'importe quelle commission.

**Correction actée (relecture du form en ligne, nov. 2026)** : "Friture / cuisson / grillade", "Sonorisation", "Secrétaire de table pour la saisie des notes" et "Juge de ligne sur le plateau de compétition" ne sont **pas des commissions** mais des **postes**, proposés comme options distinctes dans le form au même niveau que les commissions :

- *Friture / cuisson / grillade* est un **poste de la commission Restauration** (qui reste aussi choisissable seule, sans poste précis — cf. 5.1).
- *Sonorisation*, *Secrétaire de table pour la saisie des notes* et *Juge de ligne sur le plateau de compétition* sont trois **postes de la commission Plateau** (anciennement nommée *Déroulement* — le form a été mis à jour en conséquence). L'option *Plateau* reste aussi choisissable seule, sans poste précis.
- Un souhait du form désigne donc soit une commission seule (`SouhaitCommission.poste = null`), soit une commission + un poste précis de cette commission (cf. 8, modèle de données).

Le form contient aussi des questions hors périmètre planning (compétences, matériel prêté, allergies...) — non reprises dans l'app sauf décision contraire.

### 3.2 Mapping des souhaits form → commissions/postes

Les libellés du form (section 3.1, "Poste(s) souhaité(s)") ne sont **pas interprétés automatiquement** par du matching de texte : ils sont associés une fois pour toutes par un admin, via un écran dédié, à une `Commission` ou un couple `Commission`+`Poste` du référentiel. Ce référentiel (commissions/postes) doit donc déjà exister (au moins partiellement) avant de pouvoir mapper.

- **Détection des libellés** : pour une question à cases à cocher, Google Sheets stocke les choix cochés d'une réponse sous forme de texte unique (valeurs séparées par virgule) dans une seule colonne. À chaque synchronisation (cf. 3.3), l'app découpe cette valeur et collecte l'ensemble des **libellés distincts** déjà rencontrés dans les réponses importées (toutes tranches d'âge confondues, chacune avec sa propre question).
- **Écran "Mapping form"** : liste tous les libellés détectés, avec pour chacun :
  - son statut — **mappé** (affiche la commission, et le poste le cas échéant) ou **non mappé** ;
  - une action pour l'associer/réassocier : choix d'une commission puis, si elle a des postes, un choix optionnel de poste précis (ou "aucun poste précis" = souhait de la commission seule).
- Un libellé non mappé n'est **pas perdu** : la soumission qui le contient est importée normalement (identité, disponibilités), seul ce souhait précis reste de côté tant qu'il n'est pas mappé.
- **Premier mapping d'un libellé** (jusque-là non mappé) : rétroactif — les souhaits en attente sur ce libellé, dans les soumissions déjà importées, sont résolus immédiatement (création des `SouhaitCommission` correspondants), sans attendre une nouvelle synchronisation.
- **Modification d'un mapping existant** (changer la commission/poste déjà associé à un libellé) : s'applique aux prochains souhaits résolus avec ce libellé ; **ne modifie pas rétroactivement** les `SouhaitCommission` déjà créés avec l'ancien mapping.
- L'écran de synchro (cf. 3.3) affiche un compteur de libellés non mappés, avec lien direct vers cet écran.

### 3.3 Synchronisation automatique

- Le Google Form est lié à une Google Sheet de réponses.
- Un service account accède à cette Sheet via l'API Google Sheets.
- Synchronisation : job planifié (ex. toutes les 10-15 min) **+** bouton "Synchroniser maintenant" côté admin.
- Chaque ligne de la Sheet = une **soumission**. Clé d'import = (row index Sheet + horodatage de réponse) pour ne jamais réimporter deux fois la même ligne.
- Décision actée : **pas de dédoublonnage automatique** entre soumissions (une personne qui soumet deux fois crée deux soumissions distinctes). L'admin pourra marquer manuellement deux soumissions comme doublon/fusionnées dans le planning (cf. 9).
- Les souhaits de commission sont résolus via l'écran "Mapping form" (cf. 3.2), pas par correspondance automatique de texte.
- **Avertissement Transport sans permis B** : l'écran de synchro signale (avertissement, pas un blocage) tout bénévole dont **Transport est l'unique souhait de commission** (hors "joueur, peu m'importe") et qui n'a **pas le permis B** — ce souhait ne pourra jamais être honoré par la génération (cf. 6.2, règle 5), un tel bénévole n'ayant alors aucune affectation automatique possible sur ses souhaits. Liste recalculée à chaque synchronisation, à côté des doublons potentiels (cf. 3.3).

### 3.4 Saisie manuelle d'un bénévole (hors form)

- Un admin peut créer une soumission directement dans l'app, pour un bénévole arrivé hors Google Form, via un formulaire reprenant la structure du form (section 3.1) : identité/contact (saisie une fois), puis sélection d'un ou plusieurs événements et, pour chaque événement coché, un bloc disponibilités (mêmes cases matin/midi/aprem/soir par jour, converties via le mapping habituel) + poste(s) souhaité(s) — remplissable en une seule saisie plutôt qu'un événement à la fois.
- Une soumission est créée **par événement coché**, chacune suivant ses propres règles de doublon (section 9).
- La section "poste(s) souhaité(s)" est **facultative** sur cette saisie manuelle : l'admin peut créer le bénévole sans souhait de commission et l'affecter ensuite à la main dans le planning.
- La soumission ainsi créée suit les mêmes règles que les soumissions importées (doublons, fusion — cf. section 9) ; elle n'a pas de `row_source_id` Google Sheet.

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
- Chaque commission a une **couleur**, tirée aléatoirement à sa création, modifiable par un admin (écran Commissions) **dès l'écran de création**, sans attendre d'avoir enregistré la commission ni de repasser par l'écran de modification. Cette couleur sert de code couleur dans les tableaux de planning (cf. 7.1) ; un poste n'a pas de couleur propre, il hérite de celle de sa commission.
- Liste initiale = commissions relevées en section 3.1 : Accueil, Transport, Restauration (poste *Friture/cuisson/grillade*), Boutique, Contrôle des accès, Hébergement, Plateau (postes *Sonorisation*, *Secrétariat de table*, *Juge de ligne*), Animation, Remise des récompenses...
- Certaines commissions ou postes ont des **prérequis** (ex. poste "Juge de ligne" = N2 requis, commission "Plateau" (et ses postes Sonorisation/Secrétariat de table) = connaissance du déroulement d'une compétition, commission "Hébergement" = nuit possible sur place). Ces prérequis sont déclaratifs côté form (texte dans le libellé) ; l'app n'a pas aujourd'hui de champ de compétence structuré distinct — à statuer (10.5). Le texte de prérequis peut être porté par la commission ou par un poste précis (cf. 8, modèle de données), selon à quel niveau il s'applique.
- **Besoins** : pour chaque événement, l'admin saisit le nombre de bénévoles nécessaires par **commission × créneau** (créneau de la grille générale, ou de la grille propre à la commission si elle en a une — cf. 4.3). Pour une commission qui a des postes (cf. 5.1), ce total est ensuite réparti manuellement par poste. Ne s'applique pas aux commissions **sans besoin chiffré** (cf. 5.3).

### 5.1 Postes (sous-découpage d'une commission)

Une commission peut être découpée en **postes**. Exemples : la commission *Animation* a les postes *Mascotte*, *Tombola gradins*, *Micro-trottoir*, *Stand tombola*, *Photo booth* ; la commission *Restauration* a le poste *Friture / cuisson / grillade* ; la commission *Plateau* a les postes *Sonorisation*, *Secrétariat de table*, *Juge de ligne*. D'autres commissions n'ont pas de poste du tout (ex. *Accueil*, *Transport*) et restent affectées globalement à la commission.

- Les postes sont créés par l'admin, propres à une commission.
- Le besoin est saisi au niveau **commission × créneau**. Quand la commission a des postes, l'admin peut répartir une partie de ce total par **poste × créneau** ; la somme des besoins par poste sur un créneau ne doit **jamais dépasser** le besoin de la commission sur ce même créneau (contrôle de cohérence à l'écran), mais peut lui être **strictement inférieure** : le reste correspond à des affectations sur la commission sans poste précis (ex. Restauration a un besoin de 5 sur un créneau, dont 2 répartis sur le poste Friture/cuisson/grillade — les 3 autres sont affectés à Restauration sans poste).
- Une affectation sur une commission à postes précise le poste **si l'admin ou la génération en a choisi un** ; elle peut aussi rester sans poste sur une commission qui en a (cf. point précédent). Sur une commission sans poste du tout, seule la commission est renseignée.
- Un souhait du Google Form ("Poste(s) souhaité(s)", vocabulaire historique du form) peut désigner soit une commission seule, soit une commission + un poste précis de cette commission (cf. 3.1, 8).

### 5.2 Commissions à grille de créneaux propre

La commission **Plateau** (anciennement *Déroulement*) suit des règles différentes des autres :

- Elle utilise une **grille de créneaux qui lui est propre**, distincte de la grille applicative générale de l'événement, paramétrable par l'admin (cf. 4.3). Ses postes (*Sonorisation*, *Secrétariat de table*, *Juge de ligne*) suivent la même grille que leur commission : un poste n'a pas de grille propre indépendante de celle de sa commission.
- Les bénévoles qui lui sont affectés (sur la commission ou l'un de ses postes) peuvent **enchaîner tous les créneaux de cette grille** : aucune des règles de pause / non-enchaînement midi / non-enchaînement soir (section 4.2) ne s'applique à ces affectations.
- Plus largement, **toute commission** peut être configurée par l'admin pour utiliser soit la grille applicative générale (comportement par défaut), soit une grille de créneaux qui lui est propre ; dans ce second cas les règles d'enchaînement de 4.2 ne s'appliquent pas aux affectations de cette commission (ni à celles de ses postes).

### 5.3 Commissions sans besoin chiffré (liste de disponibilité)

Certaines commissions n'ont pas de nombre de bénévoles requis à atteindre — exemple : *Installation*, *Désinstallation*. L'admin veut seulement **consulter la liste** des bénévoles disponibles sur les créneaux définis pour elle (créneau de la grille générale, ou de sa grille propre si elle en a une — cf. 4.3/5.2), et ayant souhaité cette commission **si un souhait pour elle existe dans le form** (cf. 3.1) ; sans notion d'effectif à atteindre ni de génération automatique d'affectation.

Installation et Désinstallation n'ont **pas d'option dédiée dans le Google Form** (le form ne fait qu'évoquer ces activités dans le texte des journées jeudi 29 avril et lundi 3 mai — cf. 3.1) : la liste s'appuie alors uniquement sur la disponibilité du bénévole ce jour-là, sans filtrage par souhait de commission.

**À ne pas confondre avec un besoin saisi à 0** : un `BesoinCommission` à 0 sur un créneau donné est une valeur normale (= aucun bénévole requis sur *ce* créneau précis pour cette commission, mais le concept de besoin existe et peut valoir autre chose sur un autre créneau, et la génération en tient compte). "Sans besoin chiffré" est un statut de la commission elle-même (`besoin_chiffre = false`) : il n'y a alors aucune ligne de besoin du tout, sur aucun créneau, et la commission n'entre jamais dans le calcul de la génération — seulement dans l'écran de liste.

- Une commission peut être marquée **sans besoin chiffré** (toggle "Besoin chiffré", écran Commissions — disponible aussi bien à la création d'une commission qu'en modification d'une commission existante, défaut : oui). Dans ce cas, l'écran Besoins ne propose pas de saisie pour elle (cf. 5).
- Ces commissions sont **exclues de la génération du planning** (section 6) : aucune affectation automatique n'est créée pour elles, qu'un bénévole ait coché cette commission comme souhait ou non.
- L'app propose, pour une commission sans besoin chiffré, un écran listant par créneau les bénévoles disponibles et l'ayant souhaitée (lecture simple, pas d'affectation).
- L'admin peut ensuite, si besoin, affecter manuellement certains de ces bénévoles dans le planning (affectation manuelle classique, verrouillée — cf. 6.3).
- Une commission sans besoin chiffré peut avoir des postes (5.1) ; dans ce cas la liste de disponibilité ne descend pas au niveau poste, seulement commission × créneau.

### 5.4 Responsables de commission

Pour un événement donné, l'admin peut désigner un ou plusieurs bénévoles comme **responsable(s)** d'une commission.

- Un bénévole peut être responsable de **plusieurs commissions à la fois**, sur un même événement.
- Être responsable ne nécessite pas d'affectation à un poste. Le responsable **peut en plus** être affecté à un **poste de sa propre commission** sans que cela entre en conflit avec son rôle de responsable — mais **uniquement à la main** : la génération automatique (section 6) n'affecte **jamais** un responsable de commission à un poste, y compris un poste de sa propre commission. Seule une affectation manuelle peut le faire.
- Être responsable d'une commission rend le bénévole **occupé sur tous les créneaux où cette commission opère** (sa grille, générale ou propre), sur toute la durée de l'événement — même sans disponibilité déclarée ni affectation explicite sur ces créneaux.
- Un responsable ne peut pas être affecté (commission ou poste) sur un créneau qui chevauche un créneau où il est occupé en tant que responsable d'une **autre** commission, **sauf s'il est aussi responsable de cette autre commission**. Cumuler plusieurs responsabilités en simultané est **toujours permis**, même si les commissions concernées ont des créneaux qui se chevauchent — il n'y a jamais de "conflit" entre deux rôles de responsable eux-mêmes, seulement entre un rôle de responsable et une affectation réelle (commission/poste) hors de ses propres commissions.
- **Résolu** : les créneaux où un bénévole est "occupé" en tant que responsable **ne comptent pas** dans l'équilibrage de charge (6.2, point 2) ni dans les règles de pause/enchaînement (4.2). Seules les affectations réelles (commission/poste) sont prises en compte pour ces deux calculs ; le rôle de responsable n'est qu'une contrainte d'exclusivité sur les affectations, pas une charge de travail comptabilisée.
- **Affichage** : un badge "Responsable" accompagne le nom du bénévole **partout où il apparaît** (tableau planning — toutes dispositions —, détail individuel, recherche, listes admin), tant qu'il est responsable d'au moins une commission de l'événement affiché.
- Écran admin dédié, par événement, pour désigner/retirer les responsables d'une commission.

### 5.5 Responsables de poste

Pour un événement donné, l'admin peut désigner un ou plusieurs bénévoles comme **responsable(s) d'un poste** (sous-découpage d'une commission, cf. 5.1).

- Une commission **sans poste** ne peut pas avoir de responsable de poste (le concept n'existe qu'au niveau poste).
- Un poste peut avoir **plusieurs responsables à la fois**, comme pour les commissions (5.4).
- Un bénévole peut être à la fois **responsable de poste** et **responsable de la commission** de ce poste (ou d'une autre), sans restriction ni conflit.
- **Contrairement au responsable de commission**, être responsable de poste est une **étiquette purement informative** : cela n'occupe aucun créneau, ne crée aucune exclusivité, et n'a **aucun effet** sur la génération du planning, l'équilibrage de charge ou les règles de pause/enchaînement.
- **Affichage**, limité à deux endroits :
  - Dans le tableau planning, **uniquement en disposition "Créneaux en lignes"** (jamais en "Bénévoles en lignes") : un badge "Responsable de poste" à côté du bénévole concerné.
  - Dans le **détail individuel** du bénévole : badge plus explicite (texte complet, ex. "Responsable du poste Juge de ligne"), à côté de l'affectation concernée.
- **Désignation** : depuis le tableau en disposition "Créneaux en lignes", un clic sur un bénévole ouvre un menu avec deux choix : *Voir le détail* (ouvre le détail individuel, cf. 7.2) ou *Passer responsable de poste* / *Retirer responsable de poste* (bascule son statut sur le poste de la cellule cliquée, pour l'événement en cours). Pas d'écran admin dédié séparé pour ce rôle : la désignation se fait uniquement depuis cette interaction.

## 6. Génération du planning

### 6.1 Entrées

- **Pré-requis bloquants** — la génération est **impossible** tant que :
  - il reste au moins un libellé "Poste(s) souhaité(s)" non mappé (cf. 3.2), tous événements confondus ;
  - **toute commission participant à la génération** (c'est-à-dire ayant `besoin_chiffre = true`, cf. 5.3 — les commissions sans besoin chiffré n'entrent pas dans ce calcul) **n'a pas au moins un responsable** désigné sur l'événement concerné (cf. 5.4).
  
  Le bouton "Générer le planning" est désactivé tant que l'une de ces conditions n'est pas remplie, et renvoie vers l'écran concerné ("Mapping form" ou "Responsables") selon le blocage.
- Bénévoles (soumissions) avec leurs disponibilités (déduites via le mapping form → créneaux applicatifs) et leurs souhaits de commission.
- Besoins par commission × créneau (section 5), pour les commissions qui en ont (hors commissions sans besoin chiffré, cf. 5.3).
- Règles d'enchaînement (section 4.2).
- Responsables de commission (cf. 5.4) : un bénévole responsable d'une commission est traité comme occupé sur tous les créneaux de cette commission, et n'est jamais lui-même affecté par la génération — ni sur une autre commission/poste en conflit avec ce rôle, ni sur un poste de sa propre commission (cf. 5.4) : un responsable de commission n'est **jamais** affecté automatiquement à un poste, seulement à la main.

### 6.2 Algorithme — contraintes actées

1. Respecter autant que possible le(s) souhait(s) de commission du bénévole (un "joueur" est affectable à toute commission en tension).
2. Équilibrer la charge totale (nombre de créneaux travaillés) entre bénévoles.
3. Ne jamais violer les règles de pause / non-enchaînement midi / non-enchaînement soir.
4. Ne jamais affecter un responsable de commission sur un créneau en conflit avec son rôle de responsable, ni l'affecter automatiquement à un poste — sa propre commission comme une autre (cf. 5.4) : un responsable n'est affecté à un poste qu'à la main.
5. Ne jamais affecter un bénévole sans permis B à la commission **Transport** (champ `permis_b` de la soumission, cf. 3.1/8).

**Note** : contrairement aux prérequis déclaratifs de 10.5 (N2, connaissance du déroulement — jamais vérifiés par l'app), la contrainte permis B ↔ Transport est **appliquée par l'algorithme** : un bénévole sans permis B n'est jamais retenu par la génération pour Transport, même s'il l'a souhaité et même en tension d'effectif. Une affectation manuelle d'un admin sur Transport n'est, elle, pas techniquement bloquée (comme toute affectation manuelle, cf. 6.3) mais reste déconseillée dans ce cas.

### 6.3 Non-régression sur modifications manuelles

- Toute affectation modifiée à la main par un admin est marquée **verrouillée**.
- Une nouvelle génération recalcule uniquement les affectations non verrouillées ; elle ne touche jamais une affectation verrouillée, et prend les affectations verrouillées comme contraintes fixes (un verrouillé compte dans la charge du bénévole et dans l'effectif déjà couvert de sa commission/créneau).

## 7. Dashboards & écrans

### 7.1 Vue planning événement / jour (vue principale)

- Sélection d'un événement, puis d'une date (jour) de cet événement.
- Tableau : lignes = bénévoles triés par nom de famille puis prénom ; colonnes = groupe "identité" bénévole + une colonne par créneau applicatif du jour ; cellule = commission(s) affectée(s) à ce bénévole sur ce créneau, mise en couleur selon la couleur de la commission (cf. 5).
- **Total bénévoles**, au-dessus du tableau : nombre de bénévoles de la liste actuellement affichée (lignes filtrées), chacun compté une seule fois.
- **Ligne de total par créneau**, en haut du tableau (sous l'en-tête des colonnes) : nombre de bénévoles affectés sur chaque créneau, calculé sur les lignes actuellement affichées (donc filtré si un filtre commission/poste est actif — cf. ci-dessous). Un même bénévole peut compter dans plusieurs colonnes ; ce total par créneau n'est donc pas censé égaler le total bénévoles ci-dessus.
- **Filtre par commission** : limite les lignes du tableau et le recalcul de la ligne de total aux bénévoles affectés à la commission choisie sur au moins un créneau du jour. Quand la commission filtrée a des postes (cf. 5.1), un **filtre par poste** supplémentaire apparaît, pour affiner sur un poste précis de cette commission.
- Clic sur un bénévole → détail de son planning individuel sur **tout l'événement** (tous les jours). En disposition *Créneaux en lignes*, ce clic ouvre d'abord un menu à deux choix : *Voir le détail* (comportement ci-dessus) ou *Passer/retirer responsable de poste* sur le poste de la cellule cliquée (cf. 5.5) — sans effet si la cellule n'a pas de poste (commission sans poste).
- Filtre/recherche de bénévole, pour accéder rapidement à son planning individuel.
- **Disposition du tableau**, deux modes au choix :
  - *Bénévoles en lignes* (par défaut) : lignes = bénévoles, colonnes = créneaux du jour, cellule = commission(s)/poste affecté(s) — décrit ci-dessus.
  - *Créneaux en lignes* : lignes = créneaux du jour, colonnes = postes de la commission filtrée (ou la commission seule, si elle n'a pas de poste), cellule = bénévole(s) affecté(s) sur ce poste/cette commission, ce créneau (plusieurs bénévoles possibles dans une même cellule, empilés). **Le filtre par commission est obligatoire dans cette disposition** (cf. point suivant) : il n'existe pas de vue "toutes commissions" en créneaux-en-lignes.
  - Les filtres (commission, poste, recherche bénévole) et la ligne/colonne de total s'appliquent de la même façon dans les deux dispositions, sous réserve du point ci-dessus.
  - Passer en disposition *Créneaux en lignes* **force la sélection d'une commission** : si le filtre commission était sur "Toutes" au moment du changement de disposition, la première commission de la liste est sélectionnée automatiquement ; l'option "Toutes" est désactivée tant que cette disposition reste active.
- **Affichage du témoin "responsable"** (cf. 5.4) :
  - En disposition *Bénévoles en lignes* : sous le nom du bénévole, un repère "R-\<Commission\>" par commission dont il est responsable, empilés verticalement s'il y en a plusieurs.
  - En disposition *Créneaux en lignes* (toujours filtrée sur une commission, cf. ci-dessus) : les responsables de la commission filtrée **n'apparaissent jamais dans les cellules du tableau**, même s'ils sont aussi affectés à un poste précis ce jour-là ; ils sont listés à part, **au-dessus du tableau, alignés à droite**.
- **Affichage du témoin "responsable de poste"** (cf. 5.5), limité à deux endroits, jamais ailleurs (ni en disposition *Bénévoles en lignes*, ni en recherche, ni en listes admin) :
  - En disposition *Créneaux en lignes* : badge "Responsable de poste" à côté du bénévole concerné, dans la cellule du poste.
  - Au détail individuel (7.2) : badge plus explicite, nommant le poste (ex. "Responsable du poste Juge de ligne").
- **Version imprimable** : le planning affiché (dans la disposition active — *Bénévoles en lignes* ou *Créneaux en lignes*) peut être généré en version imprimable/PDF, avec en en-tête l'événement, le jour et les filtres actifs (commission/poste), et la légende des couleurs par commission ; sans les contrôles d'interface (boutons, navigation, menus).
- **Mise à jour en temps réel** : quand un admin modifie le planning (affectation manuelle, génération, responsable...), les autres admins ayant le même événement/jour ouvert voient le tableau se rafraîchir automatiquement, sans avoir à recharger la page.

### 7.2 Détail individuel

- Planning du bénévole sur l'événement en cours.
- Bouton de switch pour afficher son planning sur un **autre événement** (facilement accessible, sans ressaisir de recherche).
- Si le bénévole est responsable d'un ou plusieurs postes (cf. 5.5) sur l'événement affiché, un badge explicite ("Responsable du poste \<Poste\>") apparaît à côté de chaque affectation/poste concerné.

### 7.3 Écrans admin

- Saisie des besoins par commission × créneau (événement).
- Configuration des créneaux applicatifs (gabarit par défaut + override par jour/événement), des bornes matin/midi/soir, et des grilles de créneaux propres à certaines commissions (cf. 4.3).
- Gestion du référentiel de commissions (ajout/désactivation, postes par commission, choix grille générale ou grille propre, commission sans besoin chiffré — cf. 5.3).
- Pour une commission sans besoin chiffré : écran de liste des bénévoles disponibles/souhaitant cette commission, par créneau.
- Gestion des responsables de commission par événement (ajout/retrait, cf. 5.4).
- Lancement de la génération du planning + visualisation des affectations verrouillées vs générées.
- Export/impression du planning (cf. 7.1), dans les deux dispositions.
- Suivi de synchronisation Google Form (dernière synchro, nombre de nouvelles soumissions, bouton synchroniser).
- Écran "Mapping form" : liste des libellés détectés dans les réponses du form, mappés ou non à une commission/poste, avec action d'association (cf. 3.2).
- Envoi (manuel) du planning individuel par email aux bénévoles, événement par événement, depuis une boîte mail dédiée à l'association.

## 8. Modèle de données (entités pressenties)

- `Evenement` (nom, dates, archive: bool — défaut faux, cf. 12.6)
- `CreneauApplicatif` (evenement, commission nullable — renseigné seulement pour la grille propre d'une commission, jour, heure\_debut, heure\_fin, ordre)
- `MappingCreneauForm` (evenement, jour, libelle\_form \[matin/midi/aprem/soir\] → liste de CreneauApplicatif)
- `Commission` (nom, actif, prerequis\_libre\_texte, a\_des\_postes: bool, grille\_creneaux \[generale|propre\], besoin\_chiffre: bool, couleur — aléatoire à la création, modifiable)
- `BesoinCommission` (evenement, creneau\_applicatif, commission, nb\_requis) — nb\_requis total de la commission sur ce créneau
- `Soumission` (evenement, nom, prenom, email, telephone, tranche\_age, permis\_b, taille\_tshirt, date\_soumission, row\_source\_id, benevole\_id nullable)
- `DisponibiliteSoumission` (soumission, jour, libelle\_form)
- `SouhaitCommission` (soumission, commission, poste nullable) — poste renseigné quand le souhait du form désignait un poste précis de la commission (cf. 3.1, 5.1)
- `MappingSouhaitForm` (libelle\_form, commission nullable, poste nullable) — association admin entre un libellé brut du form et une commission/poste du référentiel ; nullable tant que non mappé (cf. 3.2)
- `Affectation` (evenement, soumission, creneau\_applicatif, commission, poste nullable, origine \[genere|manuel\], verrouille: bool)
- `ResponsableCommission` (evenement, commission, benevole) — être responsable n'est pas une `Affectation` : pas de créneau/poste propre, occupation déduite de la grille de la commission (cf. 5.4)
- `ResponsablePoste` (evenement, poste, benevole) — étiquette purement informative, sans effet sur la génération ni l'occupation (cf. 5.5)
- `Benevole` (nom, prenom, email, profil canonique issu d'une fusion de soumissions)
- `Poste` (commission, nom, prerequis\_libre\_texte nullable)
- `BesoinPoste` (evenement, creneau\_applicatif, commission, poste, nb\_requis) — somme par créneau ≤ nb\_requis de BesoinCommission (peut être strictement inférieure, cf. 5.1)
- `Admin` (nom, prenom, email, is\_superadmin: bool, cree\_le)
- `LoginChallenge` (email, code\_hash, expire\_le, tentatives, utilise: bool)
- `Session` (admin\_id, token\_hash, cree\_le, derniere\_activite, expire\_le)
- **Traçabilité** : les tables métier modifiables (`Affectation`, `BesoinCommission`, `BesoinPoste`, `Commission`, `Poste`, `MappingSouhaitForm`, `ResponsableCommission`, `ResponsablePoste`, `Evenement`) portent `created_by`/`updated_by` (référence `Admin`) + `updated_at` — admin ayant créé/modifié en dernier.

## 9. Doublons de soumission

Clé d'identification d'une soumission : nom + prénom + email.

- **Même clé, même événement** : doublon. La **dernière soumission** (la plus récente) fait foi pour les disponibilités et les souhaits de commission de cet événement ; l'ancienne reste conservée en base mais n'est plus prise en compte par défaut.
- **Même clé, événements différents** : ce n'est pas un doublon, c'est le même bénévole inscrit aux deux événements (cas normal, ex. une soumission par événement). L'admin peut alors **fusionner** ces soumissions en un seul profil bénévole, pour que son planning individuel agrège les deux événements (cf. 7.2). La fusion est toujours une action manuelle de l'admin, jamais automatique.

## 11. Authentification admin

### 11.1 Connexion sans mot de passe

- Pas de mot de passe stocké en base. Un admin est identifié par nom + prénom + email.
- Connexion par **code à usage unique envoyé par email** ("code challenge") : l'admin saisit son email, reçoit un code (ex. 6 chiffres, validité ~10 min), le saisit pour ouvrir une session.
- Si l'email ne correspond à aucun admin, pas de code envoyé (pas de création de compte libre). La réponse de l'UI reste la même dans les deux cas (email connu ou non) pour ne pas révéler quels emails sont admins.

### 11.2 Protection anti brute-force

Point sensible du fait de l'exposition sur internet (pas de réseau local uniquement) : le seul facteur est un code à 6 chiffres, donc l'app applique des limites strictes à la fois sur la **demande** de code et sur sa **vérification**.

- **Vérification du code** : un `LoginChallenge` accepte au maximum **5 tentatives** (`tentatives`, cf. 8). Au-delà, il est invalidé (`utilise = true` sans jamais avoir été validé) ; l'admin doit redemander un nouveau code. Chaque tentative incrémente le compteur, succès ou échec.
- **Demande de code** : limite de fréquence par email (ex. 1 demande toutes les 60 secondes, max 5 par heure) **et** par IP (ex. max 10 demandes par heure, tous emails confondus), pour empêcher le spam d'emails et l'épuisement de ressources. Au-delà, la demande est silencieusement ignorée côté utilisateur (pas d'indice sur la raison, même traitement que email inconnu) mais peut être tracée côté serveur pour supervision.
- **Résolu (implémentation)** : compteurs tenus en base (Postgres), pas de service de cache externe (Redis/Memcached) — volume trop faible pour le justifier (cf. `technical-specs.md` §5).
- **Expiration courte** : un code expire ~10 min après génération (cf. 11.1) ; demander un nouveau code avant expiration invalide l'ancien (un seul code actif à la fois par email).
- Ces compteurs/limites sont appliqués **côté application** (indépendamment de toute protection réseau type fail2ban, qui reste une question de configuration d'hébergement traitée séparément).

### 11.3 Session

- Session stockée côté serveur (révocable), cookie httpOnly/secure/SameSite, pas de JWT stateless.
- Durée de vie longue (plusieurs mois) avec expiration glissante : renouvelée à chaque activité, pas de ressaisie du code à chaque visite. Choix accepté car les données traitées (planning bénévoles) sont peu sensibles ; risque résiduel = vol de cookie ou compromission de la boîte mail de l'admin.
- **Déconnexion manuelle** disponible à tout moment (barre de navigation), détruit la session côté serveur.

### 11.4 Gestion des admins et rôle superadmin

- Deux rôles : **admin** (accès fonctionnel standard — planning, besoins, créneaux, commissions, bénévoles, synchro) et **superadmin** (admin + écran de gestion des admins).
- Seul un superadmin peut ajouter, supprimer ou promouvoir un admin. Un admin normal n'a pas accès à cet écran.
- Premiers admins (au moins un superadmin) créés via un outil de gestion de changements de base de données (migration/seed), pas via l'app, au démarrage du projet.
- Ajouter un admin ne demande pas de mot de passe : il se connecte directement via le flux 11.1 une fois son email enregistré.

## 12. Questions ouvertes

1. Résolu : "aprem" = 15h-17h par défaut (cf. section 4.2).
2. Gabarit de créneaux applicatifs par défaut (nombre et horaires précis) à valider avec l'association.
3. **Résolu : les compétences/prérequis (N2, connaissance du déroulement) ne sont pas pris en compte dans la génération ; ils restent déclaratifs (texte libre sur la commission).**
4. Résolu : l'application est réservée aux admins ; les bénévoles n'ont pas de compte.
5. Résolu : notification par email, envoyée manuellement par un admin, événement par événement, depuis une boîte mail dédiée à l'association (pas d'envoi automatique depuis l'application).
6. **Résolu (archivage, pas la réutilisation multi-saison)** : un admin peut archiver/désarchiver un `Evenement` (`archive: bool`, cf. 8). Archiver un événement le masque des listes/écrans actifs (planning, besoins, génération, responsables...) mais **ne le rend pas lecture seule** : un événement désarchivé redevient modifiable normalement. La réutilisation/duplication de config d'une année sur l'autre reste hors périmètre, à retraiter plus tard.
7. Résolu : la tranche retenue est "13-14 ans" (section 6) ; l'association corrige elle-même le formulaire (section 1) pour harmoniser les tranches d'âge.
8. Résolu : les créneaux "occupés" par un responsable de commission (cf. 5.4) ne comptent ni dans l'équilibrage de charge (6.2) ni dans les règles de pause/enchaînement (4.2) ; cumuler plusieurs responsabilités en simultané est toujours permis, même en cas de chevauchement entre leurs commissions.
