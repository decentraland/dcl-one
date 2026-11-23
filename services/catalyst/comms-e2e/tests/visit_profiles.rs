//! A visiting bot's profile against a stand-in content server and lambdas: the form the server
//! receives, the chain it checks, a profile already served, and a refusal that says when to come
//! back.

mod support;

use std::time::Duration;

use catalyst_comms_e2e::identity::{Identity, now_ms};
use catalyst_comms_e2e::profile::{
    self, Deployed, Ensured, PROFILE_VERSION, cid_v1_raw, profile_deployment,
};
use support::content;

fn visitor(index: u32) -> Identity {
    Identity::visitor("profile-tests", index)
}

#[tokio::test]
async fn the_realm_description_names_where_profiles_go() {
    // a relative content URL resolves against the description's own address
    let stand_in = content::start(None).await;
    let endpoints = profile::endpoints(&profile::client().unwrap(), &stand_in.api)
        .await
        .unwrap();
    assert_eq!(
        endpoints.entities.as_str(),
        format!("{}content/entities", stand_in.api)
    );
    assert_eq!(
        endpoints.lambdas.as_str(),
        format!("{}lambdas", stand_in.api)
    );
    // an absolute one, with its trailing slash, is taken as it is
    let elsewhere = content::start(Some("https://content.example/content/")).await;
    let endpoints = profile::endpoints(&profile::client().unwrap(), &elsewhere.api)
        .await
        .unwrap();
    assert_eq!(
        endpoints.entities.as_str(),
        "https://content.example/content/entities"
    );
    assert!(endpoints.hosts().contains(&"content.example".to_owned()));
}

#[tokio::test]
async fn the_content_server_receives_the_id_the_chain_and_the_file_named_by_the_cid() {
    let stand_in = content::start(None).await;
    let client = profile::client().unwrap();
    let endpoints = profile::endpoints(&client, &stand_in.api).await.unwrap();
    let identity = visitor(4);
    let deployment = profile_deployment(&identity, 4, now_ms());
    assert_eq!(
        profile::deploy(&client, &endpoints.entities, &deployment)
            .await
            .unwrap(),
        Deployed::Accepted
    );

    let received = stand_in.recorded.lock().posts[0].clone();
    assert!(
        received
            .content_type
            .starts_with("multipart/form-data; boundary="),
        "{}",
        received.content_type
    );
    assert_eq!(received.fields["entityId"], deployment.entity_id);
    for (index, link) in deployment.auth_chain.iter().enumerate() {
        assert_eq!(
            received.fields[&format!("authChain[{index}][payload]")],
            link.payload
        );
        assert!(
            received
                .fields
                .contains_key(&format!("authChain[{index}][type]"))
        );
        assert!(
            received
                .fields
                .contains_key(&format!("authChain[{index}][signature]"))
        );
    }
    assert_eq!(received.fields["authChain[0][type]"], "SIGNER");
    assert_eq!(received.fields["authChain[1][type]"], "ECDSA_EPHEMERAL");
    assert_eq!(received.fields["authChain[2][type]"], "ECDSA_SIGNED_ENTITY");
    // the chain's last link signs the entity id
    assert_eq!(
        received.fields["authChain[2][payload]"],
        deployment.entity_id
    );
    // one file: the entity, named by its CIDv1, the exact bytes the id names
    assert_eq!(received.files.len(), 1);
    let file = &received.files[0];
    assert_eq!(file.name, deployment.entity_id);
    assert_eq!(file.file_name, None);
    assert_eq!(
        file.content_type.as_deref(),
        Some("application/octet-stream")
    );
    assert_eq!(file.bytes, deployment.entity);
    assert_eq!(cid_v1_raw(&file.bytes), deployment.entity_id);
    assert_eq!(dcl_auth::hash_bytes_v1(&file.bytes), deployment.entity_id);
    // and the lambdas serve it
    assert_eq!(
        profile::served_version(&client, &endpoints.lambdas, &identity.address())
            .await
            .unwrap(),
        Some(i64::from(PROFILE_VERSION))
    );
    assert_eq!(
        stand_in.served_name(&identity.address()).as_deref(),
        Some("ce2e-visit-04")
    );
}

