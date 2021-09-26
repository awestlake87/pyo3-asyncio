use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use futures::channel::oneshot;
use pin_project_lite::pin_project;
use pyo3::{prelude::*, PyNativeType};

#[allow(deprecated)]
use crate::{
    asyncio, call_soon_threadsafe, close, create_future, dump_err, err::RustPanic, get_event_loop,
    get_running_loop, into_future_with_loop,
};

/// Generic utilities for a JoinError
pub trait JoinError {
    /// Check if the spawned task exited because of a panic
    fn is_panic(&self) -> bool;
}

/// Generic Rust async/await runtime
pub trait Runtime {
    /// The error returned by a JoinHandle after being awaited
    type JoinError: JoinError + Send;
    /// A future that completes with the result of the spawned task
    type JoinHandle: Future<Output = Result<(), Self::JoinError>> + Send;

    /// Set the task local event loop for the given future
    fn scope<F, R>(event_loop: PyObject, fut: F) -> Pin<Box<dyn Future<Output = R> + Send>>
    where
        F: Future<Output = R> + Send + 'static;
    /// Get the task local event loop for the current task
    fn get_task_event_loop(py: Python) -> Option<&PyAny>;

    /// Spawn a future onto this runtime's event loop
    fn spawn<F>(fut: F) -> Self::JoinHandle
    where
        F: Future<Output = ()> + Send + 'static;
}

/// Extension trait for async/await runtimes that support spawning local tasks
pub trait SpawnLocalExt: Runtime {
    /// Set the task local event loop for the given !Send future
    fn scope_local<F, R>(event_loop: PyObject, fut: F) -> Pin<Box<dyn Future<Output = R>>>
    where
        F: Future<Output = R> + 'static;

    /// Spawn a !Send future onto this runtime's event loop
    fn spawn_local<F>(fut: F) -> Self::JoinHandle
    where
        F: Future<Output = ()> + 'static;
}

/// Get the current event loop from either Python or Rust async task local context
///
/// This function first checks if the runtime has a task-local reference to the Python event loop.
/// If not, it calls [`get_running_loop`](crate::get_running_loop`) to get the event loop associated
/// with the current OS thread.
pub fn get_current_loop<R>(py: Python) -> PyResult<&PyAny>
where
    R: Runtime,
{
    if let Some(event_loop) = R::get_task_event_loop(py) {
        Ok(event_loop)
    } else {
        get_running_loop(py)
    }
}

