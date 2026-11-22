-- Commission/Poste referential (specs.md §5, §5.1) and the Mapping form
-- screen's own tables (§3.2). Seeded with the commissions/postes already
-- known from the real form (§3.1) — the Commissions admin screen to
-- add/edit more doesn't exist yet, this unblocks mapping in the meantime.

CREATE TABLE commission (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    nom TEXT NOT NULL UNIQUE,
    actif BOOLEAN NOT NULL DEFAULT true,
    prerequis_libre_texte TEXT,
    a_des_postes BOOLEAN NOT NULL DEFAULT false,
    grille_creneaux TEXT NOT NULL DEFAULT 'generale' CHECK (grille_creneaux IN ('generale', 'propre')),
    besoin_chiffre BOOLEAN NOT NULL DEFAULT true,
    couleur TEXT NOT NULL,
    created_by UUID REFERENCES admins (id),
    updated_by UUID REFERENCES admins (id),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE poste (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    commission_id UUID NOT NULL REFERENCES commission (id),
    nom TEXT NOT NULL,
    prerequis_libre_texte TEXT,
    created_by UUID REFERENCES admins (id),
    updated_by UUID REFERENCES admins (id),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    UNIQUE (commission_id, nom),
    -- Lets mapping_souhait_form's composite FK below enforce "this poste
    -- really belongs to this commission" at the database level.
    UNIQUE (id, commission_id)
);

-- Raw wish labels this project already detected (specs.md §3.2) are
-- resolved against this table; a label not yet listed here is simply
-- "non mappé" at the Mapping form screen until an admin adds it.
CREATE TABLE mapping_souhait_form (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    libelle TEXT NOT NULL UNIQUE,
    -- 'polyvalent': the "Je suis joueur, peu m'importe" wish — assignable
    -- to any commission in tension, never itself "non mappé" (§6.2 rule 1).
    -- 'commission': a specific commission (+ optional poste).
    kind TEXT NOT NULL CHECK (kind IN ('commission', 'polyvalent')),
    commission_id UUID REFERENCES commission (id),
    poste_id UUID REFERENCES poste (id),
    created_by UUID REFERENCES admins (id),
    updated_by UUID REFERENCES admins (id),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT kind_matches_target CHECK (
        (kind = 'polyvalent' AND commission_id IS NULL AND poste_id IS NULL)
        OR (kind = 'commission' AND commission_id IS NOT NULL)
    ),
    FOREIGN KEY (poste_id, commission_id) REFERENCES poste (id, commission_id)
);

-- Seed the one special label every form will ever have (specs.md §3.1):
-- the real text, emoji included — this is what's actually in the Sheet.
INSERT INTO mapping_souhait_form (libelle, kind)
VALUES ('Je suis joueur, peu m''importe 😜', 'polyvalent');

-- Whether this submission marked itself "joueur, peu m'importe" — a flag,
-- not a SouhaitCommission row, since it doesn't name any one commission.
ALTER TABLE soumission ADD COLUMN polyvalent BOOLEAN NOT NULL DEFAULT false;

-- When a label gets mapped, every SoumissionLibelle row already using it is
-- resolved into a SouhaitCommission (or Soumission.polyvalent) and marked
-- here — this is what makes the retroactive-resolution rule (§3.2) and the
-- "a sync racing a mapping save" case both just "run the same sweep again".
ALTER TABLE soumission_libelle ADD COLUMN resolved_at TIMESTAMPTZ;

CREATE TABLE souhait_commission (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    soumission_id UUID NOT NULL REFERENCES soumission (id) ON DELETE CASCADE,
    commission_id UUID NOT NULL REFERENCES commission (id),
    poste_id UUID REFERENCES poste (id),

    UNIQUE NULLS NOT DISTINCT (soumission_id, commission_id, poste_id)
);

-- Seed data, cf. specs.md §3.1/§5. Colors are placeholders an admin can
-- change at any time (couleur is "random at creation, editable").
INSERT INTO commission (nom, couleur, a_des_postes) VALUES
    ('Accueil', '#5b8dd9', false),
    ('Transport', '#dba23a', false),
    ('Boutique', '#4fa3a3', false),
    ('Contrôle des accès', '#7a8fa0', false),
    ('Remise des récompenses', '#c98fd9', false);

INSERT INTO commission (nom, couleur, a_des_postes, prerequis_libre_texte) VALUES
    ('Hébergement ISAAC de l''ÉTOILE', '#b08a5a', false, 'Nuit possible sur place');

INSERT INTO commission (nom, couleur, a_des_postes) VALUES
    ('Restauration', '#c05a4f', true);
INSERT INTO poste (commission_id, nom)
    SELECT id, 'Friture / cuisson / grillade' FROM commission WHERE nom = 'Restauration';

INSERT INTO commission (nom, couleur, a_des_postes) VALUES
    ('Animation', '#d98fb3', true);
INSERT INTO poste (commission_id, nom)
    SELECT id, unnest(ARRAY['Mascotte', 'Tombola gradins', 'Micro-trottoir', 'Stand tombola', 'Photo booth'])
    FROM commission WHERE nom = 'Animation';

INSERT INTO commission (nom, couleur, a_des_postes, grille_creneaux, prerequis_libre_texte) VALUES
    ('Plateau', '#9065c9', true, 'propre', 'Connaissance du déroulement d''une compétition');
INSERT INTO poste (commission_id, nom, prerequis_libre_texte)
    SELECT id, 'Sonorisation', 'Connaissance du déroulement d''une compétition' FROM commission WHERE nom = 'Plateau';
INSERT INTO poste (commission_id, nom)
    SELECT id, 'Secrétariat de table' FROM commission WHERE nom = 'Plateau';
INSERT INTO poste (commission_id, nom, prerequis_libre_texte)
    SELECT id, 'Juge de ligne', 'N2 requis' FROM commission WHERE nom = 'Plateau';

INSERT INTO commission (nom, couleur, besoin_chiffre) VALUES
    ('Installation', '#8a8578', false),
    ('Désinstallation', '#8a8578', false);

-- The real form's labels (§3.1) mapped once for all, so the Mapping form
-- screen starts without them showing up as "non mappé".
INSERT INTO mapping_souhait_form (libelle, kind, commission_id)
    SELECT 'Accueil', 'commission', id FROM commission WHERE nom = 'Accueil';
INSERT INTO mapping_souhait_form (libelle, kind, commission_id)
    SELECT 'Transport', 'commission', id FROM commission WHERE nom = 'Transport';
INSERT INTO mapping_souhait_form (libelle, kind, commission_id)
    SELECT 'Restauration', 'commission', id FROM commission WHERE nom = 'Restauration';
INSERT INTO mapping_souhait_form (libelle, kind, commission_id, poste_id)
    SELECT 'Friture / cuisson / grillade', 'commission', c.id, p.id
    FROM commission c JOIN poste p ON p.commission_id = c.id
    WHERE c.nom = 'Restauration' AND p.nom = 'Friture / cuisson / grillade';
INSERT INTO mapping_souhait_form (libelle, kind, commission_id)
    SELECT 'Boutique', 'commission', id FROM commission WHERE nom = 'Boutique';
INSERT INTO mapping_souhait_form (libelle, kind, commission_id)
    SELECT 'Contrôle des accès', 'commission', id FROM commission WHERE nom = 'Contrôle des accès';
INSERT INTO mapping_souhait_form (libelle, kind, commission_id)
    -- Real form text has no accent on "ETOILE" and uses uppercase — verified
    -- against the live form (section 6), do not "correct" it.
    SELECT 'Hébergement ISAAC de l''ETOILE (nuit possible sur place)', 'commission', id
    FROM commission WHERE nom = 'Hébergement ISAAC de l''ÉTOILE';
INSERT INTO mapping_souhait_form (libelle, kind, commission_id)
    SELECT 'Plateau (connaissance du déroulement d''une compétition nécessaire)', 'commission', id
    FROM commission WHERE nom = 'Plateau';
INSERT INTO mapping_souhait_form (libelle, kind, commission_id, poste_id)
    SELECT 'Sonorisation (connaissance du déroulement d''une compétition nécessaire)', 'commission', c.id, p.id
    FROM commission c JOIN poste p ON p.commission_id = c.id
    WHERE c.nom = 'Plateau' AND p.nom = 'Sonorisation';
INSERT INTO mapping_souhait_form (libelle, kind, commission_id, poste_id)
    SELECT 'Secrétaire de table pour la saisie des notes', 'commission', c.id, p.id
    FROM commission c JOIN poste p ON p.commission_id = c.id
    WHERE c.nom = 'Plateau' AND p.nom = 'Secrétariat de table';
INSERT INTO mapping_souhait_form (libelle, kind, commission_id, poste_id)
    SELECT 'Juge de ligne sur le plateau de compétition (N2 requis)', 'commission', c.id, p.id
    FROM commission c JOIN poste p ON p.commission_id = c.id
    WHERE c.nom = 'Plateau' AND p.nom = 'Juge de ligne';
INSERT INTO mapping_souhait_form (libelle, kind, commission_id)
    SELECT 'Animation', 'commission', id FROM commission WHERE nom = 'Animation';
INSERT INTO mapping_souhait_form (libelle, kind, commission_id)
    SELECT 'Remise des récompenses lors des palmarès', 'commission', id
    FROM commission WHERE nom = 'Remise des récompenses';
