//! <span class="module-item stab portability" style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"><code>tokio-runtime</code></span> PyO3 Asyncio functions specific to the tokio runtime
//!
//! Items marked with
//! <span
//!   class="module-item stab portability"
//!   style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"
//! ><code>unstable-streams</code></span>
//! are only available when the `unstable-streams` Cargo feature is enabled:
//!
//! ```toml
//! [dependencies.pyo3-asyncio]
//! version = "0.20"
//! features = ["unstable-streams"]
//! ```

use std::{
    future::Future,
    sync::{Arc, Mutex},
};

use pyo3::prelude::*;

use crate::{asyncio, close, ensure_future, get_running_loop};

/// Run the event loop until the given Future completes
///
/// The event loop runs until the given future is complete.
///
/// After this function returns, the event loop can be resumed with [`run_until_complete`]
///
/// # Arguments
/// * `event_loop` - The Python event loop that should run the future
/// * `fut` - The future to drive to completion
pub fn run_until_complete<F, T>(event_loop: &PyAny, fut: F) -> PyResult<T>
where
    F: Future<Output = PyResult<T>> + Send + 'static,
    T: Send + Sync + 'static,
{
    let py = event_loop.py();
    let result_tx = Arc::new(Mutex::new(None));
    let result_rx = Arc::clone(&result_tx);

    let coro = future_into_py::<_, ()>(py, async move {
        let val = fut.await?;
        if let Ok(mut result) = result_tx.lock() {
            *result = Some(val);
        }
        Ok(())
    })?;

    event_loop.call_method1(pyo3::intern!(py, "run_until_complete"), (coro,))?;

    let result = result_rx.lock().unwrap().take().unwrap();
    Ok(result)
}

/// Run the event loop until the given Future completes
///
/// # Arguments
/// * `py` - The current PyO3 GIL guard
/// * `fut` - The future to drive to completion
pub fn run<F, T>(py: Python, fut: F) -> PyResult<T>
where
    F: Future<Output = PyResult<T>> + Send + 'static,
    T: Send + Sync + 'static,
{
    let event_loop = asyncio(py)?.call_method0(pyo3::intern!(py, "new_event_loop"))?;
    asyncio(py)?.call_method1("set_event_loop", (event_loop,))?;

    let result = run_until_complete::<F, T>(event_loop, fut);

    close(event_loop)?;

    result
}

/// Convert a Rust Future into a Python awaitable
///
/// If the `asyncio.Future` returned by this conversion is cancelled via `asyncio.Future.cancel`,
/// the Rust future will be cancelled as well (new behaviour in `v0.15`).
///
/// Python `contextvars` are preserved when calling async Python functions within the Rust future
/// via [`into_future`] (new behaviour in `v0.15`).
///
/// > Although `contextvars` are preserved for async Python functions, synchronous functions will
/// unfortunately fail to resolve them when called within the Rust future. This is because the
/// function is being called from a Rust thread, not inside an actual Python coroutine context.
/// >
/// > As a workaround, you can get the `contextvars` from the current task locals using
/// [`get_current_locals`] and [`TaskLocals::context`](`crate::TaskLocals::context`), then wrap your
/// synchronous function in a call to `contextvars.Context.run`. This will set the context, call the
/// synchronous function, and restore the previous context when it returns or raises an exception.
///
/// # Arguments
/// * `py` - The current PyO3 GIL guard
/// * `fut` - The Rust future to be converted
pub fn future_into_py<F, T>(py: Python, fut: F) -> PyResult<&PyAny>
where
    F: Future<Output = PyResult<T>> + Send + 'static,
    T: Send + IntoPy<PyObject>,
{
    Ok(pyo3_async::asyncio::Coroutine::from_future(fut)
        .into_py(py)
        .into_ref(py))
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
pub fn into_future(awaitable: &PyAny) -> PyResult<impl Future<Output = PyResult<PyObject>> + Send> {
    let future =
        pyo3_async::asyncio::FutureWrapper::new(ensure_future(awaitable.py(), awaitable)?, None);
    Ok(async move { future.await })
}

/// Get the current event loop from Python
pub fn get_current_loop(py: Python) -> PyResult<&PyAny> {
    get_running_loop(py)
}