/// Run the event loop until the given Future completes
///
/// After this function returns, the event loop can be resumed with either [`run_until_complete`] or
/// [`run_forever`](`crate::run_forever`)
///
/// # Arguments
/// * `event_loop` - The Python event loop that should run the future
/// * `fut` - The future to drive to completion
///
/// # Examples
///
/// ```no_run
/// # use std::{task::{Context, Poll}, pin::Pin, future::Future};
/// #
/// # use pyo3_asyncio::generic::{JoinError, Runtime};
/// #
/// # struct MyCustomJoinError;
/// #
/// # impl JoinError for MyCustomJoinError {
/// #     fn is_panic(&self) -> bool {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # struct MyCustomJoinHandle;
/// #
/// # impl Future for MyCustomJoinHandle {
/// #     type Output = Result<(), MyCustomJoinError>;
/// #
/// #     fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # struct MyCustomRuntime;
/// #
/// # impl Runtime for MyCustomRuntime {
/// #     type JoinError = MyCustomJoinError;
/// #     type JoinHandle = MyCustomJoinHandle;
/// #     
/// #     fn scope<F, R>(_event_loop: PyObject, fut: F) -> Pin<Box<dyn Future<Output = R> + Send>>
/// #     where
/// #         F: Future<Output = R> + Send + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// #     fn get_task_event_loop(py: Python) -> Option<&PyAny> {
/// #         unreachable!()
/// #     }
/// #
/// #     fn spawn<F>(fut: F) -> Self::JoinHandle
/// #     where
/// #         F: Future<Output = ()> + Send + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # use std::time::Duration;
/// #
/// # use pyo3::prelude::*;
/// #
/// # Python::with_gil(|py| {
/// # pyo3_asyncio::with_runtime(py, || {
/// # let event_loop = py.import("asyncio")?.call_method0("new_event_loop")?;
/// # #[cfg(feature = "tokio-runtime")]
/// pyo3_asyncio::generic::run_until_complete::<MyCustomRuntime, _>(event_loop, async move {
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
pub fn run_until_complete<R, F>(event_loop: &PyAny, fut: F) -> PyResult<()>
where
    R: Runtime,
    F: Future<Output = PyResult<()>> + Send + 'static,
{
    let coro = future_into_py_with_loop::<R, _>(event_loop, async move {
        fut.await?;
        Ok(Python::with_gil(|py| py.None()))
    })?;

    event_loop.call_method1("run_until_complete", (coro,))?;

    Ok(())
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
/// # use std::{task::{Context, Poll}, pin::Pin, future::Future};
/// #
/// # use pyo3_asyncio::generic::{JoinError, Runtime};
/// #
/// # struct MyCustomJoinError;
/// #
/// # impl JoinError for MyCustomJoinError {
/// #     fn is_panic(&self) -> bool {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # struct MyCustomJoinHandle;
/// #
/// # impl Future for MyCustomJoinHandle {
/// #     type Output = Result<(), MyCustomJoinError>;
/// #
/// #     fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # struct MyCustomRuntime;
/// #
/// # impl Runtime for MyCustomRuntime {
/// #     type JoinError = MyCustomJoinError;
/// #     type JoinHandle = MyCustomJoinHandle;
/// #     
/// #     fn scope<F, R>(_event_loop: PyObject, fut: F) -> Pin<Box<dyn Future<Output = R> + Send>>
/// #     where
/// #         F: Future<Output = R> + Send + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// #     fn get_task_event_loop(py: Python) -> Option<&PyAny> {
/// #         unreachable!()
/// #     }
/// #
/// #     fn spawn<F>(fut: F) -> Self::JoinHandle
/// #     where
/// #         F: Future<Output = ()> + Send + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # use std::time::Duration;
/// # async fn custom_sleep(_duration: Duration) { }
/// #
/// # use pyo3::prelude::*;
/// #
/// fn main() {
///     Python::with_gil(|py| {
///         pyo3_asyncio::generic::run::<MyCustomRuntime, _>(py, async move {
///             custom_sleep(Duration::from_secs(1)).await;
///             Ok(())
///         })
///         .map_err(|e| {
///             e.print_and_set_sys_last_vars(py);  
///         })
///         .unwrap();
///     })
/// }
/// ```
pub fn run<R, F>(py: Python, fut: F) -> PyResult<()>
where
    R: Runtime,
    F: Future<Output = PyResult<()>> + Send + 'static,
{
    let event_loop = asyncio(py)?.call_method0("new_event_loop")?;

    let result = run_until_complete::<R, F>(event_loop, fut);

    close(event_loop)?;

    result
}

fn cancelled(future: &PyAny) -> PyResult<bool> {
    future.getattr("cancelled")?.call0()?.is_true()
}

fn set_result(event_loop: &PyAny, future: &PyAny, result: PyResult<PyObject>) -> PyResult<()> {
    match result {
        Ok(val) => {
            let set_result = future.getattr("set_result")?;
            call_soon_threadsafe(event_loop, (set_result, val))?;
        }
        Err(err) => {
            let set_exception = future.getattr("set_exception")?;
            call_soon_threadsafe(event_loop, (set_exception, err))?;
        }
    }

    Ok(())
}

/// Convert a Python `awaitable` into a Rust Future
///
/// This function simply forwards the future and the  `event_loop` returned by [`get_current_loop`]
/// to [`into_future_with_loop`](`crate::into_future_with_loop`). See
/// [`into_future_with_loop`](`crate::into_future_with_loop`) for more details.
///
/// # Arguments
/// * `awaitable` - The Python `awaitable` to be converted
///
/// # Examples
///
/// ```no_run
/// # use std::{pin::Pin, future::Future, task::{Context, Poll}, time::Duration};
/// #
/// # use pyo3::prelude::*;
/// #
/// # use pyo3_asyncio::generic::{JoinError, Runtime};
/// #
/// # struct MyCustomJoinError;
/// #
/// # impl JoinError for MyCustomJoinError {
/// #     fn is_panic(&self) -> bool {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # struct MyCustomJoinHandle;
/// #
/// # impl Future for MyCustomJoinHandle {
/// #     type Output = Result<(), MyCustomJoinError>;
/// #
/// #     fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # struct MyCustomRuntime;
/// #
/// # impl MyCustomRuntime {
/// #     async fn sleep(_: Duration) {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # impl Runtime for MyCustomRuntime {
/// #     type JoinError = MyCustomJoinError;
/// #     type JoinHandle = MyCustomJoinHandle;
/// #     
/// #     fn scope<F, R>(_event_loop: PyObject, fut: F) -> Pin<Box<dyn Future<Output = R> + Send>>
/// #     where
/// #         F: Future<Output = R> + Send + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// #     fn get_task_event_loop(py: Python) -> Option<&PyAny> {
/// #         unreachable!()
/// #     }
/// #
/// #     fn spawn<F>(fut: F) -> Self::JoinHandle
/// #     where
/// #         F: Future<Output = ()> + Send + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// # }
/// #
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
///         pyo3_asyncio::generic::into_future::<MyCustomRuntime>(
///             test_mod
///                 .call_method1(py, "py_sleep", (seconds.into_py(py),))?
///                 .as_ref(py),
///         )
///     })?
///     .await?;
///     Ok(())    
/// }
/// ```
pub fn into_future<R>(
    awaitable: &PyAny,
) -> PyResult<impl Future<Output = PyResult<PyObject>> + Send>
where
    R: Runtime,
{
    into_future_with_loop(get_current_loop::<R>(awaitable.py())?, awaitable)
}

/// Convert a Rust Future into a Python awaitable with a generic runtime
///
/// # Arguments
/// * `event_loop` - The Python event loop that the awaitable should be attached to
/// * `fut` - The Rust future to be converted
///
/// # Examples
///
/// ```no_run
/// # use std::{task::{Context, Poll}, pin::Pin, future::Future};
/// #
/// # use pyo3_asyncio::generic::{JoinError, Runtime};
/// #
/// # struct MyCustomJoinError;
/// #
/// # impl JoinError for MyCustomJoinError {
/// #     fn is_panic(&self) -> bool {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # struct MyCustomJoinHandle;
/// #
/// # impl Future for MyCustomJoinHandle {
/// #     type Output = Result<(), MyCustomJoinError>;
/// #
/// #     fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # struct MyCustomRuntime;
/// #
/// # impl MyCustomRuntime {
/// #     async fn sleep(_: Duration) {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # impl Runtime for MyCustomRuntime {
/// #     type JoinError = MyCustomJoinError;
/// #     type JoinHandle = MyCustomJoinHandle;
/// #     
/// #     fn scope<F, R>(_event_loop: PyObject, fut: F) -> Pin<Box<dyn Future<Output = R> + Send>>
/// #     where
/// #         F: Future<Output = R> + Send + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// #     fn get_task_event_loop(py: Python) -> Option<&PyAny> {
/// #         unreachable!()
/// #     }
/// #
/// #     fn spawn<F>(fut: F) -> Self::JoinHandle
/// #     where
/// #         F: Future<Output = ()> + Send + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// use std::time::Duration;
///
/// use pyo3::prelude::*;
///
/// /// Awaitable sleep function
/// #[pyfunction]
/// fn sleep_for<'p>(py: Python<'p>, secs: &'p PyAny) -> PyResult<&'p PyAny> {
///     let secs = secs.extract()?;
///     pyo3_asyncio::generic::future_into_py_with_loop::<MyCustomRuntime, _>(
///         pyo3_asyncio::generic::get_current_loop::<MyCustomRuntime>(py)?,
///         async move {
///             MyCustomRuntime::sleep(Duration::from_secs(secs)).await;
///             Python::with_gil(|py| Ok(py.None()))
///         }
///     )
/// }
/// ```
pub fn future_into_py_with_loop<R, F>(event_loop: &PyAny, fut: F) -> PyResult<&PyAny>
where
    R: Runtime,
    F: Future<Output = PyResult<PyObject>> + Send + 'static,
{
    let future_rx = create_future(event_loop)?;
    let future_tx1 = PyObject::from(future_rx);
    let future_tx2 = future_tx1.clone();

    let event_loop = PyObject::from(event_loop);

    R::spawn(async move {
        let event_loop2 = event_loop.clone();

        if let Err(e) = R::spawn(async move {
            let result = R::scope(event_loop2.clone(), fut).await;

            Python::with_gil(move |py| {
                if cancelled(future_tx1.as_ref(py))
                    .map_err(dump_err(py))
                    .unwrap_or(false)
                {
                    return;
                }

                let _ = set_result(event_loop2.as_ref(py), future_tx1.as_ref(py), result)
                    .map_err(dump_err(py));
            });
        })
        .await
        {
            if e.is_panic() {
                Python::with_gil(move |py| {
                    if cancelled(future_tx2.as_ref(py))
                        .map_err(dump_err(py))
                        .unwrap_or(false)
                    {
                        return;
                    }

                    let _ = set_result(
                        event_loop.as_ref(py),
                        future_tx2.as_ref(py),
                        Err(RustPanic::new_err("rust future panicked")),
                    )
                    .map_err(dump_err(py));
                });
            }
        }
    });

    Ok(future_rx)
}

pin_project! {
    /// Future returned by [`timeout`](timeout) and [`timeout_at`](timeout_at).
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    #[derive(Debug)]
    struct Cancellable<T> {
        #[pin]
        future: T,
        #[pin]
        cancel_rx: oneshot::Receiver<()>,

        poll_cancel_rx: bool
    }
}

impl<T> Cancellable<T> {
    fn new_with_cancel_rx(future: T, cancel_rx: oneshot::Receiver<()>) -> Self {
        Self {
            future,
            cancel_rx,

            poll_cancel_rx: true,
        }
    }
}

impl<T> Future for Cancellable<T>
where
    T: Future<Output = PyResult<PyObject>>,
{
    type Output = T::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        // First, try polling the future
        if let Poll::Ready(v) = this.future.poll(cx) {
            return Poll::Ready(v);
        }

        // Now check for cancellation
        if *this.poll_cancel_rx {
            match this.cancel_rx.poll(cx) {
                Poll::Ready(Ok(())) => {
                    *this.poll_cancel_rx = false;
                    // The python future has already been cancelled, so this return value will never
                    // be used.
                    Poll::Ready(Ok(Python::with_gil(|py| py.None())))
                }
                Poll::Ready(Err(_)) => {
                    *this.poll_cancel_rx = false;
                    Poll::Pending
                }
                Poll::Pending => Poll::Pending,
            }
        } else {
            Poll::Pending
        }
    }
}

