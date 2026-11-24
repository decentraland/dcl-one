use super::*;

const NATIVE: &str = "/convert";
const NATIVE_ALIAS: &str = "/metamorph-api/convert";
const BROWSER: &str = "/media/convert";
const BROWSER_ALIAS: &str = "/metamorph-api/media/convert";
const LANDSCAPE: &str = "/events-assets-099ac00/poster/landscape.png";
const VERTICAL: &str = "/events-assets-099ac00/poster-vertical/vertical.webp";
const DEFAULT_POSTER: &str = "/events/assets/event-default.webp";

fn webp(width: u32, height: u32, alpha: u8) -> Bytes {
    let pixels = image::RgbaImage::from_fn(width, height, |_, y| {
        if y < height / 2 {
            image::Rgba([240, 10, 10, alpha])
        } else {
            image::Rgba([10, 10, 240, 255])
        }
    });
    let mut encoded = std::io::Cursor::new(Vec::new());
    pixels
        .write_to(&mut encoded, image::ImageFormat::WebP)
        .unwrap();
    Bytes::from(encoded.into_inner())
}

// The origin declares JPEG regardless of the fixture, including the WebP at landscape.png.
async fn poster_source(body: Bytes, first_body: Option<Bytes>) -> (String, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = hits.clone();
    let stub = Router::new().fallback(move || {
        let n = counter.fetch_add(1, Ordering::SeqCst);
        let body = if n == 0 {
            first_body.as_ref().unwrap_or(&body)
        } else {
            &body
        }
        .clone();
        async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            ([(header::CONTENT_TYPE, "image/jpeg")], body)
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/realm", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, stub).await.unwrap() });
    (base, hits)
}

fn sources(base: &str, remote: bool) -> ConvertSources {
    ConvertSources {
        remote,
        own_url: Some(url::Url::parse(base).unwrap()),
        ..ConvertSources::default()
    }
}

fn assert_jpeg(body: &[u8], dimensions: (u32, u32)) {
    assert!(body.starts_with(&[0xff, 0xd8, 0xff]));
    let image = image::load_from_memory(body).unwrap().to_rgb8();
    assert_eq!(image.dimensions(), dimensions);
    let [r, _, b] = image.get_pixel(image.width() / 2, image.height() / 4).0;
    assert!(r > 200 && b < 40, "top of poster should be red");
    let [r, _, b] = image.get_pixel(image.width() / 2, image.height() * 3 / 4).0;
    assert!(b > 200 && r < 40, "bottom of poster should be blue");
}

#[tokio::test]
async fn native_poster_stays_readable_in_both_cache_orders_and_across_aliases() {
    let original = webp(1200, 600, 255);
    let (base, hits) = poster_source(original.clone(), None).await;
    let url = q(&format!("{base}{LANDSCAPE}"));
    for browser_first in [true, false] {
        let before = hits.load(Ordering::SeqCst);
        let app = app_reading(crate::http::Outbound::for_tests(), sources(&base, true));
        let browser = format!("{BROWSER}?url={url}");
        if browser_first {
            let (status, headers, bytes) = get(&app, &browser).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(headers[header::CONTENT_TYPE], "image/webp");
            assert_eq!(bytes, original);
        }
        let routes = if browser_first {
            [NATIVE, NATIVE_ALIAS]
        } else {
            [NATIVE_ALIAS, NATIVE]
        };
        let mut raster = None;
        let mut lengths = Vec::new();
        let methods = if browser_first {
            ["GET", "HEAD"]
        } else {
            ["HEAD", "GET"]
        };
        for route in routes {
            for method in methods {
                let (status, headers, bytes) =
                    request(&app, method, &format!("{route}?url={url}")).await;
                assert_eq!(status, StatusCode::OK);
                assert_eq!(headers[header::CONTENT_TYPE], "image/jpeg");
                assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=86400");
                lengths.push(
                    headers[header::CONTENT_LENGTH]
                        .to_str()
                        .unwrap()
                        .parse::<usize>()
                        .unwrap(),
                );
                if method == "HEAD" {
                    assert!(bytes.is_empty());
                } else {
                    assert_jpeg(&bytes, (960, 480));
                    if let Some(first) = &raster {
                        assert_eq!(&bytes, first);
                    }
                    raster = Some(bytes);
                }
            }
        }
        assert_eq!(lengths, vec![raster.unwrap().len(); 4]);
        for route in [BROWSER_ALIAS, BROWSER] {
            let (status, headers, bytes) = get(&app, &format!("{route}?url={url}")).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(headers[header::CONTENT_TYPE], "image/webp");
            assert_eq!(bytes, original);
        }
        for route in [BROWSER, BROWSER_ALIAS, NATIVE, NATIVE_ALIAS] {
            let (status, headers, bytes) = get(&app, &format!("{route}?url={url}&width=80")).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(headers[header::CONTENT_TYPE], "image/jpeg");
            assert_jpeg(&bytes, (80, 40));
        }
        assert_eq!(
            hits.load(Ordering::SeqCst) - before,
            3,
            "one fetch per representation"
        );
    }
}

#[tokio::test]
async fn vertical_and_default_posters_keep_alpha_and_unrelated_sources_keep_webp() {
    let original = webp(18, 36, 96);
    let (base, _) = poster_source(original.clone(), None).await;
    let app = app_reading(crate::http::Outbound::for_tests(), sources(&base, true));
    for path in [VERTICAL, DEFAULT_POSTER] {
        let (status, headers, bytes) = get(
            &app,
            &format!("{NATIVE}?url={}", q(&format!("{base}{path}"))),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(headers[header::CONTENT_TYPE], "image/png", "{path}");
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        let pixels = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(pixels.dimensions(), (18, 36));
        assert_eq!(pixels.get_pixel(8, 4).0, [240, 10, 10, 96]);
        assert_eq!(pixels.get_pixel(8, 30).0, [10, 10, 240, 255]);
    }
    let (status, headers, bytes) = get(
        &app,
        &format!(
            "{NATIVE}?url={}",
            q(&format!("{base}/other/poster/vertical.webp"))
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/webp");
    assert_eq!(bytes, original);
}

#[tokio::test]
async fn compatible_poster_bytes_are_forwarded_even_above_the_raster_bound() {
    let mut jpeg = std::io::Cursor::new(Vec::new());
    image::RgbImage::from_pixel(1100, 16, image::Rgb([20, 40, 60]))
        .write_to(&mut jpeg, image::ImageFormat::Jpeg)
        .unwrap();
    for (mime, original) in [
        ("image/png", png(1100, 16)),
        ("image/jpeg", Bytes::from(jpeg.into_inner())),
    ] {
        let (base, _) = poster_source(original.clone(), None).await;
        let app = app_reading(crate::http::Outbound::for_tests(), sources(&base, true));
        let uri = format!("{NATIVE}?url={}", q(&format!("{base}{LANDSCAPE}")));
        let (status, headers, bytes) = get(&app, &uri).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], mime);
        assert_eq!(bytes, original);
    }
}

#[tokio::test]
async fn denied_and_invalid_native_posters_do_not_populate_the_cache() {
    let malformed = Bytes::from_static(b"RIFF\x10\0\0\0WEBPVP8L");
    let (base, hits) = poster_source(webp(24, 12, 255), Some(malformed)).await;
    let uri = format!("{NATIVE}?url={}", q(&format!("{base}{LANDSCAPE}")));
    let restricted = app_reading(crate::http::Outbound::for_tests(), sources(&base, false));
    assert_eq!(get(&restricted, &uri).await.0, StatusCode::FORBIDDEN);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    let app = app_reading(crate::http::Outbound::for_tests(), sources(&base, true));
    assert_eq!(get(&app, &uri).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _, bytes) = get(&app, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_jpeg(&bytes, (24, 12));
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn poster_limits_reject_oversized_bodies_and_dimensions() {
    use image::ImageEncoder;

    // The JPEG pixel data is small. Its embedded profile puts the source over 8 MiB.
    let mut jpeg = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut jpeg);
    encoder
        .set_icc_profile(vec![0; MAX_THUMBNAIL_SOURCE_BYTES])
        .unwrap();
    encoder
        .write_image(&[20, 40, 60], 1, 1, image::ExtendedColorType::Rgb8)
        .unwrap();
    for (source, expected) in [
        (Bytes::from(jpeg), StatusCode::PAYLOAD_TOO_LARGE),
        (webp(8193, 2, 255), StatusCode::UNPROCESSABLE_ENTITY),
    ] {
        let (base, hits) = poster_source(webp(24, 12, 255), Some(source)).await;
        let app = app_reading(crate::http::Outbound::for_tests(), sources(&base, true));
        let uri = format!("{NATIVE}?url={}", q(&format!("{base}{LANDSCAPE}")));
        let (status, headers, _) = get(&app, &uri).await;
        assert_eq!(status, expected);
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
        let (status, _, bytes) = get(&app, &uri).await;
        assert_eq!(status, StatusCode::OK);
        assert_jpeg(&bytes, (24, 12));
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }
}

#[tokio::test]
async fn video_at_a_poster_url_keeps_the_native_video_contract() {
    let original = video("mp4");
    let (base, _) = poster_source(original.clone(), None).await;
    let app = app_reading(crate::http::Outbound::for_tests(), sources(&base, true));
    let uri = format!("{NATIVE}?url={}", q(&format!("{base}{LANDSCAPE}")));
    let (status, headers, body) = get(&app, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "video/mp4");
    assert_eq!(body, original);
    assert_eq!(
        get(&app, &format!("{uri}&width=640")).await.0,
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
}

#[tokio::test]
async fn unsupported_poster_image_is_rejected_only_on_the_native_route() {
    let avif = Bytes::from(ftyp(b"avif", &[*b"mif1"]));
    let (base, hits) = poster_source(avif.clone(), None).await;
    let app = app_reading(crate::http::Outbound::for_tests(), sources(&base, true));
    let url = q(&format!("{base}{LANDSCAPE}"));
    let (status, headers, body) = get(&app, &format!("{BROWSER}?url={url}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/avif");
    assert_eq!(body, avif);
    for method in ["GET", "HEAD"] {
        let (status, headers, _) = request(&app, method, &format!("{NATIVE}?url={url}")).await;
        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    }
    assert_eq!(hits.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn native_poster_burst_shares_one_fetch_and_warm_hits_skip_a_full_queue() {
    let (base, hits) = poster_source(webp(160, 80, 255), None).await;
    let settings = Settings::for_tests(BackendKind::Disabled);
    let backend = Backend::build(&settings, reqwest::Client::new());
    let state = Arc::new(
        MediaState::with_backend(lazy_pool(), settings, backend)
            .with_http(crate::http::Outbound::for_tests())
            .with_convert_sources(sources(&base, true))
            .with_convert_slots(1, 1),
    );
    let app = router(state.clone());
    let mut burst = tokio::task::JoinSet::new();
    for route in [NATIVE, NATIVE_ALIAS].into_iter().cycle().take(12) {
        let app = app.clone();
        let uri = format!("{route}?url={}", q(&format!("{base}{LANDSCAPE}")));
        burst.spawn(async move { get(&app, &uri).await });
    }
    for (status, headers, bytes) in burst.join_all().await {
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "image/jpeg");
        assert_jpeg(&bytes, (160, 80));
    }
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    let slot = state.convert_slots.acquire(Duration::ZERO).await.unwrap();
    let line = state.convert_slots.admitted.try_acquire().unwrap();
    let warm = format!("{NATIVE}?url={}", q(&format!("{base}{LANDSCAPE}")));
    assert_eq!(get(&app, &warm).await.0, StatusCode::OK);
    let cold = format!("{NATIVE}?url={}", q(&format!("{base}{VERTICAL}")));
    let (status, headers, _) = get(&app, &cold).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(headers[header::RETRY_AFTER], "1");
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    drop((slot, line));
    assert_eq!(get(&app, &cold).await.0, StatusCode::OK);
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}
