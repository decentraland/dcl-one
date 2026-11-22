//! Upgrade regression for wallet and retired-identity lookup indexes.
//! The configured DSN is used only to create and destroy this test's database.
use catalyst_db::{MIGRATIONS_SQL, SCHEMA_SQL, schema_migrate};
use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};

struct OwnedDatabase {
    pool: PgPool,
    admin: PgPool,
    name: String,
}

impl OwnedDatabase {
    async fn new() -> Option<Self> {
        let url = match std::env::var("CATALYST_TEST_DATABASE_URL") {
            Ok(url) => url,
            Err(_) if std::env::var_os("GITHUB_ACTIONS").is_some() => {
                panic!("lookup index upgrade requires CATALYST_TEST_DATABASE_URL in CI")
            }
            Err(_) => return None,
        };
        let options: PgConnectOptions = url.parse().unwrap();
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone().database("postgres"))
            .await
            .unwrap();
        let name = format!("catalyst_test_lookup_{}", uuid::Uuid::new_v4().simple());
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&admin)
            .await
            .unwrap();
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect_with(
                options
                    .database(&name)
                    .options([("TimeZone", "UTC"), ("search_path", "content,meta")]),
            )
            .await
            .unwrap();
        catalyst_db::apply_schema(&pool).await.unwrap();
        Some(Self { pool, admin, name })
    }

    async fn close(self) {
        self.pool.close().await;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{}\" WITH (FORCE)",
            self.name
        )))
        .execute(&self.admin)
        .await
        .unwrap();
        self.admin.close().await;
    }
}

async fn ledger(pool: &PgPool) -> Vec<(String, String, bool)> {
    sqlx::query_as("SELECT step, sha256, created_fresh FROM meta.schema_steps ORDER BY step")
        .fetch_all(pool)
        .await
        .unwrap()
}

const INDEXES: [&str; 3] = [
    "social.private_voice_chat_participants_address_idx",
    "social.community_voice_chat_users_address_idx",
    "explore.world_scene_place_retired_identity_idx",
];

async fn definitions(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar("SELECT pg_get_indexdef(indexrelid) FROM pg_index WHERE indexrelid = ANY(ARRAY[        'social.private_voice_chat_participants_address_idx'::regclass,        'social.community_voice_chat_users_address_idx'::regclass,        'explore.world_scene_place_retired_identity_idx'::regclass]) AND indisvalid ORDER BY indexrelid::regclass::text")
        .fetch_all(pool).await.unwrap()
}

async fn lookup_results(pool: &PgPool) -> (Vec<String>, Vec<String>, Vec<String>) {
    let private = sqlx::query_scalar("SELECT address FROM social.private_voice_chat_participants WHERE address='wallet' AND (status='connected' OR (status='connection_interrupted' AND status_updated_at > timestamp '2026-01-01') OR (status='not_connected' AND joined_at > timestamp '2026-01-01')) ORDER BY call_id")
        .fetch_all(pool).await.unwrap();
    let community = sqlx::query_scalar("SELECT address FROM social.community_voice_chat_users WHERE address='wallet' AND (status='connected' OR (status='connection_interrupted' AND status_updated_at > timestamp '2026-01-01') OR (status='not_connected' AND joined_at > timestamp '2026-01-01')) ORDER BY community_id")
        .fetch_all(pool).await.unwrap();
    let namespaces = sqlx::query_scalar("SELECT scene_namespace FROM worlds.world_storage s WHERE realm='lookup.eth' AND NOT EXISTS (SELECT 1 FROM explore.world_scene_place i WHERE i.world_name=s.realm AND NOT i.active AND i.identity::text=s.scene_namespace) AND (scene_namespace <> 'world' OR NOT EXISTS (SELECT 1 FROM explore.world_scene_place i WHERE i.world_name=s.realm AND i.active)) ORDER BY scene_namespace")
        .fetch_all(pool).await.unwrap();
    (private, community, namespaces)
}

async fn facts(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar("SELECT 'private:' || to_jsonb(p)::text FROM social.private_voice_chat_participants p UNION ALL SELECT 'community:' || to_jsonb(c)::text FROM social.community_voice_chat_users c UNION ALL SELECT 'identity:' || to_jsonb(i)::text FROM explore.world_scene_place i UNION ALL SELECT 'storage:' || to_jsonb(s)::text FROM worlds.world_storage s ORDER BY 1")
        .fetch_all(pool).await.unwrap()
}

#[tokio::test]
async fn lookup_indexes_upgrade_preserves_facts_and_legacy_namespace_semantics() {
    let Some(db) = OwnedDatabase::new().await else {
        return;
    };
    let pool = &db.pool;
    let fresh = definitions(pool).await;
    assert_eq!(fresh.len(), 3);
    sqlx::raw_sql("INSERT INTO social.private_voice_chats(id,caller_address,callee_address) VALUES ('00000000-0000-0000-0000-000000000001','wallet','peer');
        INSERT INTO social.private_voice_chat_participants(call_id,address,status) VALUES ('00000000-0000-0000-0000-000000000001','wallet','connected'),('00000000-0000-0000-0000-000000000001','peer','disconnected');
        INSERT INTO social.communities(id,name,owner_address) VALUES ('00000000-0000-0000-0000-000000000002','Lookup','owner');
        INSERT INTO social.community_voice_chat_users(community_id,address,status) VALUES ('00000000-0000-0000-0000-000000000002','wallet','connected'),('00000000-0000-0000-0000-000000000002','peer','disconnected');
        INSERT INTO explore.world_scene_place(world_name,identity,entity_id,parcels,active) VALUES ('lookup.eth','00000000-0000-0000-0000-000000000003',NULL,ARRAY['0,0'],false),('lookup.eth','00000000-0000-0000-0000-000000000004','active',ARRAY['1,1'],true);
        INSERT INTO worlds.world_storage(realm,scene_namespace,key,value,value_size) VALUES ('lookup.eth','00000000-0000-0000-0000-000000000003','k','null',4),('lookup.eth','00000000-0000-0000-0000-000000000004','k','null',4),('lookup.eth','not-a-uuid','k','null',4),('lookup.eth','world','k','null',4);")
        .execute(pool).await.unwrap();
    let before_facts = facts(pool).await;
    let before_results = lookup_results(pool).await;
    assert_eq!(
        before_results,
        (
            vec!["wallet".to_string()],
            vec!["wallet".to_string()],
            vec![
                "00000000-0000-0000-0000-000000000004".to_string(),
                "not-a-uuid".to_string()
            ]
        )
    );
    let before_ledger = ledger(pool).await;
    for index in INDEXES {
        sqlx::query(sqlx::AssertSqlSafe(format!("DROP INDEX {index}")))
            .execute(pool)
            .await
            .unwrap();
    }
    assert_eq!(lookup_results(pool).await, before_results);
    let done = schema_migrate::migrate(pool, SCHEMA_SQL, MIGRATIONS_SQL)
        .await
        .unwrap();
    assert!(done.steps.is_empty());
    for index in INDEXES {
        assert!(
            done.created
                .contains(&schema_migrate::Object::Index(index.into())),
            "{done:?}"
        );
    }
    assert_eq!(definitions(pool).await, fresh);
    assert_eq!(facts(pool).await, before_facts);
    assert_eq!(lookup_results(pool).await, before_results);
    assert_eq!(ledger(pool).await, before_ledger);
    assert!(
        schema_migrate::migrate(pool, SCHEMA_SQL, MIGRATIONS_SQL)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(facts(pool).await, before_facts);
    db.close().await;
}