#[pyclass]
struct PyDoneCallback {
    cancel_tx: Option<oneshot::Sender<()>>,
}

#[pymethods]
impl PyDoneCallback {
    #[call]
    pub fn __call__(&mut self, fut: &PyAny) -> PyResult<()> {
        let py = fut.py();

        if cancelled(fut).map_err(dump_err(py)).unwrap_or(false) {
            let _ = self.cancel_tx.take().unwrap().send(());
        }

        Ok(())
    }
}

/// Convert a Rust Future into a Python awaitable with a generic runtime
///
/// Unlike [`future_into_py_with_loop`], this function will stop the Rust future from running when
/// the `asyncio.Future` is cancelled from Python.
///
/// # Arguments
/// * `event_loop` - The Python event loop that the awaitable should be attached to
/// * `fut` - The Rust future to be converted
///
/// # Examples
///
/// ```no_run
/// # use std::{task::{Context, Poll}, pin::Pin, future::Future};
/// #
/// # use pyo3_asyncio::generic::{JoinError, Runtime};
/// #
/// # struct MyCustomJoinError;
/// #
/// # impl JoinError for MyCustomJoinError {
/// #     fn is_panic(&self) -> bool {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # struct MyCustomJoinHandle;
/// #
/// # impl Future for MyCustomJoinHandle {
/// #     type Output = Result<(), MyCustomJoinError>;
/// #
/// #     fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # struct MyCustomRuntime;
/// #
/// # impl MyCustomRuntime {
/// #     async fn sleep(_: Duration) {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # impl Runtime for MyCustomRuntime {
/// #     type JoinError = MyCustomJoinError;
/// #     type JoinHandle = MyCustomJoinHandle;
/// #     
/// #     fn scope<F, R>(_event_loop: PyObject, fut: F) -> Pin<Box<dyn Future<Output = R> + Send>>
/// #     where
/// #         F: Future<Output = R> + Send + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// #     fn get_task_event_loop(py: Python) -> Option<&PyAny> {
/// #         unreachable!()
/// #     }
/// #
/// #     fn spawn<F>(fut: F) -> Self::JoinHandle
/// #     where
/// #         F: Future<Output = ()> + Send + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// use std::time::Duration;
///
/// use pyo3::prelude::*;
///
/// /// Awaitable sleep function
/// #[pyfunction]
/// fn sleep_for<'p>(py: Python<'p>, secs: &'p PyAny) -> PyResult<&'p PyAny> {
///     let secs = secs.extract()?;
///     pyo3_asyncio::generic::cancellable_future_into_py_with_loop::<MyCustomRuntime, _>(
///         pyo3_asyncio::generic::get_current_loop::<MyCustomRuntime>(py)?,
///         async move {
///             MyCustomRuntime::sleep(Duration::from_secs(secs)).await;
///             Python::with_gil(|py| Ok(py.None()))
///         }
///     )
/// }
/// ```
pub fn cancellable_future_into_py_with_loop<R, F>(event_loop: &PyAny, fut: F) -> PyResult<&PyAny>
where
    R: Runtime,
    F: Future<Output = PyResult<PyObject>> + Send + 'static,
{
    let (cancel_tx, cancel_rx) = oneshot::channel();

    let py_fut = future_into_py_with_loop::<R, _>(
        event_loop,
        Cancellable::new_with_cancel_rx(fut, cancel_rx),
    )?;

    py_fut.call_method1(
        "add_done_callback",
        (PyDoneCallback {
            cancel_tx: Some(cancel_tx),
        },),
    )?;

    Ok(py_fut)
}

