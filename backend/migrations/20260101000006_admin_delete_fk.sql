-- Removing an admin (Admins screen, specs.md §11.4) must not get blocked
-- by every row they ever touched, nor leave their sessions dangling.
-- sessions.admin_id: the session is meaningless without the admin, so it
-- cascades. created_by/updated_by: nullable audit columns, set to NULL
-- rather than blocking the delete or losing the row they're attached to.

ALTER TABLE sessions
    DROP CONSTRAINT sessions_admin_id_fkey,
    ADD CONSTRAINT sessions_admin_id_fkey
        FOREIGN KEY (admin_id) REFERENCES admins (id) ON DELETE CASCADE;

ALTER TABLE evenement
    DROP CONSTRAINT evenement_created_by_fkey,
    ADD CONSTRAINT evenement_created_by_fkey
        FOREIGN KEY (created_by) REFERENCES admins (id) ON DELETE SET NULL,
    DROP CONSTRAINT evenement_updated_by_fkey,
    ADD CONSTRAINT evenement_updated_by_fkey
        FOREIGN KEY (updated_by) REFERENCES admins (id) ON DELETE SET NULL;

ALTER TABLE commission
    DROP CONSTRAINT commission_created_by_fkey,
    ADD CONSTRAINT commission_created_by_fkey
        FOREIGN KEY (created_by) REFERENCES admins (id) ON DELETE SET NULL,
    DROP CONSTRAINT commission_updated_by_fkey,
    ADD CONSTRAINT commission_updated_by_fkey
        FOREIGN KEY (updated_by) REFERENCES admins (id) ON DELETE SET NULL;

ALTER TABLE poste
    DROP CONSTRAINT poste_created_by_fkey,
    ADD CONSTRAINT poste_created_by_fkey
        FOREIGN KEY (created_by) REFERENCES admins (id) ON DELETE SET NULL,
    DROP CONSTRAINT poste_updated_by_fkey,
    ADD CONSTRAINT poste_updated_by_fkey
        FOREIGN KEY (updated_by) REFERENCES admins (id) ON DELETE SET NULL;

ALTER TABLE mapping_souhait_form
    DROP CONSTRAINT mapping_souhait_form_created_by_fkey,
    ADD CONSTRAINT mapping_souhait_form_created_by_fkey
        FOREIGN KEY (created_by) REFERENCES admins (id) ON DELETE SET NULL,
    DROP CONSTRAINT mapping_souhait_form_updated_by_fkey,
    ADD CONSTRAINT mapping_souhait_form_updated_by_fkey
        FOREIGN KEY (updated_by) REFERENCES admins (id) ON DELETE SET NULL;
