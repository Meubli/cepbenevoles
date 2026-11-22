-- Admin.nom/prenom from specs.md §8 ("Admin (nom, prenom, email,
-- is_superadmin, cree_le)") were never added when the OTP auth skeleton
-- was built. Nullable: admins seeded without them (BOOTSTRAP_SUPERADMIN_EMAIL
-- at startup) have no name to give; the Admins screen requires both for an
-- admin created through it.
ALTER TABLE admins
    ADD COLUMN nom TEXT,
    ADD COLUMN prenom TEXT;
