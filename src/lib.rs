#![warn(missing_docs)]

//! Rust Bindings to the Python Asyncio Event Loop
//!
//! # Motivation
//!
//! This crate aims to provide a convenient interface to manage the interop between Python and
//! Rust's async/await models. It supports conversions between Rust and Python futures and manages
//! the event loops for both languages. Python's threading model and GIL can make this interop a bit
//! trickier than one might expect, so there are a few caveats that users should be aware of.
//!
//! ## Why Two Event Loops
//!
//! Currently, we don't have a way to run Rust futures directly on Python's event loop. Likewise,
//! Python's coroutines cannot be directly spawned on a Rust event loop. The two coroutine models
//! require some additional assistance from their event loops, so in all likelihood they will need
//! a new _unique_ event loop that addresses the needs of both languages if the coroutines are to
//! be run on the same event loop.
//!
//! It's not immediately clear that this would provide worthwhile performance wins either, so in the
//! interest of keeping things simple, this crate runs both event loops independently and handles
//! the communication between them.
//!
//! ## Python's Event Loop
//!
//! Python is very picky about the threads used by the `asyncio` executor. In particular, it needs
//! to have control over the main thread in order to handle signals like CTRL-C correctly. This
//! means that Cargo's default test harness will no longer work since it doesn't provide a method of
//! overriding the main function to add our event loop initialization and finalization.
//!
//! ## Rust's Event Loop
//!
//! Currently only the Tokio runtime is supported by this crate. Tokio makes it easy to construct
//! and maintain a runtime that runs on background threads only and it provides a single threaded
//! scheduler to make it easier to work around Python's GIL.
//!
//! > _In the future, more runtimes may be supported for Rust._
//!
//! ## Features
//!
//! Items marked with
//! <span
//!   class="module-item stab portability"
//!   style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"
//! ><code>testing</code></span>
//! are only available when the `testing` Cargo feature is enabled:
//!
//! ```toml
//! [dependencies.pyo3-asyncio]
//! version = "0.1.0"
//! features = ["testing"]
//! ```

/// <span class="module-item stab portability" style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"><code>testing</code></span> Utilities for writing PyO3 Asyncio tests
#[cfg(feature = "testing")]
#[doc(inline)]
pub mod testing;

use std::{future::Future, thread};

use futures::{channel::oneshot, future};
use lazy_static::lazy_static;
use once_cell::sync::OnceCell;
use pyo3::{
    exceptions::{PyException, PyKeyboardInterrupt},
    prelude::*,
};
use tokio::{
    runtime::{Builder, Runtime},
    task::JoinHandle,
};

/// Test README
#[doc(hidden)]
pub mod doc_test {
    macro_rules! doc_comment {
        ($x:expr, $module:item) => {
            #[doc = $x]
            $module
        };
    }

    macro_rules! doctest {
        ($x:expr, $y:ident) => {
            doc_comment!(include_str!($x), mod $y {});
        };
    }

    doctest!("../README.md", readme_md);
}

lazy_static! {
    static ref CURRENT_THREAD_RUNTIME: Runtime = {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Couldn't build the runtime")
    };
}

static EVENT_LOOP: OnceCell<PyObject> = OnceCell::new();
static EXECUTOR: OnceCell<PyObject> = OnceCell::new();
static CALL_SOON: OnceCell<PyObject> = OnceCell::new();
static CREATE_TASK: OnceCell<PyObject> = OnceCell::new();
static CREATE_FUTURE: OnceCell<PyObject> = OnceCell::new();

#[allow(clippy::needless_doctest_main)]
/// Wraps the provided function with the initialization and finalization for PyO3 Asyncio
///
/// This function **_MUST_** be called from the main thread.
///
/// # Arguments
/// * `py` - The current PyO3 GIL guard
/// * `f` - The function to call in between intialization and finalization
///
/// # Examples
///
/// ```no_run
/// use pyo3::prelude::*;
///
/// fn main() {
///     Python::with_gil(|py| {
///         pyo3_asyncio::with_runtime(py, || {
///             println!("PyO3 Asyncio Initialized!");
///             Ok(())
///         })
///         .map_err(|e| {
///             e.print_and_set_sys_last_vars(py);  
///         })
///         .unwrap();
///     })
/// }
/// ```
pub fn with_runtime<F>(py: Python, f: F) -> PyResult<()>
where
    F: FnOnce() -> PyResult<()>,
{
    try_init(py)?;

    (f)()?;

    try_close(py)?;

    Ok(())
}

