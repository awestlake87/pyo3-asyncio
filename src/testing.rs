use std::{future::Future, pin::Pin};

use clap::{App, Arg};
use futures::stream::{Stream, StreamExt};
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
pub async fn test_harness(tests: impl Stream<Item = Test>, args: Args) -> PyResult<()> {
    tests
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
pub fn test_main(tests: impl Stream<Item = Test> + Send + 'static) {
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
