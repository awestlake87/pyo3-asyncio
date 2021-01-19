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
//! pyo3-asyncio = { version = "0.13", features = ["testing", "tokio-runtime"] }
//! ```
//!
//! Now, in your test's main file, call [`crate::tokio::testing::test_main`]:
//!
//! ```no_run
//! use std::{future::pending, thread};
//!
//! use lazy_static::lazy_static;
//! use tokio::runtime::{Builder, Runtime};
//!
//! lazy_static! {
//!     static ref CURRENT_THREAD_RUNTIME: Runtime = {
//!         Builder::new_current_thread()
//!             .enable_all()
//!             .build()
//!             .expect("Couldn't build the runtime")
//!     };
//! }
//!
//! fn main() {
//!     thread::spawn(|| {
//!         CURRENT_THREAD_RUNTIME.block_on(pending::<()>());
//!     });
//!
//!     pyo3_asyncio::tokio::testing::test_main(
//!         "Example Test Suite", &CURRENT_THREAD_RUNTIME, vec![]
//!     );
//! }
//! ```
//!
//! ### Adding Tests to the PyO3 Asyncio Test Harness
//!
//! ```no_run
//! use std::{thread, future::pending, time::Duration};
//!
//! use lazy_static::lazy_static;
//! use pyo3_asyncio::testing::Test;
//! use tokio::runtime::{Builder, Runtime};
//!
//! lazy_static! {
//!     static ref CURRENT_THREAD_RUNTIME: Runtime = {
//!         Builder::new_current_thread()
//!             .enable_all()
//!             .build()
//!             .expect("Couldn't build the runtime")
//!     };
//! }
//!
//! fn main() {
//!     thread::spawn(|| {
//!         CURRENT_THREAD_RUNTIME.block_on(pending::<()>());
//!     });
//!
//!     pyo3_asyncio::tokio::testing::test_main(
//!         "Example Test Suite",
//!         &CURRENT_THREAD_RUNTIME,
//!         vec![
//!             Test::new_async(
//!                 "test_async_sleep".into(),
//!                 async move {
//!                     tokio::time::sleep(Duration::from_secs(1)).await;
//!                     Ok(())
//!                 }
//!             ),
//!             pyo3_asyncio::tokio::testing::new_sync_test("test_sync_sleep".into(), || {
//!                 thread::sleep(Duration::from_secs(1));
//!                 Ok(())
//!             })
//!         ]
//!     );
//! }
//! ```

use std::future::Future;

use ::tokio::runtime::Runtime;
use futures::channel::oneshot;
use pyo3::{exceptions::PyException, prelude::*};

use crate::{get_event_loop, CALL_SOON, CREATE_FUTURE, EXPECT_INIT};

/// Run the event loop until the given Future completes
///
/// The event loop runs until the given future is complete.
///
/// After this function returns, the event loop can be resumed with either [`run_until_complete`] or
/// [`run_forever`]
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
/// # use tokio::runtime::{Builder, Runtime};
/// #
/// # let runtime = Builder::new_current_thread()
/// #     .enable_all()
/// #     .build()
/// #     .expect("Couldn't build the runtime");
/// #
/// # Python::with_gil(|py| {
/// # pyo3_asyncio::with_runtime(py, || {
/// pyo3_asyncio::tokio::run_until_complete(py, &runtime, async move {
///     tokio::time::sleep(Duration::from_secs(1)).await;
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
pub fn run_until_complete<F>(py: Python, runtime: &Runtime, fut: F) -> PyResult<()>
where
    F: Future<Output = PyResult<()>> + Send + 'static,
{
    let coro = into_coroutine(py, runtime, async move {
        fut.await?;
        Ok(Python::with_gil(|py| py.None()))
    })?;

    get_event_loop(py).call_method1("run_until_complete", (coro,))?;

    Ok(())
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

fn set_result(py: Python, future: &PyAny, result: PyResult<PyObject>) -> PyResult<()> {
    match result {
        Ok(val) => {
            let set_result = future.getattr("set_result")?;
            CALL_SOON
                .get()
                .expect(EXPECT_INIT)
                .call1(py, (set_result, val))?;
        }
        Err(err) => {
            let set_exception = future.getattr("set_exception")?;
            CALL_SOON
                .get()
                .expect(EXPECT_INIT)
                .call1(py, (set_exception, err))?;
        }
    }

    Ok(())
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
/// use lazy_static::lazy_static;
/// use pyo3::prelude::*;
/// use tokio::runtime::{Builder, Runtime};
///
/// lazy_static! {
///     static ref CURRENT_THREAD_RUNTIME: Runtime = {
///         Builder::new_current_thread()
///             .enable_all()
///             .build()
///             .expect("Couldn't build the runtime")
///     };
/// }
///
/// /// Awaitable sleep function
/// #[pyfunction]
/// fn sleep_for(py: Python, secs: &PyAny) -> PyResult<PyObject> {
///     let secs = secs.extract()?;
///
///     pyo3_asyncio::tokio::into_coroutine(py, &CURRENT_THREAD_RUNTIME, async move {
///         tokio::time::sleep(Duration::from_secs(secs)).await;
///         Python::with_gil(|py| Ok(py.None()))
///    })
/// }
/// ```
pub fn into_coroutine<F>(py: Python, runtime: &Runtime, fut: F) -> PyResult<PyObject>
where
    F: Future<Output = PyResult<PyObject>> + Send + 'static,
{
    let future_rx = CREATE_FUTURE.get().expect(EXPECT_INIT).call0(py)?;
    let future_tx1 = future_rx.clone();
    let future_tx2 = future_rx.clone();

    runtime.spawn(async move {
        if let Err(e) = ::tokio::spawn(async move {
            let result = fut.await;

            Python::with_gil(move |py| {
                if set_result(py, future_tx1.as_ref(py), result)
                    .map_err(dump_err(py))
                    .is_err()
                {

                    // Cancelled
                }
            });
        })
        .await
        {
            if e.is_panic() {
                Python::with_gil(move |py| {
                    if set_result(
                        py,
                        future_tx2.as_ref(py),
                        Err(PyException::new_err("rust future panicked")),
                    )
                    .map_err(dump_err(py))
                    .is_err()
                    {
                        // Cancelled
                    }
                });
            }
        }
    });

    Ok(future_rx)
}

#[cfg(feature = "testing")]
pub mod testing {
    use ::tokio::runtime::Runtime;
    use pyo3::prelude::*;

    use crate::{
        testing::{parse_args, test_harness, Test},
        tokio::{dump_err, run_until_complete},
        with_runtime,
    };

    /// Construct a test from a blocking function (like the traditional `#[test]` attribute)
    pub fn new_sync_test<F>(name: String, func: F) -> Test
    where
        F: FnOnce() -> PyResult<()> + Send + 'static,
    {
        Test::new_async(name, async move {
            ::tokio::task::spawn_blocking(func).await.unwrap()
        })
    }

    /// Default main function for the test harness.
    ///
    /// This is meant to perform the necessary initialization for most test cases. If you want
    /// additional control over the initialization (i.e. env_logger initialization), you can use this
    /// function as a template.
    pub fn test_main(suite_name: &str, runtime: &Runtime, tests: Vec<Test>) {
        Python::with_gil(|py| {
            with_runtime(py, || {
                let args = parse_args(suite_name);
                run_until_complete(py, runtime, test_harness(tests, args))?;
                Ok(())
            })
            .map_err(dump_err(py))
            .unwrap();
        })
    }
}
