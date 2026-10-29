//! A web app fetched by address: downloaded, unpacked, and the download not left behind. The
//! server of the crate serves the zip, as a bucket would.

mod support;

use benchmark::app_source::provision;
use benchmark::failure::Failure;
use support::TestServer;
use tokio_util::sync::CancellationToken;

/// A deflated zip (a folder `wrapped` holding `index.html`), made once by Python's zipfile.
const DEFLATED_ZIP: &str = concat!(
    "504b030414000000080000605b5ded88bb4d1c0000002902000012000000777261707065642f696e6465782e68746d6c",
    "b3c928c9cdb14b494dcb492c494d5118658c3272501936fae0240200504b030414000000000000605b5d000000000000",
    "0000000000000c000000777261707065642f7375622f504b0102140014000000080000605b5ded88bb4d1c0000002902",
    "0000120000000000000000000000800100000000777261707065642f696e6465782e68746d6c504b0102140014000000",
    "000000605b5d0000000000000000000000000c000000000000000000000080014c000000777261707065642f7375622f",
    "504b050600000000020002007a000000760000000000",
);

fn zip_bytes() -> Vec<u8> {
    (0..DEFLATED_ZIP.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&DEFLATED_ZIP[at..at + 2], 16).unwrap())
        .collect()
}

#[tokio::test]
async fn a_web_app_at_an_address_is_downloaded_unpacked_and_the_download_removed() {
    let server = TestServer::start("download").await;
    std::fs::write(server.shared.root.join("app.zip"), zip_bytes()).unwrap();
    let destination = server.dir.join("builds").join("change_build");
    std::fs::create_dir_all(destination.parent().unwrap()).unwrap();

    let address = format!("http://127.0.0.1:{}/app.zip", server.port);
    let target = destination.clone();
    let root = tokio::task::spawn_blocking(move || {
        provision(&address, &target, &CancellationToken::new())
    })
    .await
    .unwrap()
    .expect("the app is fetched");

    assert_eq!(
        root,
        std::path::absolute(destination.join("wrapped")).unwrap()
    );
    assert!(root.join("index.html").is_file());
    assert!(
        !destination.with_file_name("change_build.zip").exists(),
        "the download is deleted once it is unpacked"
    );
    server.stop().await;
}

#[tokio::test]
async fn an_interrupt_stops_a_download_before_it_starts() {
    let server = TestServer::start("download-interrupted").await;
    std::fs::write(server.shared.root.join("app.zip"), zip_bytes()).unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();

    let address = format!("http://127.0.0.1:{}/app.zip", server.port);
    let destination = server.dir.join("never");
    let result = tokio::task::spawn_blocking(move || provision(&address, &destination, &cancel))
        .await
        .unwrap();

    assert!(matches!(result, Err(Failure::Interrupted)), "{result:?}");
    assert!(!server.dir.join("never").exists());
    server.stop().await;
}