/// Attempt to initialize the Python and Rust event loops
///
/// Must be called at the start of your program
fn try_init(py: Python) -> PyResult<()> {
    let asyncio = py.import("asyncio")?;
    let event_loop = asyncio.call_method0("get_event_loop")?;
    let executor = py
        .import("concurrent.futures.thread")?
        .getattr("ThreadPoolExecutor")?
        .call0()?;

    event_loop.call_method1("set_default_executor", (executor,))?;

    let call_soon = event_loop.getattr("call_soon_threadsafe")?;
    let create_task = asyncio.getattr("run_coroutine_threadsafe")?;
    let create_future = event_loop.getattr("create_future")?;

    EVENT_LOOP.get_or_init(|| event_loop.into());
    EXECUTOR.get_or_init(|| executor.into());
    CALL_SOON.get_or_init(|| call_soon.into());
    CREATE_TASK.get_or_init(|| create_task.into());
    CREATE_FUTURE.get_or_init(|| create_future.into());

    thread::spawn(|| {
        CURRENT_THREAD_RUNTIME.block_on(future::pending::<()>());
    });

    Ok(())
}

/// Get a reference to the Python Event Loop from Rust
pub fn get_event_loop() -> PyObject {
    EVENT_LOOP.get().unwrap().clone()
}

/// Run the event loop forever
pub fn run_forever(py: Python) -> PyResult<()> {
    if let Err(e) = EVENT_LOOP.get().unwrap().call_method0(py, "run_forever") {
        if e.is_instance::<PyKeyboardInterrupt>(py) {
            Ok(())
        } else {
            Err(e)
        }
    } else {
        Ok(())
    }
}

/// Run the event loop until the given Future completes
pub fn run_until_complete<F>(py: Python, fut: F) -> PyResult<()>
where
    F: Future<Output = PyResult<()>> + Send + 'static,
{
    let coro = into_coroutine(py, async move {
        fut.await?;
        Ok(Python::with_gil(|py| py.None()))
    })?;

    EVENT_LOOP
        .get()
        .unwrap()
        .call_method1(py, "run_until_complete", (coro,))?;

    Ok(())
}

/// Shutdown the event loops and perform any necessary cleanup
fn try_close(py: Python) -> PyResult<()> {
    // Shutdown the executor and wait until all threads are cleaned up
    EXECUTOR.get().unwrap().call_method0(py, "shutdown")?;

    EVENT_LOOP.get().unwrap().call_method0(py, "stop")?;
    EVENT_LOOP.get().unwrap().call_method0(py, "close")?;
    Ok(())
}

/// Spawn a Future onto the executor
pub fn spawn<F>(fut: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    CURRENT_THREAD_RUNTIME.spawn(fut)
}

/// Spawn a blocking task onto the executor
pub fn spawn_blocking<F, R>(func: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    CURRENT_THREAD_RUNTIME.spawn_blocking(func)
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

        if self.tx.take().unwrap().send(result).is_err() {
            // cancellation is not an error
        }

        Ok(())
    }
}

/// Convert a Python coroutine into a Rust Future
pub fn into_future(
    py: Python,
    coro: &PyAny,
) -> PyResult<impl Future<Output = PyResult<PyObject>> + Send> {
    let (tx, rx) = oneshot::channel();

    let task = CREATE_TASK
        .get()
        .unwrap()
        .call1(py, (coro, EVENT_LOOP.get().unwrap()))?;
    let on_complete = PyTaskCompleter { tx: Some(tx) };

    task.call_method1(py, "add_done_callback", (on_complete,))?;

    Ok(async move { rx.await.unwrap() })
}

fn set_result(py: Python, future: &PyAny, result: PyResult<PyObject>) -> PyResult<()> {
    match result {
        Ok(val) => {
            let set_result = future.getattr("set_result")?;
            CALL_SOON.get().unwrap().call1(py, (set_result, val))?;
        }
        Err(err) => {
            let set_exception = future.getattr("set_exception")?;
            CALL_SOON.get().unwrap().call1(py, (set_exception, err))?;
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
pub fn into_coroutine<F>(py: Python, fut: F) -> PyResult<PyObject>
where
    F: Future<Output = PyResult<PyObject>> + Send + 'static,
{
    let future_rx = CREATE_FUTURE.get().unwrap().call0(py)?;
    let future_tx1 = future_rx.clone();
    let future_tx2 = future_rx.clone();

    spawn(async move {
        if let Err(e) = spawn(async move {
            let result = fut.await;

            Python::with_gil(move |py| {
                set_result(py, future_tx1.as_ref(py), result)
                    .map_err(dump_err(py))
                    .unwrap()
            });
        })
        .await
        {
            if e.is_panic() {
                Python::with_gil(move |py| {
                    set_result(
                        py,
                        future_tx2.as_ref(py),
                        Err(PyException::new_err("rust future panicked")),
                    )
                    .map_err(dump_err(py))
                    .unwrap()
                });
            }
        }
    });

    Ok(future_rx)
}
