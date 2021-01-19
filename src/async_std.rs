use std::future::Future;

use async_std::task;
use futures::channel::oneshot;
use pyo3::prelude::*;

use crate::generic::{self, JoinError, Runtime};

struct AsyncStdJoinError;

impl JoinError for AsyncStdJoinError {
    fn is_panic(&self) -> bool {
        todo!()
    }
}

struct AsyncStdRuntime;

impl Runtime for AsyncStdRuntime {
    type JoinError = AsyncStdJoinError;
    type JoinHandle = task::JoinHandle<Result<(), AsyncStdJoinError>>;

    fn spawn<F>(fut: F) -> Self::JoinHandle
    where
        F: Future<Output = ()> + Send + 'static,
    {
        task::spawn(async move {
            fut.await;
            Ok(())
        })
    }
}

/// Run the event loop until the given Future completes
///
/// The event loop runs until the given future is complete.
///
/// After this function returns, the event loop can be resumed with either [`run_until_complete`] or
/// [`crate::run_forever`]
///
/// # Arguments
/// * `py` - The current PyO3 GIL guard
/// * `fut` - The future to drive to completion
///
/// # Examples
///
/// ```no_run
/// # use std::time::Duration;
/// #
/// # use pyo3::prelude::*;
/// #
/// # Python::with_gil(|py| {
/// # pyo3_asyncio::with_runtime(py, || {
/// pyo3_asyncio::async_std::run_until_complete(py, async move {
///     async_std::task::sleep(Duration::from_secs(1)).await;
///     Ok(())
/// })?;
/// # Ok(())
/// # })
/// # .map_err(|e| {
/// #    e.print_and_set_sys_last_vars(py);  
/// # })
/// # .unwrap();
/// # });
/// ```
pub fn run_until_complete<F>(py: Python, fut: F) -> PyResult<()>
where
    F: Future<Output = PyResult<()>> + Send + 'static,
{
    generic::run_until_complete::<AsyncStdRuntime, _>(py, fut)
}

#[pyclass]
struct PyTaskCompleter {
    tx: Option<oneshot::Sender<PyResult<PyObject>>>,
}

#[pymethods]
impl PyTaskCompleter {
    #[call]
    #[args(task)]
    pub fn __call__(&mut self, task: &PyAny) -> PyResult<()> {
        debug_assert!(task.call_method0("done")?.extract()?);

        let result = match task.call_method0("result") {
            Ok(val) => Ok(val.into()),
            Err(e) => Err(e),
        };

        // unclear to me whether or not this should be a panic or silent error.
        //
        // calling PyTaskCompleter twice should not be possible, but I don't think it really hurts
        // anything if it happens.
        if let Some(tx) = self.tx.take() {
            if tx.send(result).is_err() {
                // cancellation is not an error
            }
        }

        Ok(())
    }
}

fn dump_err(py: Python<'_>) -> impl FnOnce(PyErr) + '_ {
    move |e| {
        // We can't display Python exceptions via std::fmt::Display,
        // so print the error here manually.
        e.print_and_set_sys_last_vars(py);
    }
}

/// Convert a Rust Future into a Python coroutine
///
/// # Arguments
/// * `py` - The current PyO3 GIL guard
/// * `fut` - The Rust future to be converted
///
/// # Examples
///
/// ```no_run
/// use std::time::Duration;
///
/// use pyo3::prelude::*;
///
/// /// Awaitable sleep function
/// #[pyfunction]
/// fn sleep_for(py: Python, secs: &PyAny) -> PyResult<PyObject> {
///     let secs = secs.extract()?;
///
///     pyo3_asyncio::async_std::into_coroutine(py, async move {
///         tokio::time::sleep(Duration::from_secs(secs)).await;
///         Python::with_gil(|py| Ok(py.None()))
///    })
/// }
/// ```
pub fn into_coroutine<F>(py: Python, fut: F) -> PyResult<PyObject>
where
    F: Future<Output = PyResult<PyObject>> + Send + 'static,
{
    generic::into_coroutine::<AsyncStdRuntime, _>(py, fut)
}

/// <span class="module-item stab portability" style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"><code>testing</code></span> Testing Utilities for the async-std runtime.
#[cfg(feature = "testing")]
pub mod testing {
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
    //! pyo3-asyncio = { version = "0.13", features = ["testing", "async-std-runtime"] }
    //! ```
    //!
    //! Now, in your test's main file, call [`async_std::testing::test_main`]:
    //!
    //! ```no_run
    //! fn main() {
    //!     pyo3_asyncio::async_std::testing::test_main("Example Test Suite", vec![]);
    //! }
    //! ```
    //!
    //! ### Adding Tests to the PyO3 Asyncio Test Harness
    //!
    //! ```no_run
    //! use std::{time::Duration, thread};
    //!
    //! use pyo3_asyncio::testing::Test;
    //!
    //! fn main() {
    //!     pyo3_asyncio::async_std::testing::test_main(
    //!         "Example Test Suite",
    //!         vec![
    //!             Test::new_async(
    //!                 "test_async_sleep".into(),
    //!                 async move {
    //!                     async_std::task::sleep(Duration::from_secs(1)).await;
    //!                     Ok(())
    //!                 }
    //!             ),
    //!             pyo3_asyncio::async_std::testing::new_sync_test(
    //!                 "test_sync_sleep".into(),
    //!                 || {
    //!                     thread::sleep(Duration::from_secs(1));
    //!                     Ok(())
    //!                 }
    //!             )
    //!         ]
    //!     );
    //! }
    //! ```

    use async_std::task;
    use pyo3::prelude::*;

    use crate::{
        async_std::{dump_err, run_until_complete},
        testing::{parse_args, test_harness, Test},
        with_runtime,
    };

    /// Construct a test from a blocking function (like the traditional `#[test]` attribute)
    pub fn new_sync_test<F>(name: String, func: F) -> Test
    where
        F: FnOnce() -> PyResult<()> + Send + 'static,
    {
        Test::new_async(name, async move { task::spawn_blocking(func).await })
    }

    /// Default main function for the test harness.
    ///
    /// This is meant to perform the necessary initialization for most test cases. If you want
    /// additional control over the initialization (i.e. env_logger initialization), you can use this
    /// function as a template.
    pub fn test_main(suite_name: &str, tests: Vec<Test>) {
        Python::with_gil(|py| {
            with_runtime(py, || {
                let args = parse_args(suite_name);
                run_until_complete(py, test_harness(tests, args))?;
                Ok(())
            })
            .map_err(dump_err(py))
            .unwrap();
        })
    }
}
