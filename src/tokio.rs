use std::{
    future::{pending, Future},
    thread,
};

use ::tokio::{
    runtime::{Builder, Runtime},
    task,
};
use once_cell::sync::OnceCell;
use pyo3::prelude::*;

use crate::generic;

static TOKIO_RUNTIME: OnceCell<Runtime> = OnceCell::new();

const EXPECT_TOKIO_INIT: &str = "Tokio runtime must be initialized";

impl generic::JoinError for task::JoinError {
    fn is_panic(&self) -> bool {
        task::JoinError::is_panic(self)
    }
}

struct TokioRuntime;

impl generic::Runtime for TokioRuntime {
    type JoinError = task::JoinError;
    type JoinHandle = task::JoinHandle<()>;

    fn spawn<F>(fut: F) -> Self::JoinHandle
    where
        F: Future<Output = ()> + Send + 'static,
    {
        get_runtime().spawn(async move {
            fut.await;
        })
    }
}

/// Initialize the Tokio Runtime with a custom build
pub fn init(runtime: Runtime) {
    TOKIO_RUNTIME
        .set(runtime)
        .expect("Tokio Runtime has already been initialized");
}

/// Initialize the Tokio Runtime with current-thread scheduler
pub fn init_current_thread() {
    init(
        Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Couldn't build the current-thread Tokio runtime"),
    );

    thread::spawn(|| {
        get_runtime().block_on(pending::<()>());
    });
}

/// Get a reference to the current tokio runtime
pub fn get_runtime<'a>() -> &'a Runtime {
    TOKIO_RUNTIME.get().expect(EXPECT_TOKIO_INIT)
}

/// Initialize the Tokio Runtime with the multi-thread scheduler
pub fn init_multi_thread() {
    init(
        Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Couldn't build the current-thread Tokio runtime"),
    );
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
/// # use tokio::runtime::{Builder, Runtime};
/// #
/// # let runtime = Builder::new_current_thread()
/// #     .enable_all()
/// #     .build()
/// #     .expect("Couldn't build the runtime");
/// #
/// # Python::with_gil(|py| {
/// # pyo3_asyncio::with_runtime(py, || {
/// # pyo3_asyncio::tokio::init_current_thread();
/// pyo3_asyncio::tokio::run_until_complete(py, async move {
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
pub fn run_until_complete<F>(py: Python, fut: F) -> PyResult<()>
where
    F: Future<Output = PyResult<()>> + Send + 'static,
{
    generic::run_until_complete::<TokioRuntime, _>(py, fut)
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
///     pyo3_asyncio::tokio::into_coroutine(py, async move {
///         tokio::time::sleep(Duration::from_secs(secs)).await;
///         Python::with_gil(|py| Ok(py.None()))
///    })
/// }
/// ```
pub fn into_coroutine<F>(py: Python, fut: F) -> PyResult<PyObject>
where
    F: Future<Output = PyResult<PyObject>> + Send + 'static,
{
    generic::into_coroutine::<TokioRuntime, _>(py, fut)
}

/// <span class="module-item stab portability" style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"><code>testing</code></span> Testing Utilities for the Tokio runtime.
///
// # PyO3 Asyncio Testing Utilities
///
/// This module provides some utilities for parsing test arguments as well as running and filtering
/// a sequence of tests.
///
/// As mentioned [here](crate#pythons-event-loop), PyO3 Asyncio tests cannot use the default test
/// harness since it doesn't allow Python to gain control over the main thread. Instead, we have to
/// provide our own test harness in order to create integration tests.
///
/// ## Creating A PyO3 Asyncio Integration Test
///
/// ### Main Test File
/// First, we need to create the test's main file. Although these tests are considered integration
/// tests, we cannot put them in the `tests` directory since that is a special directory owned by
/// Cargo. Instead, we put our tests in a `pytests` directory, although the name `pytests` is just
/// a convention.
///
/// `pytests/test_example.rs`
/// ```no_run
/// fn main() {
///
/// }
/// ```
///
/// ### Test Manifest Entry
/// Next, we need to add our test file to the Cargo manifest. Add the following section to your
/// `Cargo.toml`
///
/// ```toml
/// [[test]]
/// name = "test_example"
/// path = "pytests/test_example.rs"
/// harness = false
/// ```
///
/// At this point you should be able to run the test via `cargo test`
///
/// ### Using the PyO3 Asyncio Test Harness
/// Now that we've got our test registered with `cargo test`, we can start using the PyO3 Asyncio
/// test harness.
///
/// In your `Cargo.toml` add the testing feature to `pyo3-asyncio`:
/// ```toml
/// pyo3-asyncio = { version = "0.13", features = ["testing", "tokio-runtime"] }
/// ```
///
/// Now, in your test's main file, initialize the tokio runtime and call
/// [`crate::tokio::testing::test_main`]:
///
/// ```no_run
/// fn main() {
///     pyo3_asyncio::tokio::init_current_thread();
///
///     pyo3_asyncio::tokio::testing::test_main("Example Test Suite", vec![]);
/// }
/// ```
///
/// ### Adding Tests to the PyO3 Asyncio Test Harness
///
/// ```no_run
/// use std::{thread, time::Duration};
///
/// use pyo3_asyncio::testing::Test;
///
/// fn main() {
///     pyo3_asyncio::tokio::init_current_thread();
///
///     pyo3_asyncio::tokio::testing::test_main(
///         "Example Test Suite",
///         vec![
///             Test::new_async(
///                 "test_async_sleep".into(),
///                 async move {
///                     tokio::time::sleep(Duration::from_secs(1)).await;
///                     Ok(())
///                 }
///             ),
///             pyo3_asyncio::tokio::testing::new_sync_test("test_sync_sleep".into(), || {
///                 thread::sleep(Duration::from_secs(1));
///                 Ok(())
///             })
///         ]
///     );
/// }
/// ```
#[cfg(feature = "testing")]
pub mod testing {
    use pyo3::prelude::*;

    use crate::{
        dump_err,
        testing::{parse_args, test_harness, Test},
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
    ///
    /// > _The tokio runtime must be initialized before calling this function!_
    pub fn test_main(suite_name: &str, tests: Vec<Test>) {
        Python::with_gil(|py| {
            with_runtime(py, || {
                let args = parse_args(suite_name);
                crate::tokio::run_until_complete(py, test_harness(tests, args))?;

                Ok(())
            })
            .map_err(dump_err(py))
            .unwrap();
        })
    }
}
