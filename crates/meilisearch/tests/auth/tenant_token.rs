use std::collections::HashMap;

use ::time::format_description::well_known::Rfc3339;
use maplit::hashmap;
use once_cell::sync::Lazy;
use time::{Duration, OffsetDateTime};

use super::authorization::{ALL_ACTIONS, AUTHORIZATIONS};
use crate::common::{Server, Value, DOCUMENTS};
use crate::json;

fn generate_tenant_token(
    parent_uid: impl AsRef<str>,
    parent_key: impl AsRef<str>,
    mut body: HashMap<&str, Value>,
) -> String {
    use jsonwebtoken::{encode, EncodingKey, Header};

    let parent_uid = parent_uid.as_ref();
    body.insert("apiKeyUid", json!(parent_uid));
    encode(&Header::default(), &body, &EncodingKey::from_secret(parent_key.as_ref().as_bytes()))
        .unwrap()
}

static INVALID_RESPONSE: Lazy<Value> = Lazy::new(|| {
    json!({
        "message": null,
        "code": "invalid_api_key",
        "type": "auth",
        "link": "https://docs.meilisearch.com/errors#invalid_api_key"
    })
});

static ACCEPTED_KEYS: Lazy<Vec<Value>> = Lazy::new(|| {
    vec![
        json!({
            "indexes": ["*"],
            "actions": ["*"],
            "expiresAt": (OffsetDateTime::now_utc() + Duration::days(1)).format(&Rfc3339).unwrap()
        }),
        json!({
            "indexes": ["*"],
            "actions": ["search"],
            "expiresAt": (OffsetDateTime::now_utc() + Duration::days(1)).format(&Rfc3339).unwrap()
        }),
        json!({
            "indexes": ["sales"],
            "actions": ["*"],
            "expiresAt": (OffsetDateTime::now_utc() + Duration::days(1)).format(&Rfc3339).unwrap()
        }),
        json!({
            "indexes": ["sales"],
            "actions": ["search"],
            "expiresAt": (OffsetDateTime::now_utc() + Duration::days(1)).format(&Rfc3339).unwrap()
        }),
        json!({
            "indexes": ["sal*", "prod*"],
            "actions": ["search"],
            "expiresAt": (OffsetDateTime::now_utc() + Duration::days(1)).format(&Rfc3339).unwrap()
        }),
    ]
});

static REFUSED_KEYS: Lazy<Vec<Value>> = Lazy::new(|| {
    vec![
        // no search action
        json!({
            "indexes": ["*"],
            "actions": ALL_ACTIONS.iter().cloned().filter(|a| *a != "search" && *a != "*").collect::<Vec<_>>(),
            "expiresAt": (OffsetDateTime::now_utc() + Duration::days(1)).format(&Rfc3339).unwrap()
        }),
        json!({
            "indexes": ["sales"],
            "actions": ALL_ACTIONS.iter().cloned().filter(|a| *a != "search" && *a != "*").collect::<Vec<_>>(),
            "expiresAt": (OffsetDateTime::now_utc() + Duration::days(1)).format(&Rfc3339).unwrap()
        }),
        // bad index
        json!({
            "indexes": ["products"],
            "actions": ["*"],
            "expiresAt": (OffsetDateTime::now_utc() + Duration::days(1)).format(&Rfc3339).unwrap()
        }),
        json!({
            "indexes": ["prod*", "p*"],
            "actions": ["*"],
            "expiresAt": (OffsetDateTime::now_utc() + Duration::days(1)).format(&Rfc3339).unwrap()
        }),
        json!({
            "indexes": ["products"],
            "actions": ["search"],
            "expiresAt": (OffsetDateTime::now_utc() + Duration::days(1)).format(&Rfc3339).unwrap()
        }),
    ]
});

macro_rules! compute_authorized_search {
    ($tenant_tokens:expr, $filter:expr, $expected_count:expr) => {
        let mut server = Server::new_auth().await;
        server.use_admin_key("MASTER_KEY").await;
        let index = server.index("sales");
        let documents = DOCUMENTS.clone();
        let (task1,_status_code) = index.add_documents(documents, None).await;
        server.wait_task(task1.uid()).await.succeeded();
        let (task2,_status_code) = index
            .update_settings(json!({"filterableAttributes": ["color"]}))
            .await;
        server.wait_task(task2.uid()).await.succeeded();
        drop(index);

        for key_content in ACCEPTED_KEYS.iter() {
            server.use_api_key("MASTER_KEY");
            let (response, code) = server.add_api_key(key_content.clone()).await;
            assert_eq!(code, 201);
            let key = response["key"].as_str().unwrap();
            let uid = response["uid"].as_str().unwrap();

            for tenant_token in $tenant_tokens.iter() {
                let web_token = generate_tenant_token(&uid, &key, tenant_token.clone());
                server.use_api_key(&web_token);
                let index = server.index("sales");
                index
                    .search(json!({ "filter": $filter }), |response, code| {
                        assert_eq!(
                            code, 200,
                            "{} using tenant_token: {:?} generated with parent_key: {:?}",
                            response, tenant_token, key_content
                        );
                        assert_eq!(
                            response["hits"].as_array().unwrap().len(),
                            $expected_count,
                            "{} using tenant_token: {:?} generated with parent_key: {:?}",
                            response,
                            tenant_token,
                            key_content
                        );
                    })
                    .await;
            }
        }
    };
}

