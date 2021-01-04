use std::{future::Future, pin::Pin};

use clap::{App, Arg};
use futures::stream::{Stream, StreamExt};
use pyo3::prelude::*;

pub struct Args {
    filter: Option<String>,
}

impl Default for Args {
    fn default() -> Self {
        Self { filter: None }
    }
}

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

pub struct Test {
    pub name: String,
    pub task: Pin<Box<dyn Future<Output = PyResult<()>> + Send>>,
}

impl Test {
    pub fn new_async(
        name: String,
        fut: impl Future<Output = PyResult<()>> + Send + 'static,
    ) -> Self {
        Self {
            name,
            task: Box::pin(fut),
        }
    }

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
