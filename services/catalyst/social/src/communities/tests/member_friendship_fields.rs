//! Actual source handlers/components/SQL: member-friendship-fields.json.
use super::*;

fn replace_seed(value: &mut Value, replacements: &std::collections::HashMap<String, String>) {
    match value {
        Value::String(text) => {
            if let Some(replacement) = replacements.get(text) {
                *text = replacement.clone();
            }
        }
        Value::Array(values) => values
            .iter_mut()
            .for_each(|v| replace_seed(v, replacements)),
        Value::Object(values) => values
            .values_mut()
            .for_each(|v| replace_seed(v, replacements)),
        _ => {}
    }
}

#[tokio::test]
async fn v1_member_and_ban_action_aliases_match_full_source_bodies_with_v2_anonymous_controls() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let fixture: Value = serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"), "/services/catalyst/tests/fixtures/conformance/social/source-contracts/member-friendship-fields.json"))).unwrap();
    let (viewer, peer) = (wallet(), wallet());
    let id = rig.community(&viewer, false).await;
    rig.member(id, &peer, "member").await;
    rig.profile(&viewer, "Profile", true).await;
    rig.profile(&peer, "Profile", true).await;
    // Match the source registry fixture's raw JSON number before either cache reads it.
    sqlx::query("UPDATE content.deployments SET entity_metadata=jsonb_set(entity_metadata::jsonb,'{v,avatars,0,nameColor,b}','1'::jsonb)::json WHERE entity_id IN (SELECT entity_id FROM content.active_pointers WHERE pointer=ANY($1))")
        .bind(vec![viewer.address.to_string(),peer.address.to_string()]).execute(&rig.pool).await.unwrap();
    sqlx::query("UPDATE social.community_members SET joined_at=CASE WHEN member_address=$2 THEN '2024-01-01 00:00:00'::timestamp ELSE '2024-01-01 00:00:01'::timestamp END WHERE community_id=$1")
        .bind(id).bind(&viewer.address).execute(&rig.pool).await.unwrap();
    sqlx::query("INSERT INTO social.community_bans(community_id,banned_address,banned_by,banned_at,active) VALUES($1,$2,$3,'2024-01-01 00:00:00',true)")
        .bind(id).bind(&peer.address).bind(&viewer.address).execute(&rig.pool).await.unwrap();
    let replacements = std::collections::HashMap::from([
        (
            fixture["seed"]["id"].as_str().unwrap().to_owned(),
            id.to_string(),
        ),
        (
            fixture["seed"]["viewer"].as_str().unwrap().to_owned(),
            viewer.address.to_string(),
        ),
        (
            fixture["seed"]["peer"].as_str().unwrap().to_owned(),
            peer.address.to_string(),
        ),
    ]);
    let mut phase = "";
    let mut friendship_id = None;
    let mut compared = 0;
    for case in fixture["cases"].as_array().unwrap() {
        let next_phase = case["phase"].as_str().unwrap();
        if next_phase != phase {
            if let Some(old_id) = friendship_id.take() {
                sqlx::query("DELETE FROM social.friendship_actions WHERE friendship_id=$1")
                    .bind(old_id)
                    .execute(&rig.pool)
                    .await
                    .unwrap();
                sqlx::query("DELETE FROM social.friendships WHERE id=$1")
                    .bind(old_id)
                    .execute(&rig.pool)
                    .await
                    .unwrap();
            }
            if next_phase != "none" {
                let fid = Uuid::new_v4();
                let action = if next_phase == "accepted" {
                    "accept"
                } else {
                    "request"
                };
                let actor = if next_phase == "request-sent" {
                    &viewer.address
                } else {
                    &peer.address
                };
                sqlx::query("INSERT INTO social.friendships(id,address_requester,address_requested,is_active,last_action,last_actor) VALUES($1,$2,$3,$4,$5,$6)")
                    .bind(fid).bind(&viewer.address).bind(&peer.address).bind(next_phase=="accepted").bind(action).bind(actor).execute(&rig.pool).await.unwrap();
                if next_phase != "empty-history" {
                    sqlx::query("INSERT INTO social.friendship_actions(id,friendship_id,action,acting_user,timestamp) VALUES($1,$2,$3,$4,'2024-01-02 00:00:00')")
                        .bind(Uuid::new_v4()).bind(fid).bind(action).bind(actor).execute(&rig.pool).await.unwrap();
                }
                friendship_id = Some(fid);
            }
            phase = next_phase;
        }
        let path = case["path"]
            .as_str()
            .unwrap()
            .replace(":id", &id.to_string());
        let mut request = Case::new("get", path);
        if !case["anonymous"].as_bool().unwrap() {
            request = request.signed(&viewer).await;
        }
        let (status, body) = hit(&rig.app, request).await;
        let mut expected = case["response"]["body"].clone();
        replace_seed(&mut expected, &replacements);
        assert_eq!(
            status.as_u16(),
            case["response"]["status"].as_u64().unwrap() as u16,
            "phase={phase} case={compared}: {body}"
        );
        assert_eq!(
            body, expected,
            "phase={phase} case={compared}, full response (only seeded UUID/address substitution)"
        );
        compared += 1;
    }
    assert_eq!(compared, 22);
    rig.finish();
}
