use super::{AudioSource, StopFlag};
use crate::logging::{mlog, LogCat};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct AudioHandle {
    pub source: AudioSource,
    stop: StopFlag,
    writer: Option<tauri::async_runtime::JoinHandle<()>>,
}

impl AudioHandle {
    pub async fn shutdown(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(w) = self.writer.take() {
            w.abort();
            let _ = w.await;
        }
    }
}

fn silent(stop: StopFlag) -> AudioHandle {
    AudioHandle {
        source: AudioSource::Silent,
        stop,
        writer: None,
    }
}

#[cfg(windows)]
pub fn start_audio() -> AudioHandle {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use tokio::io::AsyncWriteExt;
    use tokio::net::windows::named_pipe::ServerOptions;

    let stop: StopFlag = Arc::new(AtomicBool::new(false));

    let pipe_name = format!(r"\\.\pipe\momentum_audio_{:016x}", rand::random::<u64>());
    let server = match ServerOptions::new()
        .first_pipe_instance(true)
        .create(&pipe_name)
    {
        Ok(s) => s,
        Err(e) => {
            mlog!(
                LogCat::Stream,
                "[audio] pipe create failed: {e}; using silence"
            );
            return silent(stop);
        }
    };

    let ring: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::new()));

    // cpal Stream is !Send
    let (fmt_tx, fmt_rx) = std::sync::mpsc::channel::<Result<(u32, u16), String>>();
    let ring_thread = ring.clone();
    let stop_thread = stop.clone();
    std::thread::spawn(move || {
        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            let _ = fmt_tx.send(Err("no default output device".into()));
            return;
        };
        let config = match device.default_output_config() {
            Ok(c) => c,
            Err(e) => {
                let _ = fmt_tx.send(Err(e.to_string()));
                return;
            }
        };
        if config.sample_format() != cpal::SampleFormat::F32 {
            let _ = fmt_tx.send(Err(format!(
                "output device is not f32 ({:?})",
                config.sample_format()
            )));
            return;
        }
        let rate: u32 = config.sample_rate();
        let channels = config.channels();
        let stream_config: cpal::StreamConfig = config.into();

        // Buil  input stream on OUTPUT device => cpal WASAPI loopback.
        let cap = (rate as usize * channels as usize) / 5; // ~200ms
        let ring_cb = ring_thread.clone();
        let stream = device.build_input_stream(
            stream_config,
            move |data: &[f32], _| {
                if let Ok(mut r) = ring_cb.lock() {
                    r.extend(data.iter().copied());
                    while r.len() > cap {
                        r.pop_front();
                    }
                }
            },
            move |err| mlog!(LogCat::Stream, "[audio] cpal stream error: {err}"),
            None,
        );
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                let _ = fmt_tx.send(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = stream.play() {
            let _ = fmt_tx.send(Err(e.to_string()));
            return;
        }
        let _ = fmt_tx.send(Ok((rate, channels)));
        while !stop_thread.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        drop(stream);
    });

    let (rate, channels) = match fmt_rx.recv_timeout(std::time::Duration::from_secs(2)) {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            mlog!(
                LogCat::Stream,
                "[audio] loopback init failed: {e}; using silence"
            );
            stop.store(true, Ordering::SeqCst);
            return silent(stop);
        }
        Err(_) => {
            mlog!(
                LogCat::Stream,
                "[audio] loopback init timed out; using silence"
            );
            stop.store(true, Ordering::SeqCst);
            return silent(stop);
        }
    };

    mlog!(
        LogCat::Stream,
        "[audio] loopback up: {rate} Hz, {channels} ch, pipe {pipe_name}"
    );

    let bytes_per_sec = rate as u64 * channels as u64 * 4;
    let frame = channels as usize * 4;
    let cap_samples = (rate as usize * channels as usize) / 5; // ~200ms
    let stop_writer = stop.clone();
    let ring_writer = ring.clone();

    let writer = tauri::async_runtime::spawn(async move {
        // Wait for ffmpeg to open the pipe
        if server.connect().await.is_err() {
            return;
        }
        let mut server = server;
        let start = std::time::Instant::now();
        let mut written: u64 = 0;
        let mut ticker = tokio::time::interval(std::time::Duration::from_millis(20));
        loop {
            if stop_writer.load(Ordering::SeqCst) {
                break;
            }
            ticker.tick().await;

            let target = (start.elapsed().as_secs_f64() * bytes_per_sec as f64) as u64;
            let mut need = target.saturating_sub(written) as usize;
            need -= need % frame;
            if need == 0 {
                continue;
            }

            let mut buf: Vec<u8> = Vec::with_capacity(need);
            {
                let mut r = ring_writer.lock().unwrap();
                let take = (need / 4).min(r.len());
                for _ in 0..take {
                    let s = r.pop_front().unwrap();
                    buf.extend_from_slice(&s.to_le_bytes());
                }
                while r.len() > cap_samples {
                    r.pop_front();
                }
            }
            // Pad zeros WASAPI loopback no callback
            if buf.len() < need {
                buf.resize(need, 0);
            }
            if server.write_all(&buf).await.is_err() {
                break;
            }
            written += need as u64;
        }
    });

    AudioHandle {
        source: AudioSource::Pipe(pipe_name),
        stop,
        writer: Some(writer),
    }
}

#[cfg(not(windows))]
pub fn start_audio() -> AudioHandle {
    silent(Arc::new(AtomicBool::new(false)))
}
