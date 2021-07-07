use std::{future::Future, pin::Pin, thread};

use ::tokio::{
    runtime::{Builder, Runtime},
    task,
};
use futures::future::pending;
use once_cell::{sync::OnceCell, unsync::OnceCell as UnsyncOnceCell};
use pyo3::{prelude::*, PyNativeType};

use crate::{
    generic::{self, Runtime as GenericRuntime, SpawnLocalExt},
    into_future_with_loop,
};

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

static TOKIO_RUNTIME: OnceCell<Runtime> = OnceCell::new();

const EXPECT_TOKIO_INIT: &str = "Tokio runtime must be initialized";

impl generic::JoinError for task::JoinError {
    fn is_panic(&self) -> bool {
        task::JoinError::is_panic(self)
    }
}

struct TokioRuntime;

tokio::task_local! {
    static EVENT_LOOP: UnsyncOnceCell<PyObject>;
}

impl GenericRuntime for TokioRuntime {
    type JoinError = task::JoinError;
    type JoinHandle = task::JoinHandle<()>;

    fn scope<F, R>(event_loop: PyObject, fut: F) -> Pin<Box<dyn Future<Output = R> + Send>>
    where
        F: Future<Output = R> + Send + 'static,
    {
        let cell = UnsyncOnceCell::new();
        cell.set(event_loop).unwrap();

        Box::pin(EVENT_LOOP.scope(cell, fut))
    }

    fn get_task_event_loop(py: Python) -> Option<&PyAny> {
        match EVENT_LOOP.try_with(|c| c.get().map(|event_loop| event_loop.clone().into_ref(py))) {
            Ok(event_loop) => event_loop,
            Err(_) => None,
        }
    }

    fn spawn<F>(fut: F) -> Self::JoinHandle
    where
        F: Future<Output = ()> + Send + 'static,
    {
        get_runtime().spawn(async move {
            fut.await;
        })
    }
}

impl SpawnLocalExt for TokioRuntime {
    fn scope_local<F, R>(event_loop: PyObject, fut: F) -> Pin<Box<dyn Future<Output = R>>>
    where
        F: Future<Output = R> + 'static,
    {
        let cell = UnsyncOnceCell::new();
        cell.set(event_loop).unwrap();

        Box::pin(EVENT_LOOP.scope(cell, fut))
    }

    fn spawn_local<F>(fut: F) -> Self::JoinHandle
    where
        F: Future<Output = ()> + 'static,
    {
        tokio::task::spawn_local(fut)
    }
}

/// Set the task local event loop for the given future
pub async fn scope<F, R>(event_loop: PyObject, fut: F) -> R
where
    F: Future<Output = R> + Send + 'static,
{
    TokioRuntime::scope(event_loop, fut).await
}

/// Set the task local event loop for the given !Send future
pub async fn scope_local<F, R>(event_loop: PyObject, fut: F) -> R
where
    F: Future<Output = R> + 'static,
{
    TokioRuntime::scope_local(event_loop, fut).await
}

/// Get the current event loop from either Python or Rust async task local context
pub fn current_event_loop(py: Python) -> PyResult<&PyAny> {
    generic::current_event_loop::<TokioRuntime>(py)
}

/// Get the task local event loop for the current tokio task
pub fn task_event_loop(py: Python) -> Option<&PyAny> {
    TokioRuntime::get_task_event_loop(py)
}

/// Initialize the Tokio Runtime with a custom build
pub fn init(runtime: Runtime) {
    TOKIO_RUNTIME
        .set(runtime)
        .expect("Tokio Runtime has already been initialized");
}

fn current_thread() -> Runtime {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Couldn't build the current-thread Tokio runtime")
}

fn start_current_thread() {
    thread::spawn(move || {
        TOKIO_RUNTIME.get().unwrap().block_on(pending::<()>());
    });
}

/// Initialize the Tokio Runtime with current-thread scheduler
///
/// # Panics
/// This function will panic if called a second time. See [`init_current_thread_once`] if you want
/// to avoid this panic.
pub fn init_current_thread() {
    init(current_thread());
    start_current_thread();
}

/// Get a reference to the current tokio runtime
pub fn get_runtime<'a>() -> &'a Runtime {
    TOKIO_RUNTIME.get().expect(EXPECT_TOKIO_INIT)
}

fn multi_thread() -> Runtime {
    Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Couldn't build the multi-thread Tokio runtime")
}

/// Initialize the Tokio Runtime with the multi-thread scheduler
///
/// # Panics
/// This function will panic if called a second time. See [`init_multi_thread_once`] if you want to
/// avoid this panic.
pub fn init_multi_thread() {
    init(multi_thread());
}

/// Ensure that the Tokio Runtime is initialized
///
/// If the runtime has not been initialized already, the multi-thread scheduler
/// is used. Calling this function a second time is a no-op.
pub fn init_multi_thread_once() {
    TOKIO_RUNTIME.get_or_init(|| multi_thread());
}

