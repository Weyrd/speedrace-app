use super::*;

#[tokio::test]
async fn available_and_resolve() {
    remember(Encoder::Amf, 1, false);
    remember(Encoder::Nvenc, 1, true);

    assert_eq!(available(false).await, vec![Encoder::Nvenc, Encoder::X264]);
    assert_eq!(resolve(Some(Encoder::Amf), false).await, Encoder::Nvenc);
    assert_eq!(resolve(None, false).await, Encoder::Nvenc);
    assert_eq!(resolve(Some(Encoder::Nvenc), false).await, Encoder::Nvenc);

    remember(Encoder::Nvenc, 1, false);
    assert_eq!(available(false).await, vec![Encoder::X264]);
    assert_eq!(resolve(None, false).await, Encoder::X264);
    assert_eq!(resolve(Some(Encoder::Nvenc), false).await, Encoder::X264);

    remember(Encoder::Amf, 2, false);
    remember(Encoder::Nvenc, 2, false);
    assert_eq!(
        resolve(Some(Encoder::X264), true).await,
        Encoder::X264,
        "x264 preference must skip hw probing entirely"
    );
}
