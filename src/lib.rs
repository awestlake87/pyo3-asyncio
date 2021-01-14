pub mod test;

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

/// Attempt to initialize the Python and Rust event loops
///
/// Must be called at the start of your program
pub fn try_init(py: Python) -> PyResult<()> {
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

pub fn get_event_loop() -> PyObject {
    EVENT_LOOP.get().unwrap().clone()
}

/// Run the event loop forever
///
/// This function must be called on the main thread
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
///
/// This function must be called on the main thread
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

pub fn close(py: Python) -> PyResult<()> {
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

        if let Err(_) = self.tx.take().unwrap().send(result) {
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

fn dump_err<'p>(py: Python<'p>) -> impl FnOnce(PyErr) + 'p {
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
