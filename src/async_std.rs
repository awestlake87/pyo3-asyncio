use std::{any::Any, future::Future, panic::AssertUnwindSafe, pin::Pin};

use async_std::task;
use futures::prelude::*;
use once_cell::unsync::OnceCell;
use pyo3::{prelude::*, PyNativeType};

use crate::{
    generic::{self, JoinError, Runtime, SpawnLocalExt},
    into_future_with_loop,
};

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

struct AsyncStdJoinErr(Box<dyn Any + Send + 'static>);

impl JoinError for AsyncStdJoinErr {
    fn is_panic(&self) -> bool {
        true
    }
}

async_std::task_local! {
    static EVENT_LOOP: OnceCell<PyObject> = OnceCell::new()
}

struct AsyncStdRuntime;

impl Runtime for AsyncStdRuntime {
    type JoinError = AsyncStdJoinErr;
    type JoinHandle = task::JoinHandle<Result<(), AsyncStdJoinErr>>;

    fn scope<F, R>(event_loop: PyObject, fut: F) -> Pin<Box<dyn Future<Output = R> + Send>>
    where
        F: Future<Output = R> + Send + 'static,
    {
        EVENT_LOOP.with(|c| c.set(event_loop).unwrap());
        Box::pin(fut)
    }
    fn get_task_event_loop(py: Python) -> Option<&PyAny> {
        EVENT_LOOP.with(|c| c.get().map(|event_loop| event_loop.clone().into_ref(py)))
    }

    fn spawn<F>(fut: F) -> Self::JoinHandle
    where
        F: Future<Output = ()> + Send + 'static,
    {
        task::spawn(async move {
            AssertUnwindSafe(fut)
                .catch_unwind()
                .await
                .map_err(|e| AsyncStdJoinErr(e))
        })
    }
}

impl SpawnLocalExt for AsyncStdRuntime {
    fn scope_local<F, R>(event_loop: PyObject, fut: F) -> Pin<Box<dyn Future<Output = R>>>
    where
        F: Future<Output = R> + 'static,
    {
        EVENT_LOOP.with(|c| c.set(event_loop).unwrap());
        Box::pin(fut)
    }

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

/// Set the task local event loop for the given future
pub async fn scope<F, R>(event_loop: PyObject, fut: F) -> R
where
    F: Future<Output = R> + Send + 'static,
{
    AsyncStdRuntime::scope(event_loop, fut).await
}

/// Set the task local event loop for the given !Send future
pub async fn scope_local<F, R>(event_loop: PyObject, fut: F) -> R
where
    F: Future<Output = R> + 'static,
{
    AsyncStdRuntime::scope_local(event_loop, fut).await
}

/// Get the current event loop from either Python or Rust async task local context
pub fn current_event_loop(py: Python) -> PyResult<&PyAny> {
    generic::current_event_loop::<AsyncStdRuntime>(py)
}

/// Get the task local event loop for the current async_std task
pub fn task_event_loop(py: Python) -> Option<&PyAny> {
    AsyncStdRuntime::get_task_event_loop(py)
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
///     pyo3_asyncio::async_std::into_coroutine(
///         pyo3_asyncio::get_event_loop(py)?,
///         async move {
///             async_std::task::sleep(Duration::from_secs(secs)).await;
///             Python::with_gil(|py| Ok(py.None()))
///         }
///     )
/// }
/// ```
pub fn into_coroutine<F>(event_loop: &PyAny, fut: F) -> PyResult<PyObject>
where
    F: Future<Output = PyResult<PyObject>> + Send + 'static,
{
    generic::into_coroutine::<AsyncStdRuntime, _>(event_loop, fut)
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
///     // Rc is non-send so it cannot be passed into pyo3_asyncio::async_std::into_coroutine
///     let secs = Rc::new(secs);
///     Ok(pyo3_asyncio::async_std::local_future_into_py_with_loop(
///         pyo3_asyncio::async_std::current_event_loop(py)?,
///         async move {
///             async_std::task::sleep(Duration::from_secs(*secs)).await;
///             Python::with_gil(|py| Ok(py.None()))
///         }
///     )?.into())
/// }
///
/// # #[cfg(all(feature = "async-std-runtime", feature = "attributes"))]
/// #[pyo3_asyncio::async_std::main]
/// async fn main() -> PyResult<()> {
///     Python::with_gil(|py| {
///         let py_future = sleep_for(py, 1)?;
///         pyo3_asyncio::async_std::into_future(py_future)
///     })?
///     .await?;
///
///     Ok(())
/// }
/// # #[cfg(not(all(feature = "async-std-runtime", feature = "attributes")))]
/// # fn main() {}
/// ```
pub fn local_future_into_py_with_loop<F>(event_loop: &PyAny, fut: F) -> PyResult<&PyAny>
where
    F: Future<Output = PyResult<PyObject>> + 'static,
{
    generic::local_future_into_py_with_loop::<AsyncStdRuntime, _>(event_loop, fut)
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
///     // Rc is non-send so it cannot be passed into pyo3_asyncio::async_std::into_coroutine
///     let secs = Rc::new(secs);
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
///         let py_future = sleep_for(py, 1)?;
///         pyo3_asyncio::async_std::into_future(py_future)
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

/// Convert a Python `awaitable` into a Rust Future
///
/// This function converts the `awaitable` into a Python Task using `run_coroutine_threadsafe`. A
/// completion handler sends the result of this Task through a
/// `futures::channel::oneshot::Sender<PyResult<PyObject>>` and the future returned by this function
/// simply awaits the result through the `futures::channel::oneshot::Receiver<PyResult<PyObject>>`.
///
/// # Arguments
/// * `awaitable` - The Python `awaitable` to be converted
///
/// # Examples
///
/// ```
/// use std::time::Duration;
///
/// use pyo3::prelude::*;
///
/// const PYTHON_CODE: &'static str = r#"
/// import asyncio
///
/// async def py_sleep(duration):
///     await asyncio.sleep(duration)
/// "#;
///
/// async fn py_sleep(seconds: f32) -> PyResult<()> {
///     let test_mod = Python::with_gil(|py| -> PyResult<PyObject> {
///         Ok(
///             PyModule::from_code(
///                 py,
///                 PYTHON_CODE,
///                 "test_into_future/test_mod.py",
///                 "test_mod"
///             )?
///             .into()
///         )
///     })?;
///
///     Python::with_gil(|py| {
///         pyo3_asyncio::async_std::into_future(
///             test_mod
///                 .call_method1(py, "py_sleep", (seconds.into_py(py),))?
///                 .as_ref(py),
///         )
///     })?
///     .await?;
///     Ok(())    
/// }
/// ```
pub fn into_future(awaitable: &PyAny) -> PyResult<impl Future<Output = PyResult<PyObject>> + Send> {
    into_future_with_loop(current_event_loop(awaitable.py())?, awaitable)
}
