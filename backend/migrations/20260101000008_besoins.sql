-- Écran Besoins (specs.md §5, wireframe s3). Besoin saisi au niveau
-- commission × créneau applicatif ; pour une commission à postes, une
-- partie de ce total peut être répartie par poste × créneau (§5.1). Les
-- commissions "sans besoin chiffré" (§5.3) n'ont jamais de ligne ici —
-- vérifié côté application, pas en contrainte SQL (la table ne sait pas
-- quelles commissions sont concernées sans relire `commission`).

CREATE TABLE besoin_commission (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    evenement_id UUID NOT NULL REFERENCES evenement (id),
    creneau_applicatif_id UUID NOT NULL REFERENCES creneau_applicatif (id) ON DELETE CASCADE,
    commission_id UUID NOT NULL REFERENCES commission (id),
    nb_requis INT NOT NULL CHECK (nb_requis >= 0),
    created_by UUID REFERENCES admins (id),
    updated_by UUID REFERENCES admins (id),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    UNIQUE (creneau_applicatif_id, commission_id)
);

-- `commission_id` dupliqué depuis `poste` (plutôt qu'un simple
-- `poste_id`) pour que la contrainte composite ci-dessous vérifie en base
-- que le poste appartient bien à la commission indiquée — même mécanisme
-- que `souhait_commission`/`mapping_souhait_form` (20260101000004).
CREATE TABLE besoin_poste (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    evenement_id UUID NOT NULL REFERENCES evenement (id),
    creneau_applicatif_id UUID NOT NULL REFERENCES creneau_applicatif (id) ON DELETE CASCADE,
    commission_id UUID NOT NULL REFERENCES commission (id),
    poste_id UUID NOT NULL REFERENCES poste (id),
    nb_requis INT NOT NULL CHECK (nb_requis >= 0),
    created_by UUID REFERENCES admins (id),
    updated_by UUID REFERENCES admins (id),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    UNIQUE (creneau_applicatif_id, poste_id),
    FOREIGN KEY (poste_id, commission_id) REFERENCES poste (id, commission_id)
);