#[tokio::test]
async fn the_chain_verifies_with_the_servers_own_check_and_signs_the_entity_id() {
    let identity = visitor(1);
    let timestamp = now_ms();
    let deployment = profile_deployment(&identity, 1, timestamp);
    // what the content server runs on a deployment's chain: the entity id as the payload, the
    // delegation checked at the entity's timestamp
    dcl_auth::verify_auth_chain_async(
        &deployment.auth_chain,
        &deployment.entity_id,
        Some(timestamp),
        None,
    )
    .await
    .expect("the server's check accepts the chain");
    assert!(
        dcl_auth::verify_auth_chain_async(
            &deployment.auth_chain,
            &cid_v1_raw(b"another entity"),
            Some(timestamp),
            None,
        )
        .await
        .is_err(),
        "a chain for one id passes for another"
    );
    assert_eq!(deployment.auth_chain[0].payload, identity.address());
}

#[tokio::test]
async fn a_profile_already_served_is_not_deployed_again() {
    let stand_in = content::start(None).await;
    let client = profile::client().unwrap();
    let endpoints = profile::endpoints(&client, &stand_in.api).await.unwrap();
    let identity = visitor(2);
    stand_in.serve(&identity.address(), i64::from(PROFILE_VERSION));
    assert_eq!(
        profile::ensure(&client, &endpoints, &identity, 2)
            .await
            .unwrap(),
        Ensured::AlreadyServed
    );
    assert_eq!(stand_in.posts(), 0);

    // another version served is replaced, once
    let other = visitor(3);
    stand_in.serve(&other.address(), 0);
    assert_eq!(
        profile::ensure(&client, &endpoints, &other, 3)
            .await
            .unwrap(),
        Ensured::Deployed(Deployed::Accepted)
    );
    assert_eq!(
        profile::ensure(&client, &endpoints, &other, 3)
            .await
            .unwrap(),
        Ensured::AlreadyServed
    );
    assert_eq!(stand_in.posts(), 1);
}

#[tokio::test]
async fn a_refusal_with_retry_after_is_sent_again_once() {
    let stand_in = content::start(None).await;
    let client = profile::client().unwrap();
    let endpoints = profile::endpoints(&client, &stand_in.api).await.unwrap();

    // refused once for a second: sent again after it, and accepted
    stand_in.answer_next(429, Some("1"));
    let deployment = profile_deployment(&visitor(5), 5, now_ms());
    assert_eq!(
        profile::deploy(&client, &endpoints.entities, &deployment)
            .await
            .unwrap(),
        Deployed::AcceptedOnRetry
    );
    {
        let recorded = stand_in.recorded.lock();
        assert_eq!(recorded.posts.len(), 2);
        let waited = recorded.posts[1].at - recorded.posts[0].at;
        assert!(waited >= Duration::from_millis(900), "{waited:?}");
        assert_eq!(recorded.posts[1].fields, recorded.posts[0].fields);
    }

    // refused twice: the second refusal is the error, naming its status
    stand_in.answer_next(503, Some("1"));
    stand_in.answer_next(503, Some("1"));
    let error = profile::deploy(&client, &endpoints.entities, &deployment)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("503"), "{error}");
    assert!(error.contains("sent again"), "{error}");
    assert_eq!(stand_in.posts(), 4);

    // a refusal that does not say when to come back, or asks for longer than a minute, is not
    // sent again
    for retry_after in [None, Some("120")] {
        stand_in.answer_next(429, retry_after);
        let before = stand_in.posts();
        let error = profile::deploy(&client, &endpoints.entities, &deployment)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("429"), "{error}");
        assert_eq!(stand_in.posts(), before + 1, "{retry_after:?}");
    }

    // a refusal of the deployment itself is not sent again
    stand_in.answer_next(400, Some("1"));
    let before = stand_in.posts();
    let error = profile::deploy(&client, &endpoints.entities, &deployment)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("400"), "{error}");
    assert_eq!(stand_in.posts(), before + 1);
}