/// Convert a Rust Future into a Python awaitable with a generic runtime
///
/// # Arguments
/// * `py` - The current PyO3 GIL guard
/// * `fut` - The Rust future to be converted
///
/// # Examples
///
/// ```no_run
/// # use std::{task::{Context, Poll}, pin::Pin, future::Future};
/// #
/// # use pyo3_asyncio::generic::{JoinError, Runtime};
/// #
/// # struct MyCustomJoinError;
/// #
/// # impl JoinError for MyCustomJoinError {
/// #     fn is_panic(&self) -> bool {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # struct MyCustomJoinHandle;
/// #
/// # impl Future for MyCustomJoinHandle {
/// #     type Output = Result<(), MyCustomJoinError>;
/// #
/// #     fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # struct MyCustomRuntime;
/// #
/// # impl MyCustomRuntime {
/// #     async fn sleep(_: Duration) {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # impl Runtime for MyCustomRuntime {
/// #     type JoinError = MyCustomJoinError;
/// #     type JoinHandle = MyCustomJoinHandle;
/// #     
/// #     fn scope<F, R>(_event_loop: PyObject, fut: F) -> Pin<Box<dyn Future<Output = R> + Send>>
/// #     where
/// #         F: Future<Output = R> + Send + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// #     fn get_task_event_loop(py: Python) -> Option<&PyAny> {
/// #         unreachable!()
/// #     }
/// #
/// #     fn spawn<F>(fut: F) -> Self::JoinHandle
/// #     where
/// #         F: Future<Output = ()> + Send + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// use std::time::Duration;
///
/// use pyo3::prelude::*;
///
/// /// Awaitable sleep function
/// #[pyfunction]
/// fn sleep_for<'p>(py: Python<'p>, secs: &'p PyAny) -> PyResult<&'p PyAny> {
///     let secs = secs.extract()?;
///     pyo3_asyncio::generic::future_into_py::<MyCustomRuntime, _>(py, async move {
///         MyCustomRuntime::sleep(Duration::from_secs(secs)).await;
///         Python::with_gil(|py| Ok(py.None()))
///     })
/// }
/// ```
pub fn future_into_py<R, F>(py: Python, fut: F) -> PyResult<&PyAny>
where
    R: Runtime,
    F: Future<Output = PyResult<PyObject>> + Send + 'static,
{
    future_into_py_with_loop::<R, F>(get_current_loop::<R>(py)?, fut)
}

