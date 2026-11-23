-- Écran Responsables (specs.md §5.4, wireframe s9). Désigner un bénévole
-- (une `soumission`, cf. §3.3 — pas d'entité `Benevole` séparée) comme
-- responsable d'une commission, pour un événement donné. Pas de colonnes
-- `updated_by`/`updated_at` : contrairement aux tables listées en §"Traçabilité",
-- cette ligne n'est jamais modifiée, seulement créée ou supprimée (bouton
-- "Retirer") — rien à tracer après coup.
CREATE TABLE responsable_commission (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    evenement_id UUID NOT NULL REFERENCES evenement (id),
    commission_id UUID NOT NULL REFERENCES commission (id),
    soumission_id UUID NOT NULL REFERENCES soumission (id) ON DELETE CASCADE,
    created_by UUID REFERENCES admins (id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    UNIQUE (evenement_id, commission_id, soumission_id)
);

CREATE INDEX responsable_commission_soumission_idx
    ON responsable_commission (evenement_id, soumission_id);
