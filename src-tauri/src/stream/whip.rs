/*use std::sync::atomic::{AtomicBool, Ordering};

use crate::stream::handler::StreamHandle;

 v2 migration: swap this struct for `FfmpegStreamHandle` behind the same trait.
pub struct WhipStreamHandle {
    live: AtomicBool,
}

impl WhipStreamHandle {
    pub fn new() -> Self {
            Self {
                live: AtomicBool::new(false),
            }
        }

        pub fn set_live(&self, live: bool) {
            self.live.store(live, Ordering::SeqCst);
        }
    }

    impl Default for WhipStreamHandle {
        fn default() -> Self {
            Self::new()
        }
    }

    impl StreamHandle for WhipStreamHandle {
        fn is_live(&self) -> bool {
            self.live.load(Ordering::SeqCst)
        }

        fn stop(&self) {
            self.live.store(false, Ordering::SeqCst);
        }
}*/
