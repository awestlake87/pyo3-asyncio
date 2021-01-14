//! # PyO3 Asyncio Testing Utilities
//!
//! This module provides some utilities for parsing test arguments as well as running and filtering
//! a sequence of tests.
//!
//! As mentioned [here](crate#pythons-event-loop), PyO3 Asyncio tests cannot use the default test
//! harness since it doesn't allow Python to gain control over the main thread. Instead, we have to
//! provide our own test harness in order to create integration tests.
//!
//! ## Creating A PyO3 Asyncio Integration Test
//!
//! ### Main Test File
//! First, we need to create the test's main file. Although these tests are considered integration
//! tests, we cannot put them in the `tests` directory since that is a special directory owned by
//! Cargo. Instead, we put our tests in a `pytests` directory, although the name `pytests` is just
//! a convention.
//!
//! `pytests/test_example.rs`
//! ```no_run
//!
//! fn main() {
//!
//! }
//! ```
//!
//! ### Test Manifest Entry
//! Next, we need to add our test file to the Cargo manifest. Add the following section to your
//! `Cargo.toml`
//!
//! ```toml
//! [[test]]
//! name = "test_example"
//! path = "pytests/test_example.rs"
//! harness = false
//! ```
//!
//! At this point you should be able to run the test via `cargo test`
//!
//! ### Using the PyO3 Asyncio Test Harness
//! Now that we've got our test registered with `cargo test`, we can start using the PyO3 Asyncio
//! test harness.
//!
//! In your `Cargo.toml` add the testing feature to `pyo3-asyncio`:
//! ```toml
//! pyo3-asyncio = { version = "0.13", features = ["testing"] }
//! ```
//!
//! Now, in your test's main file, call [`testing::test_main`]:
//!
//! ```no_run
//! fn main() {
//!     pyo3_asyncio::testing::test_main(vec![]);
//! }
//! ```
//!
//! ### Adding Tests to the PyO3 Asyncio Test Harness
//!
//! ```no_run
//! use pyo3_asyncio::testing::Test;
//!
//! fn main() {
//!     pyo3_asyncio::testing::test_main(vec![
//!         Test::new_async(
//!             "test_async_sleep".into(),
//!              async move {
//!                  // async test body
//!                 Ok(())
//!              }
//!         ),
//!         Test::new_sync(
//!             "test_blocking_sleep".into(),
//!             || {
//!                 // blocking test body
//!                 Ok(())
//!             }
//!         ),
//!     ]);
//! }
//! ```

use std::{future::Future, pin::Pin};

use clap::{App, Arg};
use futures::stream::{self, StreamExt};
use pyo3::prelude::*;

use crate::{dump_err, run_until_complete, with_runtime};

/// Args that should be provided to the test program
///
/// These args are meant to mirror the default test harness's args.
/// > Currently only `--filter` is supported.
pub struct Args {
    filter: Option<String>,
}

impl Default for Args {
    fn default() -> Self {
        Self { filter: None }
    }
}

/// Parse the test args from the command line
///
/// This should be called at the start of your test harness to give the CLI some
/// control over how our tests are run.
///
/// Ideally, we should mirror the default test harness's arguments exactly, but
/// for the sake of simplicity, only filtering is supported for now. If you want
/// more features, feel free to request them
/// [here](https://github.com/awestlake87/pyo3-asyncio/issues).
///
/// # Examples
///
/// Running the following function:
/// ```no_run
/// # use pyo3_asyncio::testing::parse_args;
/// let args = parse_args("PyO3 Asyncio Example Test Suite");
/// ```
///
/// Produces the following usage string:
///
/// ```bash
/// Pyo3 Asyncio Example Test Suite
/// USAGE:
/// test_example [TESTNAME]
///
/// FLAGS:
/// -h, --help       Prints help information
/// -V, --version    Prints version information
///
/// ARGS:
/// <TESTNAME>    If specified, only run tests containing this string in their names
/// ```
pub fn parse_args(suite_name: &str) -> Args {
    let matches = App::new(suite_name)
        .arg(
            Arg::with_name("TESTNAME")
                .help("If specified, only run tests containing this string in their names"),
        )
        .get_matches();

    Args {
        filter: matches.value_of("TESTNAME").map(|name| name.to_string()),
    }
}

/// Wrapper around a test function or future to be passed to the test harness
pub struct Test {
    name: String,
    task: Pin<Box<dyn Future<Output = PyResult<()>> + Send>>,
}

impl Test {
    /// Construct a test from a future
    pub fn new_async(
        name: String,
        fut: impl Future<Output = PyResult<()>> + Send + 'static,
    ) -> Self {
        Self {
            name,
            task: Box::pin(fut),
        }
    }

    /// Construct a test from a blocking function (like the traditional `#[test]` attribute)
    pub fn new_sync<F>(name: String, func: F) -> Self
    where
        F: FnOnce() -> PyResult<()> + Send + 'static,
    {
        Self {
            name,
            task: Box::pin(async move { crate::spawn_blocking(func).await.unwrap() }),
        }
    }
}

/// Run a sequence of tests while applying any necessary filtering from the `Args`
pub async fn test_harness(tests: Vec<Test>, args: Args) -> PyResult<()> {
    stream::iter(tests)
        .for_each_concurrent(Some(4), |test| {
            let mut ignore = false;

            if let Some(filter) = args.filter.as_ref() {
                if !test.name.contains(filter) {
                    ignore = true;
                }
            }

            async move {
                if !ignore {
                    test.task.await.unwrap();
                    println!("test {} ... ok", test.name);
                }
            }
        })
        .await;

    Ok(())
}

/// Default main function for the test harness.
///
/// This is meant to perform the necessary initialization for most test cases. If you want
/// additional control over the initialization (i.e. env_logger initialization), you can use this
/// function as a template.
pub fn test_main(tests: Vec<Test>) {
    Python::with_gil(|py| {
        with_runtime(py, || {
            let args = parse_args("Pyo3 Asyncio Test Suite");

            run_until_complete(py, test_harness(tests, args))?;
            Ok(())
        })
        .map_err(dump_err(py))
        .unwrap();
    })
}
