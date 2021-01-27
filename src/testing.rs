//! # PyO3 Asyncio Testing Utilities
//!
//! This module provides some utilities for parsing test arguments as well as running and filtering
//! a sequence of tests.
//!
//! As mentioned [here](crate#pythons-event-loop), PyO3 Asyncio tests cannot use the default test
//! harness since it doesn't allow Python to gain control over the main thread. Instead, we have to
//! provide our own test harness in order to create integration tests.
//!
//! Running `pyo3-asyncio` code in doc tests _is_ supported however since each doc test has its own
//! `main` function. When writing doc tests, you may use the
//! [`#[pyo3_asyncio::async_std::main]`](crate::async_std::main) or
//! [`#[pyo3_asyncio::tokio::main]`](crate::tokio::main) macros on the test's main function to run
//! your test.
//!
//! If you don't want to write doc tests, you're unfortunately stuck with integration tests since
//! lib tests do not offer the same level of flexibility for the `main` fn. That being said,
//! overriding the default test harness can be quite different from what you're used to doing for
//! integration tests, so these next sections will walk you through this process.
//!
//! ### Main Test File
//! First, we need to create the test's main file. Although these tests are considered integration
//! tests, we cannot put them in the `tests` directory since that is a special directory owned by
//! Cargo. Instead, we put our tests in a `pytests` directory, although the name `pytests` is just
//! a convention.
//!
//! We'll also want to provide the test's main function. It's highly recommended that you use the
//! [`pyo3_asyncio::testing::test_main!`](pyo3_asyncio_macros::test_main) macro as it will take all of the tests marked with
//! [`#[pyo3_asyncio::tokio::test]`](crate::tokio::test) or
//! [`#[pyo3_asyncio::async_std::test]`](crate::async_std::test) and run them automatically.
//!
//! `pytests/test_example.rs` for the `tokio` runtime:
//! ```ignore
//! # #[cfg(all(feature = "tokio-runtime", feature = "attributes"))]
//! pyo3_asyncio::testing::test_main!(#[pyo3_asyncio::tokio::main], "Example Test Suite");
//! ```
//!
//! `pytests/test_example.rs` for the `async-std` runtime:
//! ```ignore
//! # #[cfg(all(feature = "async-std-runtime", feature = "attributes"))]
//! pyo3_asyncio::testing::test_main!(#[pyo3_asyncio::async_std::main], "Example Test Suite");
//! ```
//!
//! ### Cargo Configuration
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
//! Also, add the `testing` and `attributes` features to `pyo3-asyncio` and select your preferred
//! runtime:
//!
//! ```toml
//! [dependencies]
//! pyo3-asyncio = { version = "0.13", features = ["testing", "attributes", "async-std-runtime"] }
//! ```
//!
//! In order for the `test_main!` macro to find your tests, you'll also need to add an extra
//! `dev-dependency` for [`inventory`](https://github.com/dtolnay/inventory):
//! ```toml
//! [dev-dependencies]
//! inventory = "0.1"
//! ```
//!
//! At this point you should be able to run the test via `cargo test`
//!
//! ### Adding Tests to the PyO3 Asyncio Test Harness
//!
//! For `async-std` use the [`pyo3_asyncio::async_std::test`](crate::async_std::test) attribute:
//! ```
//! # #[cfg(all(feature = "async-std-runtime", feature = "attributes"))]
//! # mod tests {
//! use std::{time::Duration, thread};
//!
//! use pyo3::prelude::*;
//!
//! #[pyo3_asyncio::async_std::test]
//! async fn test_async_sleep() -> PyResult<()> {
//!     async_std::task::sleep(Duration::from_secs(1)).await;
//!     Ok(())
//! }
//!
//! #[pyo3_asyncio::async_std::test]
//! fn test_blocking_sleep() -> PyResult<()> {
//!     thread::sleep(Duration::from_secs(1));
//!     Ok(())
//! }
//! # }
//!
//! // ...
//! #
//! # // Doctests don't detect main fn when using the test_main!() macro, so we expand it into the
//! # // components of that macro instead.
//! #
//! # #[cfg(all(feature = "async-std-runtime", feature = "attributes"))]
//! # use pyo3::prelude::*;
//! #
//! # #[cfg(all(feature = "async-std-runtime", feature = "attributes"))]
//! # pyo3_asyncio::testing::test_structs!();
//! #
//! # #[cfg(all(feature = "async-std-runtime", feature = "attributes"))]
//! # #[pyo3_asyncio::async_std::main]
//! # async fn main() -> pyo3::PyResult<()> {
//! #     pyo3_asyncio::testing::test_main_body!("Example Test Suite");
//! #
//! #     Ok(())
//! # }
//! # #[cfg(not(all(feature = "async-std-runtime", feature = "attributes")))]
//! # fn main() {}
//! ```
//!
//! For `tokio` use the [`pyo3_asyncio::tokio::test`](crate::tokio::test) attribute:
//! ```
//! # #[cfg(all(feature = "tokio-runtime", feature = "attributes"))]
//! # mod tests {
//! use std::{time::Duration, thread};
//!
//! use pyo3::prelude::*;
//!
//! #[pyo3_asyncio::tokio::test]
//! async fn test_async_sleep() -> PyResult<()> {
//!     tokio::time::sleep(Duration::from_secs(1)).await;
//!     Ok(())
//! }
//!
//! #[pyo3_asyncio::tokio::test]
//! fn test_blocking_sleep() -> PyResult<()> {
//!     thread::sleep(Duration::from_secs(1));
//!     Ok(())
//! }
//! # }
//!
//! // ...
//! #
//! # // Doctests don't detect main fn when using the test_main!() macro, so we expand it into the
//! # // components of that macro instead.
//! # #[cfg(all(feature = "tokio-runtime", feature = "attributes"))]
//! # use pyo3::prelude::*;
//! #
//! # #[cfg(all(feature = "tokio-runtime", feature = "attributes"))]
//! # pyo3_asyncio::testing::test_structs!();
//! #
//! # #[cfg(all(feature = "tokio-runtime", feature = "attributes"))]
//! # #[pyo3_asyncio::tokio::main]
//! # async fn main() -> PyResult<()> {
//! #     pyo3_asyncio::testing::test_main_body!("Example Test Suite");
//! #
//! #     Ok(())
//! # }
//! # #[cfg(not(all(feature = "tokio-runtime", feature = "attributes")))]
//! # fn main() {}
//! ```
//!
//! ### Caveats
//!
//! The `test_main!()` macro _must_ be placed in the crate root. The `inventory` crate places
//! restrictions on the structures used by the `#[test]` attributes that force us to create a custom
//! `Test` structure in the crate root. If `test_main!()` is not expanded in the crate root, then
//! the resolution of `crate::Test` will fail in the proc macro expansion and the test will not
//! compile.
//!
//! #### Lib Tests
//!
//! Unfortunately, as we mentioned at the beginning, these utilities will only run in integration
//! tests and doc tests. Running lib tests are out of the question since we need control over the
//! main function. You can however perform compilation checks for lib tests. This is unfortunately
//! much more useful in doc tests than it is for lib tests, but the option is there if you want it.
//!
//! #### Allowing Compilation Checks in Lib Tests
//!
//! In order to allow the `#[test]` attributes to expand, we need to expand the `test_structs!()`
//! macro in the crate root. After that, `pyo3-asyncio` tests can be defined anywhere. Again, these
//! will not run, but they will be compile-checked during testing.
//!
//! `my-crate/src/lib.rs`
//! ```
//! # #[cfg(all(
//! #     any(feature = "async-std-runtime", feature = "tokio-runtime"),
//! #     feature = "attributes"
//! # ))]
//! pyo3_asyncio::testing::test_structs!();
//!
//! # #[cfg(all(
//! #     any(feature = "async-std-runtime", feature = "tokio-runtime"),
//! #     feature = "attributes"
//! # ))]
//! mod tests {
//!     use pyo3::prelude::*;
//!     
//! #   #[cfg(feature = "async-std-runtime")]
//!     #[pyo3_asyncio::async_std::test]
//!     async fn test_async_std_async_test_compiles() -> PyResult<()> {
//!         Ok(())
//!     }
//! #   #[cfg(feature = "async-std-runtime")]
//!     #[pyo3_asyncio::async_std::test]
//!     fn test_async_std_sync_test_compiles() -> PyResult<()> {
//!         Ok(())
//!     }
//!
//! #   #[cfg(feature = "tokio-runtime")]
//!     #[pyo3_asyncio::tokio::test]
//!     async fn test_tokio_async_test_compiles() -> PyResult<()> {
//!         Ok(())
//!     }
//! #   #[cfg(feature = "tokio-runtime")]
//!     #[pyo3_asyncio::tokio::test]
//!     fn test_tokio_sync_test_compiles() -> PyResult<()> {
//!         Ok(())
//!     }
//! }
//!
//! # fn main() {}
//! ```
//!
//! #### Expanding `test_main!()` for Doc Tests
//!
//! This is probably a pretty niche topic, and there's really no reason you would _need_ to do this
//! since usually you'd probably just want to use the `#[main]` attributes for your doc tests like we
//! mentioned at the beginning of this page. But since we had to do it for _this particular module_
//! of the docs, it's probably worth mentioning as a footnote.
//!
//! For some reason, doc tests don't interpret the `test_main!()` macro as providing `fn main()` and
//! will wrap the test body in another `fn main()`. For technical reasons listed in the
//! [Caveats](#caveats) section above, this is problematic because the `Test` structure that is
//! expanded by the `test_main!` macro will not be inside the crate root anymore.
//!
//! To get around this, we can instead expand `test_main!()` into its components for the doc test:
//! * [`test_structs!()`](crate::testing::test_structs)
//! * [`test_main_body!(suite_name: &'static str)`](crate::testing::test_main_body)
//!
//! The following `test_main!()` macro:
//!
//! ```ignore
//! pyo3_asyncio::testing::test_main!(#[pyo3_asyncio::async_std::main], "Example Test Suite");
//! ```
//!
//! Is equivalent to this expansion:
//!
//! ```
//! # #[cfg(all(feature = "async-std-runtime", feature = "attributes"))]
//! use pyo3::prelude::*;
//!
//! # #[cfg(all(feature = "async-std-runtime", feature = "attributes"))]
//! pyo3_asyncio::testing::test_structs!();
//!
//! # #[cfg(all(feature = "async-std-runtime", feature = "attributes"))]
//! #[pyo3_asyncio::async_std::main]
//! async fn main() -> PyResult<()> {
//!     pyo3_asyncio::testing::test_main_body!("Example Test Suite");
//!     Ok(())
//! }
//! # #[cfg(not(all(feature = "async-std-runtime", feature = "attributes")))]
//! # fn main() {}
//! ```