/// Convert a Rust Future into a Python awaitable with a generic runtime
///
/// Unlike [`future_into_py`], this function will stop the Rust future from running when the
/// `asyncio.Future` is cancelled from Python.
///
/// # Arguments
/// * `py` - The current PyO3 GIL guard
/// * `fut` - The Rust future to be converted
///
/// # Examples
///
/// ```no_run
/// # use std::{task::{Context, Poll}, pin::Pin, future::Future};
/// #
/// # use pyo3_asyncio::generic::{JoinError, Runtime};
/// #
/// # struct MyCustomJoinError;
/// #
/// # impl JoinError for MyCustomJoinError {
/// #     fn is_panic(&self) -> bool {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # struct MyCustomJoinHandle;
/// #
/// # impl Future for MyCustomJoinHandle {
/// #     type Output = Result<(), MyCustomJoinError>;
/// #
/// #     fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # struct MyCustomRuntime;
/// #
/// # impl MyCustomRuntime {
/// #     async fn sleep(_: Duration) {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # impl Runtime for MyCustomRuntime {
/// #     type JoinError = MyCustomJoinError;
/// #     type JoinHandle = MyCustomJoinHandle;
/// #     
/// #     fn scope<F, R>(_event_loop: PyObject, fut: F) -> Pin<Box<dyn Future<Output = R> + Send>>
/// #     where
/// #         F: Future<Output = R> + Send + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// #     fn get_task_event_loop(py: Python) -> Option<&PyAny> {
/// #         unreachable!()
/// #     }
/// #
/// #     fn spawn<F>(fut: F) -> Self::JoinHandle
/// #     where
/// #         F: Future<Output = ()> + Send + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// use std::time::Duration;
///
/// use pyo3::prelude::*;
///
/// /// Awaitable sleep function
/// #[pyfunction]
/// fn sleep_for<'p>(py: Python<'p>, secs: &'p PyAny) -> PyResult<&'p PyAny> {
///     let secs = secs.extract()?;
///     pyo3_asyncio::generic::cancellable_future_into_py::<MyCustomRuntime, _>(py, async move {
///         MyCustomRuntime::sleep(Duration::from_secs(secs)).await;
///         Python::with_gil(|py| Ok(py.None()))
///     })
/// }
/// ```
pub fn cancellable_future_into_py<R, F>(py: Python, fut: F) -> PyResult<&PyAny>
where
    R: Runtime,
    F: Future<Output = PyResult<PyObject>> + Send + 'static,
{
    cancellable_future_into_py_with_loop::<R, F>(get_current_loop::<R>(py)?, fut)
}

