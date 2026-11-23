-- Écran Créneaux (specs.md §4.1, §4.2, §4.3). Périmètre choisi avec
-- l'utilisateur : gabarit de créneaux applicatifs par jour/événement
-- (grille générale), grilles propres par commission (+ duplication d'une
-- grille existante comme point de départ), bornes matin/midi/soir
-- paramétrables par événement, et mapping créneau form -> créneaux
-- applicatifs.

-- Bornes qui catégorisent chaque créneau applicatif en matin/midi/aprem/
-- soir (§4.2 point 4), utilisées plus tard par les règles d'enchaînement :
-- un créneau dont l'heure de début tombe dans [00:00, born_fin_matin) est
-- "matin", [born_fin_matin, born_fin_midi) est "midi",
-- [born_fin_midi, born_fin_aprem) est "aprem", le reste "soir". Défauts
-- cohérents avec §4.2 (midi = 11h-13h/13h-15h, aprem par défaut 15h-17h,
-- soir à partir de 17h).
ALTER TABLE evenement ADD COLUMN born_fin_matin TIME NOT NULL DEFAULT '11:00';
ALTER TABLE evenement ADD COLUMN born_fin_midi TIME NOT NULL DEFAULT '15:00';
ALTER TABLE evenement ADD COLUMN born_fin_aprem TIME NOT NULL DEFAULT '17:00';

-- Un créneau applicatif de la grille générale d'un événement
-- (commission_id NULL) ou de la grille propre d'une commission sur cet
-- événement (commission_id NOT NULL, §4.3 — la commission doit avoir
-- grille_creneaux = 'propre', vérifié côté application). `ordre` fixe
-- l'affichage dans le jour quand deux créneaux partagent la même heure de
-- début (cas limite, pas interdit par construction).
CREATE TABLE creneau_applicatif (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    evenement_id UUID NOT NULL REFERENCES evenement (id),
    commission_id UUID REFERENCES commission (id),
    jour DATE NOT NULL,
    heure_debut TIME NOT NULL,
    heure_fin TIME NOT NULL,
    ordre INT NOT NULL DEFAULT 0,
    created_by UUID REFERENCES admins (id),
    updated_by UUID REFERENCES admins (id),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT heures_coherentes CHECK (heure_fin > heure_debut)
);

CREATE INDEX creneau_applicatif_evenement_idx
    ON creneau_applicatif (evenement_id, commission_id, jour);

-- Mapping créneau form (jour + matin/midi/aprem/soir, §3.1) -> un ou
-- plusieurs créneaux applicatifs de la grille générale (§4.1 dernier
-- point). N'a de sens que sur la grille générale (le form ne connaît pas
-- les commissions) — l'application vérifie que `creneau_applicatif_id`
-- référence bien un créneau à `commission_id IS NULL` du même jour.
CREATE TABLE creneau_form_mapping (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    evenement_id UUID NOT NULL REFERENCES evenement (id),
    jour DATE NOT NULL,
    slot_form TEXT NOT NULL CHECK (slot_form IN ('matin', 'midi', 'aprem', 'soir')),
    creneau_applicatif_id UUID NOT NULL REFERENCES creneau_applicatif (id) ON DELETE CASCADE,

    UNIQUE (evenement_id, jour, slot_form, creneau_applicatif_id)
);
