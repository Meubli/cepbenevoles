-- Écran Planning (specs.md §6/§7.1) — ce lot couvre uniquement le schéma
-- et le CRUD de l'affectation manuelle (§6.3 : toute affectation posée à
-- la main est "verrouillée", jamais touchée par une future génération).
-- La génération automatique elle-même (§6.2) n'est pas construite ici :
-- seul l'admin peut créer une ligne, toujours avec verrouille = true.

CREATE TABLE affectation (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    soumission_id UUID NOT NULL REFERENCES soumission (id) ON DELETE CASCADE,
    creneau_applicatif_id UUID NOT NULL REFERENCES creneau_applicatif (id) ON DELETE CASCADE,
    commission_id UUID NOT NULL REFERENCES commission (id),
    poste_id UUID REFERENCES poste (id),
    -- Toujours true pour l'instant (pas de génération automatique encore) —
    -- colonne posée dès maintenant car §6.3 en a besoin : une future
    -- affectation générée (verrouille = false) ne doit jamais être touchée
    -- une fois passée à la main par un admin.
    verrouille BOOLEAN NOT NULL DEFAULT true,
    created_by UUID REFERENCES admins (id),
    updated_by UUID REFERENCES admins (id),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Un bénévole ne peut occuper qu'une seule commission/poste par créneau.
    UNIQUE (soumission_id, creneau_applicatif_id),
    -- Même garde-fou DB que mapping_souhait_form (§3.2) : un poste_id posé
    -- doit réellement appartenir à commission_id.
    FOREIGN KEY (poste_id, commission_id) REFERENCES poste (id, commission_id)
);

CREATE INDEX affectation_creneau_idx ON affectation (creneau_applicatif_id);
CREATE INDEX affectation_soumission_idx ON affectation (soumission_id);