/// Ensure that the Tokio Runtime is initialized
///
/// If the runtime has not been initialized already, the current-thread
/// scheduler is used. Calling this function a second time is a no-op.
pub fn init_current_thread_once() {
    let mut initialized = false;
    TOKIO_RUNTIME.get_or_init(|| {
        initialized = true;
        current_thread()
    });

    if initialized {
        start_current_thread();
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

/// Run the event loop until the given Future completes
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
/// fn main() {
///     Python::with_gil(|py| {
///         pyo3_asyncio::tokio::run(py, async move {
///             tokio::time::sleep(Duration::from_secs(1)).await;
///             Ok(())
///         })
///         .map_err(|e| {
///             e.print_and_set_sys_last_vars(py);  
///         })
///         .unwrap();
///     })
/// }
/// ```
pub fn run<F>(py: Python, fut: F) -> PyResult<()>
where
    F: Future<Output = PyResult<()>> + Send + 'static,
{
    generic::run::<TokioRuntime, F>(py, fut)
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
///     pyo3_asyncio::tokio::into_coroutine(py, async move {
///         tokio::time::sleep(Duration::from_secs(secs)).await;
///         Python::with_gil(|py| Ok(py.None()))
///     })
/// }
/// ```
#[deprecated(
    since = "0.14.0",
    note = "Use the pyo3_asyncio::tokio::future_into_py instead"
)]
#[allow(deprecated)]
pub fn into_coroutine<F>(py: Python, fut: F) -> PyResult<PyObject>
where
    F: Future<Output = PyResult<PyObject>> + Send + 'static,
{
    generic::into_coroutine::<TokioRuntime, _>(py, fut)
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
/// fn sleep_for<'p>(py: Python<'p>, secs: &'p PyAny) -> PyResult<&'p PyAny> {
///     let secs = secs.extract()?;
///     pyo3_asyncio::tokio::future_into_py(py, async move {
///         tokio::time::sleep(Duration::from_secs(secs)).await;
///         Python::with_gil(|py| Ok(py.None()))
///     })
/// }
/// ```
pub fn future_into_py<F>(py: Python, fut: F) -> PyResult<&PyAny>
where
    F: Future<Output = PyResult<PyObject>> + Send + 'static,
{
    generic::future_into_py::<TokioRuntime, _>(py, fut)
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
///     pyo3_asyncio::tokio::local_future_into_py_with_loop(
///         pyo3_asyncio::tokio::current_event_loop(py)?,
///         async move {
///             tokio::time::sleep(Duration::from_secs(*secs)).await;
///             Python::with_gil(|py| Ok(py.None()))
///         }
///     )
/// }
///
/// # #[cfg(all(feature = "tokio-runtime", feature = "attributes"))]
/// #[pyo3_asyncio::tokio::main]
/// async fn main() -> PyResult<()> {
///     let event_loop = Python::with_gil(|py| {
///         PyObject::from(pyo3_asyncio::tokio::task_event_loop(py).unwrap())
///     });
///
///     // the main coroutine is running in a Send context, so we cannot use LocalSet here. Instead
///     // we use spawn_blocking in order to use LocalSet::block_on
///     tokio::task::spawn_blocking(move || {
///         // LocalSet allows us to work with !Send futures within tokio. Without it, any calls to
///         // pyo3_asyncio::tokio::local_future_into_py will panic.
///         tokio::task::LocalSet::new().block_on(
///             pyo3_asyncio::tokio::get_runtime(),  
///             pyo3_asyncio::tokio::scope_local(event_loop, async {
///                 Python::with_gil(|py| {
///                     let py_future = sleep_for(py, 1)?;
///                     pyo3_asyncio::tokio::into_future(py_future)
///                 })?
///                 .await?;
///
///                 Ok(())
///             })
///         )
///     }).await.unwrap()
/// }
/// # #[cfg(not(all(feature = "tokio-runtime", feature = "attributes")))]
/// # fn main() {}
/// ```
pub fn local_future_into_py_with_loop<'p, F>(event_loop: &'p PyAny, fut: F) -> PyResult<&PyAny>
where
    F: Future<Output = PyResult<PyObject>> + 'static,
{
    generic::local_future_into_py_with_loop::<TokioRuntime, _>(event_loop, fut)
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
///     pyo3_asyncio::tokio::local_future_into_py(py, async move {
///         tokio::time::sleep(Duration::from_secs(*secs)).await;
///         Python::with_gil(|py| Ok(py.None()))
///     })
/// }
///
/// # #[cfg(all(feature = "tokio-runtime", feature = "attributes"))]
/// #[pyo3_asyncio::tokio::main]
/// async fn main() -> PyResult<()> {
///     let event_loop = Python::with_gil(|py| {
///         PyObject::from(pyo3_asyncio::tokio::task_event_loop(py).unwrap())
///     });
///
///     // the main coroutine is running in a Send context, so we cannot use LocalSet here. Instead
///     // we use spawn_blocking in order to use LocalSet::block_on
///     tokio::task::spawn_blocking(move || {
///         // LocalSet allows us to work with !Send futures within tokio. Without it, any calls to
///         // pyo3_asyncio::tokio::local_future_into_py will panic.
///         tokio::task::LocalSet::new().block_on(
///             pyo3_asyncio::tokio::get_runtime(),  
///             pyo3_asyncio::tokio::scope_local(event_loop, async {
///                 Python::with_gil(|py| {
///                     let py_future = sleep_for(py, 1)?;
///                     pyo3_asyncio::tokio::into_future(py_future)
///                 })?
///                 .await?;
///
///                 Ok(())
///             })
///         )
///     }).await.unwrap()
/// }
/// # #[cfg(not(all(feature = "tokio-runtime", feature = "attributes")))]
/// # fn main() {}
/// ```
pub fn local_future_into_py<F>(py: Python, fut: F) -> PyResult<&PyAny>
where
    F: Future<Output = PyResult<PyObject>> + 'static,
{
    generic::local_future_into_py::<TokioRuntime, _>(py, fut)
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
///         pyo3_asyncio::tokio::into_future(
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
