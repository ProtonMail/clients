use std::{fs, path::PathBuf};

use futures::future::try_join_all;
use mail_core_common::datatypes::{LightOrDarkMode, SenderImageSize};
use mail_core_common::test_utils::test_context::TestContext;
use pretty_assertions::assert_eq;
use test_case::test_case;
use wiremock::{
    Mock, ResponseTemplate,
    matchers::{method, path, query_param},
};

fn test_address() -> String {
    "test@example.com".to_owned()
}

#[tokio::test]
async fn get_empty_sender_image() {
    let ctx = TestContext::new().await;
    let user_ctx = ctx.user_context().await;

    ctx.mock_get_images_logo(&test_address(), vec![]).await;

    let image_path = user_ctx
        .image_for_sender(
            test_address().into(),
            None,
            None,
            None,
            None,
            &mut user_ctx.mail_stash().connection(),
        )
        .await
        .expect("failed to get image");

    assert!(image_path.is_none());
}

#[ignore] // TODO: ET-6124
#[tokio::test]
async fn concurrency() {
    let ctx = TestContext::new().await;
    let user_ctx = ctx.user_context().await;

    Mock::given(method("GET"))
        .and(path("/api/core/v4/images/logo"))
        .and(query_param("Address", test_address()))
        .respond_with(
            ResponseTemplate::new(200).set_body_bytes("This is a very nice image lol".as_bytes()),
        )
        .named("Get images/logo (but allow > 1)")
        .mount(ctx.mock_server())
        .await;

    let requests = (0..30).map(|_| {
        let ctx_clone = user_ctx.clone();
        async move {
            let mut tether = ctx_clone.mail_stash().connection();
            ctx_clone
                .image_for_sender(test_address().into(), None, None, None, None, &mut tether)
                .await
        }
    });

    let result = try_join_all(requests).await.unwrap();
    for result in result {
        result.unwrap();
    }

    let mut tether = user_ctx.mail_stash().connection();
    let path_given = user_ctx
        .image_for_sender(test_address().into(), None, None, None, None, &mut tether)
        .await
        .unwrap()
        .unwrap();

    let path = ctx
        .tmp_dir
        .path()
        .join("core-cache")
        .join(ctx.user_context().await.user_id().to_string())
        .join("sender_images")
        .join(format!("{}-0xeb804ef98475036c.svg", test_address(),));
    assert_eq!(path, PathBuf::from(path_given));

    let image = fs::read_to_string(path).unwrap();
    assert_eq!(image, "This is a very nice image lol");
}

#[test_case(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A], "png"; "png")]
#[test_case(&[0x52, 0x49, 0x46, 0x46, 0x00, 0x01, 0x02, 0x03, 0x57, 0x45, 0x42, 0x50], "webp"; "webp")]
#[tokio::test]
async fn image_extension(bytes: &[u8], expected: &str) {
    let ctx = TestContext::new().await;
    let user_ctx = ctx.user_context().await;

    ctx.mock_get_images_logo(&test_address(), bytes.to_vec())
        .await;

    let image_path = user_ctx
        .image_for_sender(
            test_address().into(),
            None,
            None,
            None,
            None,
            &mut user_ctx.mail_stash().connection(),
        )
        .await
        .expect("failed to get image")
        .unwrap();

    assert!(image_path.ends_with(expected));
    assert_eq!(fs::read(&image_path).unwrap(), bytes);

    // Called two times but only calls the api once.
    let image_path_2 = user_ctx
        .image_for_sender(
            test_address().into(),
            None,
            None,
            None,
            None,
            &mut user_ctx.mail_stash().connection(),
        )
        .await
        .expect("failed to get image")
        .unwrap();
    assert_eq!(image_path, image_path_2);
}

#[test_case(SenderImageSize::S16, "16", "1"; "s16")]
#[test_case(SenderImageSize::S32, "32", "2"; "s32")]
#[test_case(SenderImageSize::S64, "64", "3"; "s64")]
#[test_case(SenderImageSize::S128, "128", "4"; "s128")]
#[tokio::test]
async fn sender_image_size_sets_size_and_scale_factor(
    size: SenderImageSize,
    expected_size: &str,
    expected_factor: &str,
) {
    let ctx = TestContext::new().await;
    let user_ctx = ctx.user_context().await;

    Mock::given(method("GET"))
        .and(path("/api/core/v4/images/logo"))
        .and(query_param("Address", test_address()))
        .and(query_param("Format", "png"))
        .and(query_param("Mode", "light"))
        .and(query_param("Size", expected_size))
        .and(query_param("MaxScaleUpFactor", expected_factor))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"image data".as_slice()))
        .expect(1)
        .named("get images/logo with address, format, mode, size and scale factor")
        .mount(ctx.mock_server())
        .await;

    let image_path = user_ctx
        .image_for_sender(
            test_address().into(),
            None,
            Some("png".to_owned()),
            Some(LightOrDarkMode::Light),
            Some(size),
            &mut user_ctx.mail_stash().connection(),
        )
        .await
        .expect("failed to get image");

    assert!(image_path.is_some());
}