macro_rules! compute_forbidden_search {
    ($tenant_tokens:expr, $parent_keys:expr) => {
        let mut server = Server::new_auth().await;
        server.use_admin_key("MASTER_KEY").await;
        let index = server.index("sales");
        let documents = DOCUMENTS.clone();
        let (task, _status_code) = index.add_documents(documents, None).await;
        server.wait_task(task.uid()).await.succeeded();
        drop(index);

        for key_content in $parent_keys.iter() {
            server.use_api_key("MASTER_KEY");
            let (response, code) = server.add_api_key(key_content.clone()).await;
            assert_eq!(code, 201, "{:?}", response);
            let key = response["key"].as_str().unwrap();
            let uid = response["uid"].as_str().unwrap();

            for tenant_token in $tenant_tokens.iter() {
                let web_token = generate_tenant_token(&uid, &key, tenant_token.clone());
                server.use_api_key(&web_token);
                let index = server.index("sales");
                index
                    .search(json!({}), |mut response, code| {
                        // We don't assert anything on the message since it may change between cases
                        response["message"] = serde_json::json!(null);
                        assert_eq!(
                            response,
                            INVALID_RESPONSE.clone(),
                            "{} using tenant_token: {:?} generated with parent_key: {:?}",
                            response,
                            tenant_token,
                            key_content
                        );
                        assert_eq!(
                            code, 403,
                            "{} using tenant_token: {:?} generated with parent_key: {:?}",
                            response, tenant_token, key_content
                        );
                    })
                    .await;
            }
        }
    };
}

#[actix_rt::test]
async fn search_authorized_simple_token() {
    let tenant_tokens = [
        hashmap! {
            "searchRules" => json!({"*": {}}),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!(["*"]),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({"sales": {}}),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!(["sales"]),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({"*": {}}),
            "exp" => json!(null)
        },
        hashmap! {
            "searchRules" => json!({"*": null}),
            "exp" => json!(null)
        },
        hashmap! {
            "searchRules" => json!(["*"]),
            "exp" => json!(null)
        },
        hashmap! {
            "searchRules" => json!({"sales": {}}),
            "exp" => json!(null)
        },
        hashmap! {
            "searchRules" => json!({"sales": null}),
            "exp" => json!(null)
        },
        hashmap! {
            "searchRules" => json!(["sales"]),
            "exp" => json!(null)
        },
        hashmap! {
            "searchRules" => json!(["sa*"]),
            "exp" => json!(null)
        },
    ];

    compute_authorized_search!(tenant_tokens, {}, 5);
}

