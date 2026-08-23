use super::*;
use crate::stream::CaptureSource;

fn settings(framerate: u32, resolution: u32) -> StreamSettings {
    StreamSettings {
        source: CaptureSource::Monitor { index: 0 },
        bitrate_kbps: 2000,
        framerate,
        resolution,
    }
}

#[tokio::test]
async fn ladder_construction() {
    remember(Encoder::Amf, 1, false);
    remember(Encoder::Nvenc, 1, true);

    let ladder = build_ladder(Some(Encoder::Amf), &settings(60, 720), false).await;
    assert_eq!(ladder.first().map(|r| r.encoder), Some(Encoder::Nvenc));
    assert!(!ladder.iter().any(|r| r.encoder == Encoder::Amf));

    remember(Encoder::Nvenc, 1, false);
    let ladder = build_ladder(None, &settings(30, 720), false).await;
    assert!(ladder.iter().all(|r| r.encoder == Encoder::X264));
    assert_eq!(ladder.len(), 1);
    assert_eq!(ladder[0].framerate, 30);
    assert_eq!(ladder[0].resolution, 720);

    let ladder = build_ladder(None, &settings(60, 1080), false).await;
    assert_eq!(
        ladder,
        vec![
            Rung {
                encoder: Encoder::X264,
                framerate: 60,
                resolution: 1080,
            },
            Rung {
                encoder: Encoder::X264,
                framerate: 30,
                resolution: 1080,
            },
            Rung {
                encoder: Encoder::X264,
                framerate: 30,
                resolution: 720,
            },
        ]
    );

    remember(Encoder::Nvenc, 1, true);
    let ladder = build_ladder(Some(Encoder::Nvenc), &settings(60, 1080), false).await;
    assert_eq!(ladder[0].encoder, Encoder::Nvenc);
    assert_eq!(ladder[0].framerate, 60);
    assert_eq!(ladder[0].resolution, 1080);
}
