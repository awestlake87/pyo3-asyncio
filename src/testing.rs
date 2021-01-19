//! # PyO3 Asyncio Testing Utilities
//!
//! This module provides some utilities for parsing test arguments as well as running and filtering
//! a sequence of tests.
//!
//! As mentioned [here](crate#pythons-event-loop), PyO3 Asyncio tests cannot use the default test
//! harness since it doesn't allow Python to gain control over the main thread. Instead, we have to
//! provide our own test harness in order to create integration tests.

use std::{future::Future, pin::Pin};

use clap::{App, Arg};
use futures::stream::{self, StreamExt};
use pyo3::prelude::*;

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