use std::{future::Future, pin::Pin};

use clap::{App, Arg};
use futures::stream::{self, StreamExt};
use pyo3::prelude::*;

/// <span class="module-item stab portability" style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"><code>attributes</code></span>
/// Provides the boilerplate for the `pyo3-asyncio` test harness in one line
#[cfg(feature = "attributes")]
pub use pyo3_asyncio_macros::test_main;

/// <span class="module-item stab portability" style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"><code>attributes</code></span>
/// Provides the `Test` structure and the `inventory` boilerplate for the `pyo3-asyncio` test harness
#[cfg(feature = "attributes")]
pub use pyo3_asyncio_macros::test_structs;

/// <span class="module-item stab portability" style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"><code>attributes</code></span>
/// Expands the `pyo3-asyncio` test harness call within the `main` fn
#[cfg(feature = "attributes")]
pub use pyo3_asyncio_macros::test_main_body;

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
/// ```
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

/// Abstract Test Trait
///
/// This trait works in tandem with the pyo3-asyncio-macros to generate test objects that work with
/// the pyo3-asyncio test harness.
pub trait Test: Send {
    /// Get the name of the test
    fn name(&self) -> &str;
    /// Instantiate the task that runs the test
    fn task(&self) -> Pin<Box<dyn Future<Output = PyResult<()>> + Send>>;
}

