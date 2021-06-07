use std::future::Future;

use async_std::task;
use pyo3::prelude::*;

use crate::generic::{self, JoinError, Runtime, SpawnLocalExt};

/// <span class="module-item stab portability" style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"><code>attributes</code></span>
/// re-exports for macros
#[cfg(feature = "attributes")]
pub mod re_exports {
    /// re-export spawn_blocking for use in `#[test]` macro without external dependency
    pub use async_std::task::spawn_blocking;
}

/// <span class="module-item stab portability" style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"><code>attributes</code></span> Provides the boilerplate for the `async-std` runtime and runs an async fn as main
#[cfg(feature = "attributes")]
pub use pyo3_asyncio_macros::async_std_main as main;

/// <span class="module-item stab portability" style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"><code>attributes</code></span>
/// <span class="module-item stab portability" style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"><code>testing</code></span>
/// Registers an `async-std` test with the `pyo3-asyncio` test harness
#[cfg(all(feature = "attributes", feature = "testing"))]
pub use pyo3_asyncio_macros::async_std_test as test;

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

impl SpawnLocalExt for AsyncStdRuntime {
    fn spawn_local<F>(fut: F) -> Self::JoinHandle
    where
        F: Future<Output = ()> + 'static,
    {
        task::spawn_local(async move {
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
/// ```
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

/// Convert a Rust Future into a Python awaitable
///
/// # Arguments
/// * `py` - The current PyO3 GIL guard
/// * `fut` - The Rust future to be converted
///
/// # Examples
///
/// ```
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
///         async_std::task::sleep(Duration::from_secs(secs)).await;
///         Python::with_gil(|py| Ok(py.None()))
///     })
/// }
/// ```
pub fn into_coroutine<F>(py: Python, fut: F) -> PyResult<PyObject>
where
    F: Future<Output = PyResult<PyObject>> + Send + 'static,
{
    generic::into_coroutine::<AsyncStdRuntime, _>(py, fut)
}

/// Convert a `!Send` Rust Future into a Python awaitable
///
/// # Arguments
/// * `py` - The current PyO3 GIL guard
/// * `fut` - The Rust future to be converted
///
/// # Examples
///
/// ```
/// use std::{rc::Rc, time::Duration};
///
/// use pyo3::prelude::*;
///
/// /// Awaitable non-send sleep function
/// #[pyfunction]
/// fn sleep_for(py: Python, secs: u64) -> PyResult<&PyAny> {
///     // Rc is non-send so it cannot be passed into pyo3_asyncio::tokio::into_coroutine
///     let secs = Rc::new(secs);
///
///     pyo3_asyncio::async_std::local_future_into_py(py, async move {
///         async_std::task::sleep(Duration::from_secs(*secs)).await;
///         Python::with_gil(|py| Ok(py.None()))
///     })
/// }
///
/// # #[cfg(all(feature = "async-std-runtime", feature = "attributes"))]
/// #[pyo3_asyncio::async_std::main]
/// async fn main() -> PyResult<()> {
///     Python::with_gil(|py| {
///        let py_future = sleep_for(py, 1)?;
///        pyo3_asyncio::into_future(py_future)
///     })?
///     .await?;
///
///     Ok(())
/// }
/// # #[cfg(not(all(feature = "async-std-runtime", feature = "attributes")))]
/// # fn main() {}
/// ```
pub fn local_future_into_py<F>(py: Python, fut: F) -> PyResult<&PyAny>
where
    F: Future<Output = PyResult<PyObject>> + 'static,
{
    generic::local_future_into_py::<AsyncStdRuntime, _>(py, fut)
}
