#![allow(unused_macros)]
pub const OUR_LOGGER_NAME: &str = "monetdb";

macro_rules! log {
    (target: $target:expr, $($arg:tt)+) => (::log::log!(target: $target, $($arg)+));
    ($($arg:tt)+) => (::log::log!(target: "monetdb", $($arg)+))
}

macro_rules! error {
    (target: $target:expr, $($arg:tt)+) => (::log::error!(target: $target, $($arg)+));
    ($($arg:tt)+) => (::log::error!(target: "monetdb", $($arg)+))
}

macro_rules! warn {
    (target: $target:expr, $($arg:tt)+) => (::log::warn!(target: $target, $($arg)+));
    ($($arg:tt)+) => (::log::warn!(target: "monetdb", $($arg)+))
}

macro_rules! info {
    (target: $target:expr, $($arg:tt)+) => (::log::info!(target: $target, $($arg)+));
    ($($arg:tt)+) => (::log::info!(target: "monetdb", $($arg)+))
}

macro_rules! debug {
    (target: $target:expr, $($arg:tt)+) => (::log::debug!(target: $target, $($arg)+));
    ($($arg:tt)+) => (::log::debug!(target: "monetdb", $($arg)+))
}

macro_rules! trace {
    (target: $target:expr, $($arg:tt)+) => (::log::trace!(target: $target, $($arg)+));
    ($($arg:tt)+) => (::log::trace!(target: "monetdb", $($arg)+))
}

macro_rules! log_enabled {
    (target: $target:expr, $($arg:tt)+) => (::log::log_enabled!(target: $target, $($arg)+));
    ($($arg:tt)+) => (::log::log_enabled!(target: "monetdb", $($arg)+))
}

#[cfg(test)]
mod tests {
    use std::{
        mem,
        sync::{Mutex, MutexGuard},
    };

    use log::Level;

    use super::OUR_LOGGER_NAME;

    struct TestLogger {
        global_lock: Mutex<bool>,
        targets: Mutex<Vec<(String, Level)>>,
    }

    impl TestLogger {
        fn start(&self) -> MutexGuard<bool> {
            // the global mutex gets poisoned if an assertion fails while
            // it is held. we don't care.
            let mut guard = match self.global_lock.lock() {
                Ok(g) => g,
                Err(e) => e.into_inner(),
            };

            // initialize the logger exactly once
            if !*guard {
                log::set_logger(&TEST_LOGGER).unwrap();
                log::set_max_level(log::LevelFilter::Trace);
                *guard = true;
            }

            let mut targets = self.targets.lock().unwrap();
            targets.clear();
            guard
        }

        fn logged(&self) -> Vec<(String, Level)> {
            let mut targets = self.targets.lock().unwrap();
            mem::take(&mut *targets)
        }
    }

    static TEST_LOGGER: TestLogger = TestLogger {
        global_lock: Mutex::new(false),
        targets: Mutex::new(vec![]),
    };

    impl log::Log for TestLogger {
        fn enabled(&self, metadata: &log::Metadata) -> bool {
            metadata.target() != "custom"
        }

        fn log(&self, record: &log::Record) {
            let mut targets = self.targets.lock().unwrap();
            targets.push((record.target().to_string(), record.level()));
        }

        fn flush(&self) {}
    }

    #[test]
    #[ignore]
    fn test_log() {
        let _exclusion = TEST_LOGGER.start();
        log!(Level::Info, "regular target");
        log!(target: "custom", Level::Trace, "custom target");
        assert_eq!(
            TEST_LOGGER.logged(),
            &[
                (OUR_LOGGER_NAME.to_string(), Level::Info),
                ("custom".to_string(), Level::Trace)
            ]
        )
    }

    #[test]
    #[ignore]
    fn test_error() {
        let _exclusion = TEST_LOGGER.start();
        let lvl = Level::Error;
        error!("regular target");
        error!(target: "custom", "custom target");
        assert_eq!(
            TEST_LOGGER.logged(),
            &[
                (OUR_LOGGER_NAME.to_string(), lvl),
                ("custom".to_string(), lvl)
            ]
        )
    }

    #[test]
    #[ignore]
    fn test_warn() {
        let _exclusion = TEST_LOGGER.start();
        let lvl = Level::Warn;
        warn!("regular target");
        warn!(target: "custom", "custom target");
        assert_eq!(
            TEST_LOGGER.logged(),
            &[
                (OUR_LOGGER_NAME.to_string(), lvl),
                ("custom".to_string(), lvl)
            ]
        )
    }

    #[test]
    #[ignore]
    fn test_info() {
        let _exclusion = TEST_LOGGER.start();
        let lvl = Level::Info;
        info!("regular target");
        info!(target: "custom", "custom target");
        assert_eq!(
            TEST_LOGGER.logged(),
            &[
                (OUR_LOGGER_NAME.to_string(), lvl),
                ("custom".to_string(), lvl)
            ]
        )
    }

    #[test]
    #[ignore]
    fn test_debug() {
        let _exclusion = TEST_LOGGER.start();
        let lvl = Level::Debug;
        debug!("regular target");
        debug!(target: "custom", "custom target");
        assert_eq!(
            TEST_LOGGER.logged(),
            &[
                (OUR_LOGGER_NAME.to_string(), lvl),
                ("custom".to_string(), lvl)
            ]
        )
    }

    #[test]
    #[ignore]
    fn test_trace() {
        let _exclusion = TEST_LOGGER.start();
        let lvl = Level::Trace;
        trace!("regular target");
        trace!(target: "custom", "custom target");
        assert_eq!(
            TEST_LOGGER.logged(),
            &[
                (OUR_LOGGER_NAME.to_string(), lvl),
                ("custom".to_string(), lvl)
            ]
        )
    }

    #[test]
    #[ignore]
    fn test_log_enabled() {
        let _exclusion = TEST_LOGGER.start();
        assert!(log_enabled!(Level::Debug));
        assert!(!log_enabled!(target: "custom", Level::Debug));
    }
}