/// Convert a Rust Future into a Python awaitable with a generic runtime
///
/// # Arguments
/// * `py` - The current PyO3 GIL guard
/// * `fut` - The Rust future to be converted
///
/// # Examples
///
/// ```no_run
/// # use std::{task::{Context, Poll}, pin::Pin, future::Future};
/// #
/// # use pyo3_asyncio::generic::{JoinError, Runtime};
/// #
/// # struct MyCustomJoinError;
/// #
/// # impl JoinError for MyCustomJoinError {
/// #     fn is_panic(&self) -> bool {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # struct MyCustomJoinHandle;
/// #
/// # impl Future for MyCustomJoinHandle {
/// #     type Output = Result<(), MyCustomJoinError>;
/// #
/// #     fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # struct MyCustomRuntime;
/// #
/// # impl MyCustomRuntime {
/// #     async fn sleep(_: Duration) {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # impl Runtime for MyCustomRuntime {
/// #     type JoinError = MyCustomJoinError;
/// #     type JoinHandle = MyCustomJoinHandle;
/// #     
/// #     fn scope<F, R>(_event_loop: PyObject, fut: F) -> Pin<Box<dyn Future<Output = R> + Send>>
/// #     where
/// #         F: Future<Output = R> + Send + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// #     fn get_task_event_loop(py: Python) -> Option<&PyAny> {
/// #         unreachable!()
/// #     }
/// #
/// #     fn spawn<F>(fut: F) -> Self::JoinHandle
/// #     where
/// #         F: Future<Output = ()> + Send + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// use std::time::Duration;
///
/// use pyo3::prelude::*;
///
/// /// Awaitable sleep function
/// #[pyfunction]
/// fn sleep_for(py: Python, secs: &PyAny) -> PyResult<PyObject> {
///     let secs = secs.extract()?;
///     pyo3_asyncio::generic::into_coroutine::<MyCustomRuntime, _>(py, async move {
///         MyCustomRuntime::sleep(Duration::from_secs(secs)).await;
///         Python::with_gil(|py| Ok(py.None()))
///     })
/// }
/// ```
#[deprecated(
    since = "0.14.0",
    note = "Use the pyo3_asyncio::generic::future_into_py instead\n    (see the [migration guide](https://github.com/awestlake87/pyo3-asyncio/#migrating-from-013-to-014) for more details)"
)]
#[allow(deprecated)]
pub fn into_coroutine<R, F>(py: Python, fut: F) -> PyResult<PyObject>
where
    R: Runtime,
    F: Future<Output = PyResult<PyObject>> + Send + 'static,
{
    Ok(future_into_py_with_loop::<R, F>(get_event_loop(py), fut)?.into())
}