#[actix_rt::test]
async fn search_authorized_filter_token() {
    let tenant_tokens = [
        hashmap! {
            "searchRules" => json!({"*": {"filter": "color = blue"}}),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({"sales": {"filter": "color = blue"}}),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({"*": {"filter": ["color = blue"]}}),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({"sales": {"filter": ["color = blue"]}}),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        // filter on sales should override filters on *
        hashmap! {
            "searchRules" => json!({
                "*": {"filter": "color = green"},
                "sales": {"filter": "color = blue"}
            }),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({
                "*": {},
                "sales": {"filter": "color = blue"}
            }),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({
                "*": {"filter": "color = green"},
                "sales": {"filter": ["color = blue"]}
            }),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({
                "*": {},
                "sales": {"filter": ["color = blue"]}
            }),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
    ];

    compute_authorized_search!(tenant_tokens, {}, 3);
}

#[actix_rt::test]
async fn filter_search_authorized_filter_token() {
    let tenant_tokens = [
        hashmap! {
            "searchRules" => json!({"*": {"filter": "color = blue"}}),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({"sales": {"filter": "color = blue"}}),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({"*": {"filter": ["color = blue"]}}),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({"sales": {"filter": ["color = blue"]}}),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        // filter on sales should override filters on *
        hashmap! {
            "searchRules" => json!({
                "*": {"filter": "color = green"},
                "sales": {"filter": "color = blue"}
            }),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({
                "*": {},
                "sales": {"filter": "color = blue"}
            }),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({
                "*": {"filter": "color = green"},
                "sales": {"filter": ["color = blue"]}
            }),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({
                "*": {},
                "sales": {"filter": ["color = blue"]}
            }),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({
                "*": {},
                "sal*": {"filter": ["color = blue"]}
            }),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
    ];

    compute_authorized_search!(tenant_tokens, "color = yellow", 1);
}

/// Tests that those Tenant Token are incompatible with the REFUSED_KEYS defined above.
#[actix_rt::test]
async fn error_search_token_forbidden_parent_key() {
    let tenant_tokens = [
        hashmap! {
            "searchRules" => json!({"*": {}}),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({"*": null}),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!(["*"]),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({"sales": {}}),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({"sales": null}),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!(["sales"]),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!(["sali*", "s*", "sales*"]),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
    ];

    compute_forbidden_search!(tenant_tokens, REFUSED_KEYS);
}

#[actix_rt::test]
async fn error_search_forbidden_token() {
    let tenant_tokens = [
        // bad index
        hashmap! {
            "searchRules" => json!({"products": {}}),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!(["products"]),
            "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({"products": {}}),
            "exp" => json!(null)
        },
        hashmap! {
            "searchRules" => json!({"products": null}),
            "exp" => json!(null)
        },
        hashmap! {
            "searchRules" => json!(["products"]),
            "exp" => json!(null)
        },
        // expired token
        hashmap! {
            "searchRules" => json!({"*": {}}),
            "exp" => json!((OffsetDateTime::now_utc() - Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({"*": null}),
            "exp" => json!((OffsetDateTime::now_utc() - Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!(["*"]),
            "exp" => json!((OffsetDateTime::now_utc() - Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({"sales": {}}),
            "exp" => json!((OffsetDateTime::now_utc() - Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!({"sales": null}),
            "exp" => json!((OffsetDateTime::now_utc() - Duration::hours(1)).unix_timestamp())
        },
        hashmap! {
            "searchRules" => json!(["sales"]),
            "exp" => json!((OffsetDateTime::now_utc() - Duration::hours(1)).unix_timestamp())
        },
    ];

    compute_forbidden_search!(tenant_tokens, ACCEPTED_KEYS);
}

#[actix_rt::test]
async fn error_access_forbidden_routes() {
    let mut server = Server::new_auth().await;
    server.use_api_key("MASTER_KEY");

    let content = json!({
        "indexes": ["*"],
        "actions": ["*"],
        "expiresAt": (OffsetDateTime::now_utc() + Duration::hours(1)).format(&Rfc3339).unwrap(),
    });

    let (response, code) = server.add_api_key(content).await;
    assert_eq!(code, 201);
    assert!(response["key"].is_string());

    let key = response["key"].as_str().unwrap();
    let uid = response["uid"].as_str().unwrap();

    let tenant_token = hashmap! {
        "searchRules" => json!(["*"]),
        "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
    };
    let web_token = generate_tenant_token(uid, key, tenant_token);
    server.use_api_key(&web_token);

    for ((method, route), actions) in AUTHORIZATIONS.iter() {
        // Tenant tokens (which only carry searchRules) are now also accepted on document browse
        // routes (documents.get) since the DOCUMENTS_GET action is in the tenant token gate.
        // Routes requiring documents.get are no longer forbidden for tenant tokens.
        if !actions.contains("search") && !actions.contains("documents.get") {
            let (mut response, code) = server.dummy_request(method, route).await;
            response["message"] = serde_json::json!(null);
            assert_eq!(response, INVALID_RESPONSE.clone());
            assert_eq!(code, 403);
        }
    }
}

#[actix_rt::test]
async fn error_access_expired_parent_key() {
    use std::{thread, time};
    let mut server = Server::new_auth().await;
    server.use_api_key("MASTER_KEY");

    let content = json!({
        "indexes": ["*"],
        "actions": ["*"],
        "expiresAt": (OffsetDateTime::now_utc() + Duration::seconds(1)).format(&Rfc3339).unwrap(),
    });

    let (response, code) = server.add_api_key(content).await;
    assert_eq!(code, 201);
    assert!(response["key"].is_string());

    let key = response["key"].as_str().unwrap();
    let uid = response["uid"].as_str().unwrap();

    let tenant_token = hashmap! {
        "searchRules" => json!(["*"]),
        "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
    };
    let web_token = generate_tenant_token(uid, key, tenant_token);
    server.use_api_key(&web_token);

    // test search request while parent_key is not expired
    let (mut response, code) = server.dummy_request("POST", "/indexes/products/search").await;
    response["message"] = serde_json::json!(null);
    assert_ne!(response, INVALID_RESPONSE.clone());
    assert_ne!(code, 403);

    // wait until the key is expired.
    thread::sleep(time::Duration::new(1, 0));

    let (mut response, code) = server.dummy_request("POST", "/indexes/products/search").await;
    response["message"] = serde_json::json!(null);
    assert_eq!(response, INVALID_RESPONSE.clone());
    assert_eq!(code, 403);
}

#[actix_rt::test]
async fn error_access_modified_token() {
    let mut server = Server::new_auth().await;
    server.use_api_key("MASTER_KEY");

    let content = json!({
        "indexes": ["*"],
        "actions": ["*"],
        "expiresAt": (OffsetDateTime::now_utc() + Duration::hours(1)).format(&Rfc3339).unwrap(),
    });

    let (response, code) = server.add_api_key(content).await;
    assert_eq!(code, 201);
    assert!(response["key"].is_string());

    let key = response["key"].as_str().unwrap();
    let uid = response["uid"].as_str().unwrap();

    let tenant_token = hashmap! {
        "searchRules" => json!(["products"]),
        "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
    };
    let web_token = generate_tenant_token(uid, key, tenant_token);
    server.use_api_key(&web_token);

    // test search request while web_token is valid
    let (response, code) = server.dummy_request("POST", "/indexes/products/search").await;
    assert_ne!(response, INVALID_RESPONSE.clone());
    assert_ne!(code, 403);

    let tenant_token = hashmap! {
        "searchRules" => json!(["*"]),
        "exp" => json!((OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp())
    };

    let alt = generate_tenant_token(uid, key, tenant_token);
    let altered_token = [
        web_token.split('.').next().unwrap(),
        alt.split('.').nth(1).unwrap(),
        web_token.split('.').nth(2).unwrap(),
    ]
    .join(".");

    server.use_api_key(&altered_token);
    let (mut response, code) = server.dummy_request("POST", "/indexes/products/search").await;
    response["message"] = serde_json::json!(null);
    assert_eq!(response, INVALID_RESPONSE.clone());
    assert_eq!(code, 403);
}

// ---- indexRules integration tests ----
// These tests verify AUTH-01, AUTH-02, AUTH-03, and the DOCUMENTS_GET action gate.

/// AUTH-01: A JWT carrying `indexRules` (Map format with filter) is decoded correctly
/// and the request to GET /indexes/{uid}/documents is accepted (HTTP 200).
#[actix_rt::test]
async fn index_rules_claim_decoded() {
    let mut server = Server::new_auth().await;
    server.use_admin_key("MASTER_KEY").await;

    // Create index with a document and make tenant_id filterable.
    // Explicit primary key avoids ambiguity between "id" and "tenant_id" during inference.
    let index = server.index("sales");
    let (task, _) = index.add_documents(crate::json!([{"id": 1, "tenant_id": "a"}]), Some("id")).await;
    server.wait_task(task.uid()).await.succeeded();
    let (task, _) = index.update_settings(crate::json!({"filterableAttributes": ["tenant_id"]})).await;
    server.wait_task(task.uid()).await.succeeded();
    drop(index);

    // Create a parent API key with full access
    server.use_api_key("MASTER_KEY");
    let key_body = crate::json!({
        "indexes": ["*"],
        "actions": ["*"],
        "expiresAt": (time::OffsetDateTime::now_utc() + time::Duration::days(1)).format(&::time::format_description::well_known::Rfc3339).unwrap()
    });
    let (response, code) = server.add_api_key(key_body).await;
    assert_eq!(code, 201, "{:?}", response);
    let key = response["key"].as_str().unwrap();
    let uid = response["uid"].as_str().unwrap();

    // Generate tenant token with both searchRules and indexRules (Map format with filter)
    let token = generate_tenant_token(uid, key, hashmap! {
        "searchRules" => crate::json!({"*": {}}),
        "indexRules" => crate::json!({"sales": {"filter": "tenant_id = a"}}),
        "exp" => crate::json!((time::OffsetDateTime::now_utc() + time::Duration::hours(1)).unix_timestamp())
    });
    server.use_api_key(&token);

    let (response, code) = server.dummy_request("GET", "/indexes/sales/documents").await;
    assert_eq!(code, 200, "Expected 200 on GET /indexes/sales/documents with indexRules, got {}: {:?}", code, response);
}

/// AUTH-02: `searchRules` and `indexRules` are fully independent.
/// A JWT with both claims uses `searchRules` for search and `indexRules` for document browse.
/// Setting `indexRules` must NOT affect search behavior.
#[actix_rt::test]
async fn index_rules_independent_from_search_rules() {
    let mut server = Server::new_auth().await;
    server.use_admin_key("MASTER_KEY").await;

    // Create index with documents.
    // Explicit primary key avoids ambiguity between "id" and "tenant_id" during inference.
    let index = server.index("sales");
    let (task, _) = index.add_documents(crate::json!([
        {"id": 1, "tenant_id": "a"},
        {"id": 2, "tenant_id": "b"}
    ]), Some("id")).await;
    server.wait_task(task.uid()).await.succeeded();
    let (task, _) = index.update_settings(crate::json!({"filterableAttributes": ["tenant_id"]})).await;
    server.wait_task(task.uid()).await.succeeded();
    drop(index);

    // Create parent key
    server.use_api_key("MASTER_KEY");
    let key_body = crate::json!({
        "indexes": ["*"],
        "actions": ["*"],
        "expiresAt": (time::OffsetDateTime::now_utc() + time::Duration::days(1)).format(&::time::format_description::well_known::Rfc3339).unwrap()
    });
    let (response, code) = server.add_api_key(key_body).await;
    assert_eq!(code, 201, "{:?}", response);
    let key = response["key"].as_str().unwrap();
    let uid = response["uid"].as_str().unwrap();

    // searchRules filters tenant_id = b, indexRules filters tenant_id = a
    let token = generate_tenant_token(uid, key, hashmap! {
        "searchRules" => crate::json!({"sales": {"filter": "tenant_id = b"}}),
        "indexRules" => crate::json!({"sales": {"filter": "tenant_id = a"}}),
        "exp" => crate::json!((time::OffsetDateTime::now_utc() + time::Duration::hours(1)).unix_timestamp())
    });
    server.use_api_key(&token);

    // Search should be accepted (searchRules covers the sales index)
    let (response, code) = server.dummy_request("POST", "/indexes/sales/search").await;
    assert_eq!(code, 200, "Expected search to be accepted with searchRules covering sales, got {}: {:?}", code, response);

    // Document browse should be accepted (indexRules covers the sales index)
    let (response, code) = server.dummy_request("GET", "/indexes/sales/documents").await;
    assert_eq!(code, 200, "Expected document browse to be accepted with indexRules covering sales, got {}: {:?}", code, response);
}

/// AUTH-03 (Set format): `"indexRules": ["sales"]` (array/Set) grants access to
/// GET /indexes/sales/documents.
#[actix_rt::test]
async fn index_rules_set_format() {
    let mut server = Server::new_auth().await;
    server.use_admin_key("MASTER_KEY").await;

    let index = server.index("sales");
    let (task, _) = index.add_documents(crate::json!([{"id": 1}]), None).await;
    server.wait_task(task.uid()).await.succeeded();
    drop(index);

    server.use_api_key("MASTER_KEY");
    let key_body = crate::json!({
        "indexes": ["*"],
        "actions": ["*"],
        "expiresAt": (time::OffsetDateTime::now_utc() + time::Duration::days(1)).format(&::time::format_description::well_known::Rfc3339).unwrap()
    });
    let (response, code) = server.add_api_key(key_body).await;
    assert_eq!(code, 201, "{:?}", response);
    let key = response["key"].as_str().unwrap();
    let uid = response["uid"].as_str().unwrap();

    // Set format: indexRules as an array
    let token = generate_tenant_token(uid, key, hashmap! {
        "searchRules" => crate::json!({"*": {}}),
        "indexRules" => crate::json!(["sales"]),
        "exp" => crate::json!((time::OffsetDateTime::now_utc() + time::Duration::hours(1)).unix_timestamp())
    });
    server.use_api_key(&token);

    let (response, code) = server.dummy_request("GET", "/indexes/sales/documents").await;
    assert_eq!(code, 200, "Expected 200 for indexRules Set format [\"sales\"], got {}: {:?}", code, response);
}

/// AUTH-03 (Map format): `"indexRules": {"sales": {"filter": "..."}}` grants access to
/// GET /indexes/sales/documents.
#[actix_rt::test]
async fn index_rules_map_format() {
    let mut server = Server::new_auth().await;
    server.use_admin_key("MASTER_KEY").await;

    let index = server.index("sales");
    // Explicit primary key avoids ambiguity between "id" and "tenant_id" during inference.
    let (task, _) = index.add_documents(crate::json!([{"id": 1, "tenant_id": "a"}]), Some("id")).await;
    server.wait_task(task.uid()).await.succeeded();
    let (task, _) = index.update_settings(crate::json!({"filterableAttributes": ["tenant_id"]})).await;
    server.wait_task(task.uid()).await.succeeded();
    drop(index);

    server.use_api_key("MASTER_KEY");
    let key_body = crate::json!({
        "indexes": ["*"],
        "actions": ["*"],
        "expiresAt": (time::OffsetDateTime::now_utc() + time::Duration::days(1)).format(&::time::format_description::well_known::Rfc3339).unwrap()
    });
    let (response, code) = server.add_api_key(key_body).await;
    assert_eq!(code, 201, "{:?}", response);
    let key = response["key"].as_str().unwrap();
    let uid = response["uid"].as_str().unwrap();

    // Map format: indexRules as an object with per-index filter
    let token = generate_tenant_token(uid, key, hashmap! {
        "searchRules" => crate::json!({"*": {}}),
        "indexRules" => crate::json!({"sales": {"filter": "tenant_id = a"}}),
        "exp" => crate::json!((time::OffsetDateTime::now_utc() + time::Duration::hours(1)).unix_timestamp())
    });
    server.use_api_key(&token);

    let (response, code) = server.dummy_request("GET", "/indexes/sales/documents").await;
    assert_eq!(code, 200, "Expected 200 for indexRules Map format with filter, got {}: {:?}", code, response);
}

/// Gate test: A tenant JWT with `indexRules: {"sales": null}` can reach
/// GET /indexes/sales/documents (DOCUMENTS_GET action accepted).
#[actix_rt::test]
async fn documents_get_tenant_token_gate() {
    let mut server = Server::new_auth().await;
    server.use_admin_key("MASTER_KEY").await;

    let index = server.index("sales");
    let (task, _) = index.add_documents(crate::json!([{"id": 1}]), None).await;
    server.wait_task(task.uid()).await.succeeded();
    drop(index);

    server.use_api_key("MASTER_KEY");
    let key_body = crate::json!({
        "indexes": ["*"],
        "actions": ["*"],
        "expiresAt": (time::OffsetDateTime::now_utc() + time::Duration::days(1)).format(&::time::format_description::well_known::Rfc3339).unwrap()
    });
    let (response, code) = server.add_api_key(key_body).await;
    assert_eq!(code, 201, "{:?}", response);
    let key = response["key"].as_str().unwrap();
    let uid = response["uid"].as_str().unwrap();

    // indexRules with null value (no filter, just whitelist access)
    let token = generate_tenant_token(uid, key, hashmap! {
        "searchRules" => crate::json!({"*": {}}),
        "indexRules" => crate::json!({"sales": null}),
        "exp" => crate::json!((time::OffsetDateTime::now_utc() + time::Duration::hours(1)).unix_timestamp())
    });
    server.use_api_key(&token);

    let (response, code) = server.dummy_request("GET", "/indexes/sales/documents").await;
    assert_eq!(code, 200, "DOCUMENTS_GET action gate: expected 200 for tenant token with indexRules, got {}: {:?}", code, response);
}

// =============================================================================
// Phase 2 — Route injection test stubs (Wave 0 / TDD RED phase)
// Plans 02-01 and 02-02 will implement the route-layer logic that makes these pass.
// =============================================================================

/// DOCS-01: GET /indexes/{index}/documents with `indexRules` filter must return
/// only documents matching the filter (tenant_id = "a" → 2 docs, not 3).
#[actix_rt::test]
async fn index_rules_list_filtered() {
    let mut server = Server::new_auth().await;
    server.use_admin_key("MASTER_KEY").await;

    let index = server.index("sales");
    let (task, _) = index.add_documents(crate::json!([
        {"id": 1, "tenant_id": "a"},
        {"id": 2, "tenant_id": "a"},
        {"id": 3, "tenant_id": "b"}
    ]), Some("id")).await;
    server.wait_task(task.uid()).await.succeeded();
    let (task, _) = index.update_settings(crate::json!({"filterableAttributes": ["tenant_id"]})).await;
    server.wait_task(task.uid()).await.succeeded();
    drop(index);

    server.use_api_key("MASTER_KEY");
    let key_body = crate::json!({
        "indexes": ["*"],
        "actions": ["*"],
        "expiresAt": (time::OffsetDateTime::now_utc() + time::Duration::days(1))
            .format(&::time::format_description::well_known::Rfc3339).unwrap()
    });
    let (response, code) = server.add_api_key(key_body).await;
    assert_eq!(code, 201, "{:?}", response);
    let key = response["key"].as_str().unwrap();
    let uid = response["uid"].as_str().unwrap();

    let token = generate_tenant_token(uid, key, hashmap! {
        "searchRules" => crate::json!({"*": {}}),
        "indexRules" => crate::json!({"sales": {"filter": "tenant_id = a"}}),
        "exp" => crate::json!((time::OffsetDateTime::now_utc() + time::Duration::hours(1)).unix_timestamp())
    });
    server.use_api_key(&token);

    let (response, code) = server.dummy_request("GET", "/indexes/sales/documents").await;
    assert_eq!(code, 200, "DOCS-01: expected 200, got {}: {:?}", code, response);
    let results = response["results"].as_array().expect("DOCS-01: expected results array");
    assert_eq!(results.len(), 2, "DOCS-01: expected 2 documents (tenant_id = a), got {}: {:?}", results.len(), response);
}

/// DOCS-02: POST /indexes/{index}/documents/fetch with `indexRules` filter must return
/// only documents matching the filter (tenant_id = "a" → 2 docs, not 3).
#[actix_rt::test]
async fn index_rules_fetch_filtered() {
    let mut server = Server::new_auth().await;
    server.use_admin_key("MASTER_KEY").await;

    let index = server.index("sales");
    let (task, _) = index.add_documents(crate::json!([
        {"id": 1, "tenant_id": "a"},
        {"id": 2, "tenant_id": "a"},
        {"id": 3, "tenant_id": "b"}
    ]), Some("id")).await;
    server.wait_task(task.uid()).await.succeeded();
    let (task, _) = index.update_settings(crate::json!({"filterableAttributes": ["tenant_id"]})).await;
    server.wait_task(task.uid()).await.succeeded();
    drop(index);

    server.use_api_key("MASTER_KEY");
    let key_body = crate::json!({
        "indexes": ["*"],
        "actions": ["*"],
        "expiresAt": (time::OffsetDateTime::now_utc() + time::Duration::days(1))
            .format(&::time::format_description::well_known::Rfc3339).unwrap()
    });
    let (response, code) = server.add_api_key(key_body).await;
    assert_eq!(code, 201, "{:?}", response);
    let key = response["key"].as_str().unwrap();
    let uid = response["uid"].as_str().unwrap();

    let token = generate_tenant_token(uid, key, hashmap! {
        "searchRules" => crate::json!({"*": {}}),
        "indexRules" => crate::json!({"sales": {"filter": "tenant_id = a"}}),
        "exp" => crate::json!((time::OffsetDateTime::now_utc() + time::Duration::hours(1)).unix_timestamp())
    });
    server.use_api_key(&token);

    let (response, code) = server.dummy_request("POST", "/indexes/sales/documents/fetch").await;
    assert_eq!(code, 200, "DOCS-02: expected 200, got {}: {:?}", code, response);
    let results = response["results"].as_array().expect("DOCS-02: expected results array");
    assert_eq!(results.len(), 2, "DOCS-02: expected 2 documents (tenant_id = a), got {}: {:?}", results.len(), response);
}

/// DOCS-03: GET /indexes/{index}/documents/{id} for a document outside the tenant's
/// scope must return 404 (not 403, to avoid confirming document existence).
#[actix_rt::test]
async fn index_rules_single_doc_out_of_scope() {
    let mut server = Server::new_auth().await;
    server.use_admin_key("MASTER_KEY").await;

    let index = server.index("sales");
    let (task, _) = index.add_documents(crate::json!([
        {"id": 1, "tenant_id": "a"},
        {"id": 2, "tenant_id": "a"},
        {"id": 3, "tenant_id": "b"}
    ]), Some("id")).await;
    server.wait_task(task.uid()).await.succeeded();
    let (task, _) = index.update_settings(crate::json!({"filterableAttributes": ["tenant_id"]})).await;
    server.wait_task(task.uid()).await.succeeded();
    drop(index);

    server.use_api_key("MASTER_KEY");
    let key_body = crate::json!({
        "indexes": ["*"],
        "actions": ["*"],
        "expiresAt": (time::OffsetDateTime::now_utc() + time::Duration::days(1))
            .format(&::time::format_description::well_known::Rfc3339).unwrap()
    });
    let (response, code) = server.add_api_key(key_body).await;
    assert_eq!(code, 201, "{:?}", response);
    let key = response["key"].as_str().unwrap();
    let uid = response["uid"].as_str().unwrap();

    let token = generate_tenant_token(uid, key, hashmap! {
        "searchRules" => crate::json!({"*": {}}),
        "indexRules" => crate::json!({"sales": {"filter": "tenant_id = a"}}),
        "exp" => crate::json!((time::OffsetDateTime::now_utc() + time::Duration::hours(1)).unix_timestamp())
    });
    server.use_api_key(&token);

    // Document 3 has tenant_id = "b" — out of tenant "a" scope.
    // Must return 404 (not 403) to avoid leaking existence information.
    let (response, code) = server.dummy_request("GET", "/indexes/sales/documents/3").await;
    assert_eq!(code, 404, "DOCS-03: expected 404 for out-of-scope document, got {}: {:?}", code, response);
}

/// DOCS-04: A tenant JWT with `searchRules` but NO `indexRules` claim must be
/// rejected with 403 when accessing GET /indexes/{index}/documents (fail-closed).
#[actix_rt::test]
async fn index_rules_fail_closed() {
    let mut server = Server::new_auth().await;
    server.use_admin_key("MASTER_KEY").await;

    let index = server.index("sales");
    let (task, _) = index.add_documents(crate::json!([
        {"id": 1, "tenant_id": "a"},
        {"id": 2, "tenant_id": "a"},
        {"id": 3, "tenant_id": "b"}
    ]), Some("id")).await;
    server.wait_task(task.uid()).await.succeeded();
    let (task, _) = index.update_settings(crate::json!({"filterableAttributes": ["tenant_id"]})).await;
    server.wait_task(task.uid()).await.succeeded();
    drop(index);

    server.use_api_key("MASTER_KEY");
    let key_body = crate::json!({
        "indexes": ["*"],
        "actions": ["*"],
        "expiresAt": (time::OffsetDateTime::now_utc() + time::Duration::days(1))
            .format(&::time::format_description::well_known::Rfc3339).unwrap()
    });
    let (response, code) = server.add_api_key(key_body).await;
    assert_eq!(code, 201, "{:?}", response);
    let key = response["key"].as_str().unwrap();
    let uid = response["uid"].as_str().unwrap();

    // Token has searchRules only — NO indexRules claim.
    let token = generate_tenant_token(uid, key, hashmap! {
        "searchRules" => crate::json!({"*": {}}),
        "exp" => crate::json!((time::OffsetDateTime::now_utc() + time::Duration::hours(1)).unix_timestamp())
    });
    server.use_api_key(&token);

    let (response, code) = server.dummy_request("GET", "/indexes/sales/documents").await;
    assert_eq!(code, 403, "DOCS-04: expected 403 (fail-closed) for JWT without indexRules, got {}: {:?}", code, response);
}

/// DOCS-05: A regular admin API key (not a tenant JWT) must bypass `indexRules`
/// filtering and return all documents unfiltered.
#[actix_rt::test]
async fn index_rules_admin_key_unaffected() {
    let mut server = Server::new_auth().await;
    server.use_admin_key("MASTER_KEY").await;

    let index = server.index("sales");
    let (task, _) = index.add_documents(crate::json!([
        {"id": 1, "tenant_id": "a"},
        {"id": 2, "tenant_id": "a"},
        {"id": 3, "tenant_id": "b"}
    ]), Some("id")).await;
    server.wait_task(task.uid()).await.succeeded();
    let (task, _) = index.update_settings(crate::json!({"filterableAttributes": ["tenant_id"]})).await;
    server.wait_task(task.uid()).await.succeeded();
    drop(index);

    server.use_api_key("MASTER_KEY");
    let key_body = crate::json!({
        "indexes": ["*"],
        "actions": ["*"],
        "expiresAt": (time::OffsetDateTime::now_utc() + time::Duration::days(1))
            .format(&::time::format_description::well_known::Rfc3339).unwrap()
    });
    let (response, code) = server.add_api_key(key_body).await;
    assert_eq!(code, 201, "{:?}", response);
    let key = response["key"].as_str().unwrap();

    // Use the API key directly — NOT a tenant token.
    server.use_api_key(key);

    let (response, code) = server.dummy_request("GET", "/indexes/sales/documents").await;
    assert_eq!(code, 200, "DOCS-05: expected 200 for admin key, got {}: {:?}", code, response);
    let results = response["results"].as_array().expect("DOCS-05: expected results array");
    assert_eq!(results.len(), 3, "DOCS-05: expected all 3 documents for admin key, got {}: {:?}", results.len(), response);
}

/// Phase 3 — Criterion 5: `searchRules` filter behavior is unchanged when `indexRules`
/// is also present. Search results must be scoped by `searchRules`, NOT by `indexRules`.
/// With searchRules = tenant_id = b (1 doc) and indexRules = tenant_id = a (2 docs),
/// a search must return exactly 1 hit (tenant_id = b), not 2 (cross-contamination).
#[actix_rt::test]
async fn search_rules_filter_unaffected_by_index_rules() {
    let mut server = Server::new_auth().await;
    server.use_admin_key("MASTER_KEY").await;

    let index = server.index("sales");
    let (task, _) = index.add_documents(crate::json!([
        {"id": 1, "tenant_id": "a"},
        {"id": 2, "tenant_id": "a"},
        {"id": 3, "tenant_id": "b"}
    ]), Some("id")).await;
    server.wait_task(task.uid()).await.succeeded();
    let (task, _) = index.update_settings(crate::json!({"filterableAttributes": ["tenant_id"]})).await;
    server.wait_task(task.uid()).await.succeeded();
    drop(index);

    server.use_api_key("MASTER_KEY");
    let key_body = crate::json!({
        "indexes": ["*"],
        "actions": ["*"],
        "expiresAt": (time::OffsetDateTime::now_utc() + time::Duration::days(1))
            .format(&::time::format_description::well_known::Rfc3339).unwrap()
    });
    let (response, code) = server.add_api_key(key_body).await;
    assert_eq!(code, 201, "{:?}", response);
    let key = response["key"].as_str().unwrap();
    let uid = response["uid"].as_str().unwrap();

    // searchRules scoped to tenant_id = b (1 document)
    // indexRules scoped to tenant_id = a (2 documents)
    // Search MUST return tenant_id = b results (1 hit), NOT tenant_id = a (cross-contamination).
    let token = generate_tenant_token(uid, key, hashmap! {
        "searchRules" => crate::json!({"sales": {"filter": "tenant_id = b"}}),
        "indexRules" => crate::json!({"sales": {"filter": "tenant_id = a"}}),
        "exp" => crate::json!((time::OffsetDateTime::now_utc() + time::Duration::hours(1)).unix_timestamp())
    });
    server.use_api_key(&token);

    let index = server.index("sales");
    index
        .search(crate::json!({}), |response, code| {
            assert_eq!(
                code, 200,
                "Phase3-Crit5: search must return 200 with searchRules scoped to tenant_id = b, got {}: {:?}",
                code, response
            );
            assert_eq!(
                response["hits"].as_array().unwrap().len(),
                1,
                "Phase3-Crit5: search must return 1 hit (tenant_id = b only), not 2 (indexRules cross-contamination): {:?}",
                response
            );
        })
        .await;
}

/// Phase 3 — Edge case: `fuse_filters()` applies both `indexRules` filter AND the
/// caller-supplied filter. Only the intersection is returned.
/// indexRules = tenant_id = a (allows docs 1, 2) AND caller filter = id > 1 → doc 2 only.
#[actix_rt::test]
async fn index_rules_filter_fused_with_query_filter() {
    let mut server = Server::new_auth().await;
    server.use_admin_key("MASTER_KEY").await;

    let index = server.index("sales");
    let (task, _) = index.add_documents(crate::json!([
        {"id": 1, "tenant_id": "a"},
        {"id": 2, "tenant_id": "a"},
        {"id": 3, "tenant_id": "b"}
    ]), Some("id")).await;
    server.wait_task(task.uid()).await.succeeded();
    // Need both "tenant_id" and "id" filterable: indexRules uses tenant_id, caller uses id.
    let (task, _) = index.update_settings(crate::json!({"filterableAttributes": ["tenant_id", "id"]})).await;
    server.wait_task(task.uid()).await.succeeded();
    drop(index);

    server.use_api_key("MASTER_KEY");
    let key_body = crate::json!({
        "indexes": ["*"],
        "actions": ["*"],
        "expiresAt": (time::OffsetDateTime::now_utc() + time::Duration::days(1))
            .format(&::time::format_description::well_known::Rfc3339).unwrap()
    });
    let (response, code) = server.add_api_key(key_body).await;
    assert_eq!(code, 201, "{:?}", response);
    let key = response["key"].as_str().unwrap();
    let uid = response["uid"].as_str().unwrap();

    // indexRules: tenant_id = a → allows doc 1 (id=1) and doc 2 (id=2)
    // Caller filter: id > 1 → from the allowed set, only doc 2 passes both filters
    let token = generate_tenant_token(uid, key, hashmap! {
        "searchRules" => crate::json!({"*": {}}),
        "indexRules" => crate::json!({"sales": {"filter": "tenant_id = a"}}),
        "exp" => crate::json!((time::OffsetDateTime::now_utc() + time::Duration::hours(1)).unix_timestamp())
    });
    server.use_api_key(&token);

    let index = server.index("sales");
    let (response, code) = index.fetch_documents(crate::json!({"filter": "id > 1"})).await;
    assert_eq!(code, 200, "Phase3-FusedFilter: expected 200, got {}: {:?}", code, response);
    let results = response["results"].as_array().expect("Phase3-FusedFilter: expected results array");
    assert_eq!(
        results.len(), 1,
        "Phase3-FusedFilter: expected 1 result (doc 2 only — intersection of indexRules and caller filter), got {}: {:?}",
        results.len(), response
    );
    assert_eq!(
        results[0]["id"], 2,
        "Phase3-FusedFilter: expected doc id=2, got {:?}",
        results[0]
    );
}

/// Phase 3 — Edge case: `indexRules` with a null rule (whitelisted index, no filter)
/// must return all documents. A null rule means "allow all" — not 0 results, not 403.
#[actix_rt::test]
async fn index_rules_null_rule_returns_all_documents() {
    let mut server = Server::new_auth().await;
    server.use_admin_key("MASTER_KEY").await;

    let index = server.index("sales");
    let (task, _) = index.add_documents(crate::json!([
        {"id": 1, "tenant_id": "a"},
        {"id": 2, "tenant_id": "a"},
        {"id": 3, "tenant_id": "b"}
    ]), Some("id")).await;
    server.wait_task(task.uid()).await.succeeded();
    let (task, _) = index.update_settings(crate::json!({"filterableAttributes": ["tenant_id"]})).await;
    server.wait_task(task.uid()).await.succeeded();
    drop(index);

    server.use_api_key("MASTER_KEY");
    let key_body = crate::json!({
        "indexes": ["*"],
        "actions": ["*"],
        "expiresAt": (time::OffsetDateTime::now_utc() + time::Duration::days(1))
            .format(&::time::format_description::well_known::Rfc3339).unwrap()
    });
    let (response, code) = server.add_api_key(key_body).await;
    assert_eq!(code, 201, "{:?}", response);
    let key = response["key"].as_str().unwrap();
    let uid = response["uid"].as_str().unwrap();

    // indexRules: {"sales": null} — null means "whitelisted, no filter restriction"
    // Expected: all 3 documents returned (not 0, not 403)
    let token = generate_tenant_token(uid, key, hashmap! {
        "searchRules" => crate::json!({"*": {}}),
        "indexRules" => crate::json!({"sales": null}),
        "exp" => crate::json!((time::OffsetDateTime::now_utc() + time::Duration::hours(1)).unix_timestamp())
    });
    server.use_api_key(&token);

    let (response, code) = server.dummy_request("GET", "/indexes/sales/documents").await;
    assert_eq!(code, 200, "Phase3-NullRule: expected 200 for null-rule token, got {}: {:?}", code, response);
    let results = response["results"].as_array().expect("Phase3-NullRule: expected results array");
    assert_eq!(
        results.len(), 3,
        "Phase3-NullRule: expected all 3 documents for null-rule (whitelist), got {}: {:?}",
        results.len(), response
    );
}