/// Run a sequence of tests while applying any necessary filtering from the `Args`
pub async fn test_harness(tests: Vec<impl Test + 'static>, args: Args) -> PyResult<()> {
    stream::iter(tests)
        .for_each_concurrent(Some(4), |test| {
            let mut ignore = false;

            if let Some(filter) = args.filter.as_ref() {
                if !test.name().contains(filter) {
                    ignore = true;
                }
            }

            async move {
                if !ignore {
                    test.task().await.unwrap();

                    println!("test {} ... ok", test.name());
                }
            }
        })
        .await;

    Ok(())
}

#[cfg(test)]
#[cfg(all(
    feature = "testing",
    feature = "attributes",
    any(feature = "async-std-runtime", feature = "tokio-runtime")
))]
mod tests {
    use pyo3::prelude::*;

    use crate as pyo3_asyncio;

    #[cfg(feature = "async-std-runtime")]
    #[pyo3_asyncio::async_std::test]
    async fn test_async_std_async_test_compiles() -> PyResult<()> {
        Ok(())
    }
    #[cfg(feature = "async-std-runtime")]
    #[pyo3_asyncio::async_std::test]
    fn test_async_std_sync_test_compiles() -> PyResult<()> {
        Ok(())
    }

    #[cfg(feature = "tokio-runtime")]
    #[pyo3_asyncio::tokio::test]
    async fn test_tokio_async_test_compiles() -> PyResult<()> {
        Ok(())
    }
    #[cfg(feature = "tokio-runtime")]
    #[pyo3_asyncio::tokio::test]
    fn test_tokio_sync_test_compiles() -> PyResult<()> {
        Ok(())
    }
}