/// Convert a `!Send` Rust Future into a Python awaitable with a generic runtime
///
/// # Arguments
/// * `event_loop` - The Python event loop that the awaitable should be attached to
/// * `fut` - The Rust future to be converted
///
/// # Examples
///
/// ```no_run
/// # use std::{task::{Context, Poll}, pin::Pin, future::Future};
/// #
/// # use pyo3_asyncio::generic::{JoinError, SpawnLocalExt, Runtime};
/// #
/// # struct MyCustomJoinError;
/// #
/// # impl JoinError for MyCustomJoinError {
/// #     fn is_panic(&self) -> bool {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # struct MyCustomJoinHandle;
/// #
/// # impl Future for MyCustomJoinHandle {
/// #     type Output = Result<(), MyCustomJoinError>;
/// #
/// #     fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # struct MyCustomRuntime;
/// #
/// # impl MyCustomRuntime {
/// #     async fn sleep(_: Duration) {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # impl Runtime for MyCustomRuntime {
/// #     type JoinError = MyCustomJoinError;
/// #     type JoinHandle = MyCustomJoinHandle;
/// #     
/// #     fn scope<F, R>(_event_loop: PyObject, fut: F) -> Pin<Box<dyn Future<Output = R> + Send>>
/// #     where
/// #         F: Future<Output = R> + Send + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// #     fn get_task_event_loop(py: Python) -> Option<&PyAny> {
/// #         unreachable!()
/// #     }
/// #
/// #     fn spawn<F>(fut: F) -> Self::JoinHandle
/// #     where
/// #         F: Future<Output = ()> + Send + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # impl SpawnLocalExt for MyCustomRuntime {
/// #     fn scope_local<F, R>(_event_loop: PyObject, fut: F) -> Pin<Box<dyn Future<Output = R>>>
/// #     where
/// #         F: Future<Output = R> + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// #    
/// #     fn spawn_local<F>(fut: F) -> Self::JoinHandle
/// #     where
/// #         F: Future<Output = ()> + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// use std::{rc::Rc, time::Duration};
///
/// use pyo3::prelude::*;
///
/// /// Awaitable sleep function
/// #[pyfunction]
/// fn sleep_for(py: Python, secs: u64) -> PyResult<&PyAny> {
///     // Rc is !Send so it cannot be passed into pyo3_asyncio::generic::future_into_py
///     let secs = Rc::new(secs);
///
///     pyo3_asyncio::generic::local_future_into_py_with_loop::<MyCustomRuntime, _>(
///         pyo3_asyncio::get_running_loop(py)?,
///         async move {
///             MyCustomRuntime::sleep(Duration::from_secs(*secs)).await;
///             Python::with_gil(|py| Ok(py.None()))
///         }
///     )
/// }
/// ```
pub fn local_future_into_py_with_loop<R, F>(event_loop: &PyAny, fut: F) -> PyResult<&PyAny>
where
    R: SpawnLocalExt,
    F: Future<Output = PyResult<PyObject>> + 'static,
{
    let future_rx = create_future(event_loop)?;
    let future_tx1 = PyObject::from(future_rx);
    let future_tx2 = future_tx1.clone();

    let event_loop = PyObject::from(event_loop);

    R::spawn_local(async move {
        let event_loop2 = event_loop.clone();

        if let Err(e) = R::spawn_local(async move {
            let result = R::scope_local(event_loop2.clone(), fut).await;

            Python::with_gil(move |py| {
                if cancelled(future_tx1.as_ref(py))
                    .map_err(dump_err(py))
                    .unwrap_or(false)
                {
                    return;
                }

                let _ = set_result(event_loop2.as_ref(py), future_tx1.as_ref(py), result)
                    .map_err(dump_err(py));
            });
        })
        .await
        {
            if e.is_panic() {
                Python::with_gil(move |py| {
                    if cancelled(future_tx2.as_ref(py))
                        .map_err(dump_err(py))
                        .unwrap_or(false)
                    {
                        return;
                    }

                    let _ = set_result(
                        event_loop.as_ref(py),
                        future_tx2.as_ref(py),
                        Err(RustPanic::new_err("Rust future panicked")),
                    )
                    .map_err(dump_err(py));
                });
            }
        }
    });

    Ok(future_rx)
}

