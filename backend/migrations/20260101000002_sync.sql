-- Core entities needed to persist parsed Sheet rows (specs.md §3.3, §8).
-- Commissions/postes/besoins/affectations land with the planning-generation
-- feature; this migration only covers events and raw submissions.

CREATE TABLE evenement (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    nom TEXT NOT NULL,
    date_debut DATE NOT NULL,
    date_fin DATE NOT NULL,
    lieu TEXT,
    archive BOOLEAN NOT NULL DEFAULT false,
    -- ID of the Google Sheet this event's responses come from. Several
    -- events can share the same sheet_id (2027: one form covers both
    -- events) — bloc_formulaire then says which section of a row is this
    -- event's (cf. specs.md §3.3).
    sheet_id TEXT,
    bloc_formulaire TEXT,
    created_by UUID REFERENCES admins (id),
    updated_by UUID REFERENCES admins (id),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT dates_coherentes CHECK (date_fin >= date_debut)
);

CREATE TABLE soumission (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    evenement_id UUID NOT NULL REFERENCES evenement (id),
    nom TEXT NOT NULL,
    prenom TEXT NOT NULL,
    email TEXT NOT NULL,
    telephone TEXT NOT NULL,
    tranche_age TEXT NOT NULL,
    permis_b BOOLEAN NOT NULL,
    taille_tshirt TEXT NOT NULL,
    date_soumission TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Import dedup key (specs.md §3.3): a resync must not re-insert a row
    -- already imported for this event. This is NOT the §9 doublon key
    -- (nom+prenom+email across submissions) — both submissions from a
    -- resubmit are kept, "latest wins" is applied at read time, not here.
    CONSTRAINT import_unique UNIQUE (evenement_id, date_soumission, email)
);

CREATE TABLE disponibilite_soumission (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    soumission_id UUID NOT NULL REFERENCES soumission (id) ON DELETE CASCADE,
    jour DATE NOT NULL,
    slot TEXT NOT NULL CHECK (slot IN ('matin', 'midi', 'aprem', 'soir'))
);

-- Raw, unmapped wish labels (specs.md §3.2). A label stays here until an
-- admin maps it to a Commission/Poste via the Mapping form screen; mapping
-- a label retroactively resolves every SoumissionLibelle row that has it,
-- without needing to re-sync.
CREATE TABLE soumission_libelle (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    soumission_id UUID NOT NULL REFERENCES soumission (id) ON DELETE CASCADE,
    libelle_brut TEXT NOT NULL
);
