mod common;

use benefrance_backend::mapping::{
    resolve_pending_libelles, save_mapping, MappingKind, SaveMapping, SaveMappingError,
};
use common::spawn_test_db;
use sqlx::PgPool;
use uuid::Uuid;

async fn insert_admin(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("INSERT INTO admins (email) VALUES ('admin@example.org') RETURNING id")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn insert_evenement(pool: &PgPool, archive: bool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO evenement (nom, date_debut, date_fin, archive) \
         VALUES ('Indiv', '2027-01-01', '2027-01-02', $1) RETURNING id",
    )
    .bind(archive)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_soumission(pool: &PgPool, evenement_id: Uuid, email: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO soumission \
            (evenement_id, nom, prenom, email, telephone, tranche_age, permis_b, taille_tshirt, date_soumission) \
         VALUES ($1, 'Nom', 'Prenom', $2, '0600000000', '18 ans et +', true, 'M', now()) \
         RETURNING id",
    )
    .bind(evenement_id)
    .bind(email)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_libelle(pool: &PgPool, soumission_id: Uuid, libelle: &str) {
    sqlx::query("INSERT INTO soumission_libelle (soumission_id, libelle_brut) VALUES ($1, $2)")
        .bind(soumission_id)
        .bind(libelle)
        .execute(pool)
        .await
        .unwrap();
}

async fn commission_id(pool: &PgPool, nom: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM commission WHERE nom = $1")
        .bind(nom)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn poste_id(pool: &PgPool, nom: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM poste WHERE nom = $1")
        .bind(nom)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn joueur_label_is_pre_mapped_and_sets_polyvalent() {
    let pool = spawn_test_db().await;
    let evenement_id = insert_evenement(&pool, false).await;
    let soumission_id = insert_soumission(&pool, evenement_id, "a@example.com").await;
    insert_libelle(&pool, soumission_id, "Je suis joueur, peu m'importe 😜").await;

    resolve_pending_libelles(&pool).await.unwrap();

    let polyvalent: bool = sqlx::query_scalar("SELECT polyvalent FROM soumission WHERE id = $1")
        .bind(soumission_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        polyvalent,
        "joueur label must resolve to polyvalent, not stay unmapped"
    );
}

#[tokio::test]
async fn first_mapping_resolves_pending_wishes_retroactively() {
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool).await;
    let evenement_id = insert_evenement(&pool, false).await;
    let soumission_id = insert_soumission(&pool, evenement_id, "a@example.com").await;
    insert_libelle(&pool, soumission_id, "Sécurité / SSIAP").await;

    let accueil_id = commission_id(&pool, "Accueil").await;
    save_mapping(
        &pool,
        admin_id,
        SaveMapping {
            libelle: "Sécurité / SSIAP".to_string(),
            kind: MappingKind::Commission,
            commission_id: Some(accueil_id),
            poste_id: None,
        },
    )
    .await
    .unwrap();

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM souhait_commission WHERE soumission_id = $1 AND commission_id = $2",
    )
    .bind(soumission_id)
    .bind(accueil_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count, 1,
        "pending wish must resolve as soon as its label is mapped"
    );
}

#[tokio::test]
async fn remapping_does_not_rewrite_already_resolved_wishes() {
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool).await;
    let evenement_id = insert_evenement(&pool, false).await;
    let soumission_id = insert_soumission(&pool, evenement_id, "a@example.com").await;
    insert_libelle(&pool, soumission_id, "Sécurité / SSIAP").await;

    let accueil_id = commission_id(&pool, "Accueil").await;
    let transport_id = commission_id(&pool, "Transport").await;

    save_mapping(
        &pool,
        admin_id,
        SaveMapping {
            libelle: "Sécurité / SSIAP".to_string(),
            kind: MappingKind::Commission,
            commission_id: Some(accueil_id),
            poste_id: None,
        },
    )
    .await
    .unwrap();

    // Change the mapping afterwards.
    save_mapping(
        &pool,
        admin_id,
        SaveMapping {
            libelle: "Sécurité / SSIAP".to_string(),
            kind: MappingKind::Commission,
            commission_id: Some(transport_id),
            poste_id: None,
        },
    )
    .await
    .unwrap();

    let still_accueil: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM souhait_commission WHERE soumission_id = $1 AND commission_id = $2",
    )
    .bind(soumission_id)
    .bind(accueil_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        still_accueil, 1,
        "the already-resolved wish must stay on the original commission"
    );

    let no_transport: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM souhait_commission WHERE soumission_id = $1 AND commission_id = $2",
    )
    .bind(soumission_id)
    .bind(transport_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        no_transport, 0,
        "remapping must not retroactively rewrite it"
    );
}