/// Convert a `!Send` Rust Future into a Python awaitable with a generic runtime
///
/// # Arguments
/// * `py` - The current PyO3 GIL guard
/// * `fut` - The Rust future to be converted
///
/// # Examples
///
/// ```no_run
/// # use std::{task::{Context, Poll}, pin::Pin, future::Future};
/// #
/// # use pyo3_asyncio::generic::{JoinError, SpawnLocalExt, Runtime};
/// #
/// # struct MyCustomJoinError;
/// #
/// # impl JoinError for MyCustomJoinError {
/// #     fn is_panic(&self) -> bool {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # struct MyCustomJoinHandle;
/// #
/// # impl Future for MyCustomJoinHandle {
/// #     type Output = Result<(), MyCustomJoinError>;
/// #
/// #     fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # struct MyCustomRuntime;
/// #
/// # impl MyCustomRuntime {
/// #     async fn sleep(_: Duration) {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # impl Runtime for MyCustomRuntime {
/// #     type JoinError = MyCustomJoinError;
/// #     type JoinHandle = MyCustomJoinHandle;
/// #     
/// #     fn scope<F, R>(_event_loop: PyObject, fut: F) -> Pin<Box<dyn Future<Output = R> + Send>>
/// #     where
/// #         F: Future<Output = R> + Send + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// #     fn get_task_event_loop(py: Python) -> Option<&PyAny> {
/// #         unreachable!()
/// #     }
/// #
/// #     fn spawn<F>(fut: F) -> Self::JoinHandle
/// #     where
/// #         F: Future<Output = ()> + Send + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// # impl SpawnLocalExt for MyCustomRuntime {
/// #     fn scope_local<F, R>(_event_loop: PyObject, fut: F) -> Pin<Box<dyn Future<Output = R>>>
/// #     where
/// #         F: Future<Output = R> + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// #    
/// #     fn spawn_local<F>(fut: F) -> Self::JoinHandle
/// #     where
/// #         F: Future<Output = ()> + 'static
/// #     {
/// #         unreachable!()
/// #     }
/// # }
/// #
/// use std::{rc::Rc, time::Duration};
///
/// use pyo3::prelude::*;
///
/// /// Awaitable sleep function
/// #[pyfunction]
/// fn sleep_for(py: Python, secs: u64) -> PyResult<&PyAny> {
///     // Rc is !Send so it cannot be passed into pyo3_asyncio::generic::future_into_py
///     let secs = Rc::new(secs);
///
///     pyo3_asyncio::generic::local_future_into_py::<MyCustomRuntime, _>(py, async move {
///         MyCustomRuntime::sleep(Duration::from_secs(*secs)).await;
///         Python::with_gil(|py| Ok(py.None()))
///     })
/// }
/// ```
pub fn local_future_into_py<R, F>(py: Python, fut: F) -> PyResult<&PyAny>
where
    R: SpawnLocalExt,
    F: Future<Output = PyResult<PyObject>> + 'static,
{
    local_future_into_py_with_loop::<R, F>(get_current_loop::<R>(py)?, fut)
}
