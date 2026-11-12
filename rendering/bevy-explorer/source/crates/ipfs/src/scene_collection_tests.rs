use base64::{prelude::BASE64_STANDARD, Engine};
use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use serde_json::json;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    time::timeout,
};

use super::*;

const LIMIT: Duration = Duration::from_secs(10);

async fn fixture() -> Arc<IpfsIo> {
    IoTaskPool::get_or_init(bevy::tasks::TaskPool::new);
    let io = Arc::new(IpfsIo::new(
        false,
        Box::new(MemoryAssetReader {
            root: Dir::default(),
        }),
        None,
        Default::default(),
        1,
        #[cfg(feature = "ipfs_debug")]
        tokio::sync::mpsc::unbounded_channel().0,
    ));
    io.context.write().await.register_entity(
        "scene".to_owned(),
        IpfsEntity {
            collection: ContentMap::from_content([
                (
                    "scene.js".to_owned(),
                    format!(
                        "b64-{}",
                        BASE64_STANDARD.encode("/workspace/project/scene.js-fixture")
                    ),
                ),
                ("removed.glb".to_owned(), "removed".to_owned()),
                ("assets/imported/chair.glb".to_owned(), "chair".to_owned()),
            ]),
            metadata: None,
        },
    );
    io
}

#[tokio::test]
async fn live_content_refresh_uses_owned_metadata_after_registry_eviction() {
    timeout(LIMIT, async {
        let io = fixture().await;
        io.context
            .write()
            .await
            .entities
            .get_mut("scene")
            .unwrap()
            .metadata_evicted = true;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let public_url = format!("http://{}", listener.local_addr().unwrap());
        let about = ServerAbout {
            content: Some(EndpointConfig {
                healthy: true,
                public_url: public_url.clone(),
            }),
            ..Default::default()
        };
        io.context.write().await.about = Some(about.clone());
        io.realm_config_sender.send_replace(Some(RealmConfig {
            about_url: format!("{public_url}/about"),
            address: public_url,
            about,
            connected: true,
            secret: None,
            spawn_parcel: None,
        }));
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut request = BufReader::new(stream);
            let mut line = String::new();
            request.read_line(&mut line).await.unwrap();
            assert_eq!(line, "POST /entities/active HTTP/1.1\r\n");
            let mut length = None;
            loop {
                line.clear();
                assert!(request.read_line(&mut line).await.unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':') {
                    if name.eq_ignore_ascii_case("content-length") {
                        length = Some(value.trim().parse::<usize>().unwrap());
                    }
                }
            }
            let mut body = vec![0; length.unwrap()];
            request.read_exact(&mut body).await.unwrap();
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
                json!({"pointers": ["-7,13"]})
            );
            let response = json!([{
                "id": "scene",
                "pointers": ["-7,13"],
                "content": [
                    {"file": "scene.js", "hash": "fresh-script"},
                    {"file": "assets/imported/chair.glb", "hash": "chair"},
                    {"file": "new.glb", "hash": "new-model"}
                ],
                "metadata": null
            }])
            .to_string();
            request
                .into_inner()
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                        response.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        });
        let metadata = json!({"scene": {"base": "-7,13", "parcels": ["-6,13", "-7,13"]}});
        assert!(io
            .refresh_scene_collection("scene", Some(&metadata))
            .await
            .unwrap());
        server.await.unwrap();
        assert_eq!(
            io.scene_content_files("scene").await,
            ["assets/imported/chair.glb", "new.glb", "scene.js"]
        );
        let context = io.context.read().await;
        assert_eq!(context.entities["scene"].collection.0["scene.js"], "fresh-script");
        assert!(context.entities["scene"].metadata.is_none());
        assert!(context.entities["scene"].metadata_evicted);
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn content_refresh_reports_missing_fallback_metadata_without_replacing_content() {
    timeout(LIMIT, async {
        let io = fixture().await;
        let previous = io.scene_content_files("scene").await;
        let error = io
            .refresh_scene_collection("scene", None)
            .await
            .unwrap_err();
        assert!(error.contains("metadata is unavailable"));
        assert_eq!(io.scene_content_files("scene").await, previous);
    })
    .await
    .unwrap();
}