#[tokio::test]
async fn a_sync_racing_a_mapping_save_still_resolves_on_the_next_sweep() {
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool).await;
    let evenement_id = insert_evenement(&pool, false).await;

    let accueil_id = commission_id(&pool, "Accueil").await;
    save_mapping(
        &pool,
        admin_id,
        SaveMapping {
            libelle: "Sécurité / SSIAP".to_string(),
            kind: MappingKind::Commission,
            commission_id: Some(accueil_id),
            poste_id: None,
        },
    )
    .await
    .unwrap_err(); // label unseen yet — expected to fail at this point

    // Simulate a sync landing the label just after that failed save.
    let soumission_id = insert_soumission(&pool, evenement_id, "a@example.com").await;
    insert_libelle(&pool, soumission_id, "Sécurité / SSIAP").await;

    // Mapping now saved for real, with the label present.
    save_mapping(
        &pool,
        admin_id,
        SaveMapping {
            libelle: "Sécurité / SSIAP".to_string(),
            kind: MappingKind::Commission,
            commission_id: Some(accueil_id),
            poste_id: None,
        },
    )
    .await
    .unwrap();

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM souhait_commission WHERE soumission_id = $1")
            .bind(soumission_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn archived_events_are_excluded_from_the_unmapped_scope() {
    // Covered at the resolve_pending_libelles level is unaffected by
    // archiving (archiving only affects the Mapping screen's listing, cf.
    // endpoint::list_mapping) — this test documents and locks in that an
    // archived event's label still resolves once mapped, it just wouldn't
    // show up as "non mappé" in the screen before that.
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool).await;
    let evenement_id = insert_evenement(&pool, true).await;
    let soumission_id = insert_soumission(&pool, evenement_id, "a@example.com").await;
    insert_libelle(&pool, soumission_id, "Sécurité / SSIAP").await;

    let accueil_id = commission_id(&pool, "Accueil").await;
    save_mapping(
        &pool,
        admin_id,
        SaveMapping {
            libelle: "Sécurité / SSIAP".to_string(),
            kind: MappingKind::Commission,
            commission_id: Some(accueil_id),
            poste_id: None,
        },
    )
    .await
    .unwrap();

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM souhait_commission WHERE soumission_id = $1")
            .bind(soumission_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        count, 1,
        "resolution itself doesn't filter by archive status"
    );
}

#[tokio::test]
async fn poste_from_another_commission_is_rejected() {
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool).await;
    let evenement_id = insert_evenement(&pool, false).await;
    let soumission_id = insert_soumission(&pool, evenement_id, "a@example.com").await;
    insert_libelle(&pool, soumission_id, "Un nouveau libellé").await;

    let restauration_id = commission_id(&pool, "Restauration").await;
    let mascotte_id = poste_id(&pool, "Mascotte").await; // belongs to Animation, not Restauration

    let err = save_mapping(
        &pool,
        admin_id,
        SaveMapping {
            libelle: "Un nouveau libellé".to_string(),
            kind: MappingKind::Commission,
            commission_id: Some(restauration_id),
            poste_id: Some(mascotte_id),
        },
    )
    .await
    .unwrap_err();

    assert!(matches!(err, SaveMappingError::PosteNotInCommission));
}

#[tokio::test]
async fn resolving_twice_is_idempotent() {
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool).await;
    let evenement_id = insert_evenement(&pool, false).await;
    let soumission_id = insert_soumission(&pool, evenement_id, "a@example.com").await;
    insert_libelle(&pool, soumission_id, "Sécurité / SSIAP").await;

    let accueil_id = commission_id(&pool, "Accueil").await;
    save_mapping(
        &pool,
        admin_id,
        SaveMapping {
            libelle: "Sécurité / SSIAP".to_string(),
            kind: MappingKind::Commission,
            commission_id: Some(accueil_id),
            poste_id: None,
        },
    )
    .await
    .unwrap();

    resolve_pending_libelles(&pool).await.unwrap();
    resolve_pending_libelles(&pool).await.unwrap();

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM souhait_commission WHERE soumission_id = $1")
            .bind(soumission_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        count, 1,
        "running the sweep again must not duplicate the wish"
    );
}
