-- Créneaux repas (specs.md §4.4) : liste nommée de créneaux applicatifs de
-- la grille générale, définie par événement et par jour, décorrélée du
-- mapping form. Remplace les règles d'enchaînement midi/soir (§4.2.2/4.2.3,
-- caduques).

CREATE TABLE creneau_repas (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    evenement_id UUID NOT NULL REFERENCES evenement (id),
    jour DATE NOT NULL,
    nom TEXT NOT NULL,
    created_by UUID REFERENCES admins (id),
    updated_by UUID REFERENCES admins (id),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX creneau_repas_evenement_idx ON creneau_repas (evenement_id, jour);

-- Membres d'un créneau repas. Min. 2 lignes par creneau_repas_id, imposé
-- côté application (pas en SQL — un ajout un par un violerait transitoirement
-- une contrainte "au moins 2").
CREATE TABLE creneau_repas_membre (
    creneau_repas_id UUID NOT NULL REFERENCES creneau_repas (id) ON DELETE CASCADE,
    creneau_applicatif_id UUID NOT NULL REFERENCES creneau_applicatif (id) ON DELETE CASCADE,

    PRIMARY KEY (creneau_repas_id, creneau_applicatif_id)
);
