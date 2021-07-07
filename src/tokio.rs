use std::{future::Future, sync::Mutex};

use ::tokio::{
    runtime::{Builder, Runtime},
    task,
};
use once_cell::sync::{Lazy, OnceCell};
use pyo3::prelude::*;

use crate::generic;

/// <span class="module-item stab portability" style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"><code>attributes</code></span>
/// re-exports for macros
#[cfg(feature = "attributes")]
pub mod re_exports {
    /// re-export pending to be used in tokio macros without additional dependency
    pub use futures::future::pending;
    /// re-export tokio::runtime to build runtimes in tokio macros without additional dependency
    pub use tokio::runtime;
}

/// <span class="module-item stab portability" style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"><code>attributes</code></span>
#[cfg(feature = "attributes")]
pub use pyo3_asyncio_macros::tokio_main as main;

/// <span class="module-item stab portability" style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"><code>attributes</code></span>
/// <span class="module-item stab portability" style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"><code>testing</code></span>
/// Registers a `tokio` test with the `pyo3-asyncio` test harness
#[cfg(all(feature = "attributes", feature = "testing"))]
pub use pyo3_asyncio_macros::tokio_test as test;

static TOKIO_BUILDER: Lazy<Mutex<Builder>> = Lazy::new(|| Mutex::new(multi_thread()));
static TOKIO_RUNTIME: OnceCell<Runtime> = OnceCell::new();

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

impl generic::SpawnLocalExt for TokioRuntime {
    fn spawn_local<F>(fut: F) -> Self::JoinHandle
    where
        F: Future<Output = ()> + 'static,
    {
        tokio::task::spawn_local(fut)
    }
}

/// Initialize the Tokio runtime with a custom build
pub fn init(builder: Builder) {
    *TOKIO_BUILDER.lock().unwrap() = builder
}

/// Get a reference to the current tokio runtime
pub fn get_runtime<'a>() -> &'a Runtime {
    TOKIO_RUNTIME.get_or_init(|| {
        TOKIO_BUILDER
            .lock()
            .unwrap()
            .build()
            .expect("Unable to build Tokio runtime")
    })
}

fn multi_thread() -> Builder {
    let mut builder = Builder::new_multi_thread();
    builder.enable_all();
    builder
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
///     pyo3_asyncio::tokio::into_coroutine(py, async move {
///         tokio::time::sleep(Duration::from_secs(secs)).await;
///         Python::with_gil(|py| Ok(py.None()))
///     })
/// }
/// ```
pub fn into_coroutine<F>(py: Python, fut: F) -> PyResult<PyObject>
where
    F: Future<Output = PyResult<PyObject>> + Send + 'static,
{
    generic::into_coroutine::<TokioRuntime, _>(py, fut)
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
///     pyo3_asyncio::tokio::local_future_into_py(py, async move {
///         tokio::time::sleep(Duration::from_secs(*secs)).await;
///         Python::with_gil(|py| Ok(py.None()))
///     })
/// }
///
/// # #[cfg(all(feature = "tokio-runtime", feature = "attributes"))]
/// #[pyo3_asyncio::tokio::main]
/// async fn main() -> PyResult<()> {
///     // the main coroutine is running in a Send context, so we cannot use LocalSet here. Instead
///     // we use spawn_blocking in order to use LocalSet::block_on
///     tokio::task::spawn_blocking(|| {
///         // LocalSet allows us to work with !Send futures within tokio. Without it, any calls to
///         // pyo3_asyncio::tokio::local_future_into_py will panic.
///         tokio::task::LocalSet::new().block_on(pyo3_asyncio::tokio::get_runtime(), async {
///             Python::with_gil(|py| {
///                let py_future = sleep_for(py, 1)?;
///                pyo3_asyncio::into_future(py_future)
///             })?
///             .await?;
///
///             Ok(())
///         })
///     }).await.unwrap()
/// }
/// # #[cfg(not(all(feature = "tokio-runtime", feature = "attributes")))]
/// # fn main() {}
/// ```
pub fn local_future_into_py<'p, F>(py: Python<'p>, fut: F) -> PyResult<&'p PyAny>
where
    F: Future<Output = PyResult<PyObject>> + 'static,
{
    generic::local_future_into_py::<TokioRuntime, _>(py, fut)
}
