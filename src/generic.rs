use std::future::Future;

use pyo3::{exceptions::PyException, prelude::*};

use crate::{dump_err, get_event_loop, CALL_SOON, CREATE_FUTURE, EXPECT_INIT};

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

    /// Spawn a future onto this runtime's event loop
    fn spawn<F>(fut: F) -> Self::JoinHandle
    where
        F: Future<Output = ()> + Send + 'static;
}

/// Run the event loop until the given Future completes
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
/// pyo3_asyncio::generic::run_until_complete::<MyCustomRuntime, _>(py, async move {
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
pub fn run_until_complete<R, F>(py: Python, fut: F) -> PyResult<()>
where
    R: Runtime,
    F: Future<Output = PyResult<()>> + Send + 'static,
{
    let coro = into_coroutine::<R, _>(py, async move {
        fut.await?;
        Ok(Python::with_gil(|py| py.None()))
    })?;

    get_event_loop(py).call_method1("run_until_complete", (coro,))?;

    Ok(())
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

/// Convert a Rust Future into a Python coroutine with a generic runtime
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
///
///     pyo3_asyncio::generic::into_coroutine::<MyCustomRuntime, _>(py, async move {
///         MyCustomRuntime::sleep(Duration::from_secs(secs)).await;
///         Python::with_gil(|py| Ok(py.None()))
///    })
/// }
/// ```
pub fn into_coroutine<R, F>(py: Python, fut: F) -> PyResult<PyObject>
where
    R: Runtime,
    F: Future<Output = PyResult<PyObject>> + Send + 'static,
{
    let future_rx = CREATE_FUTURE.get().expect(EXPECT_INIT).call0(py)?;
    let future_tx1 = future_rx.clone();
    let future_tx2 = future_rx.clone();

    R::spawn(async move {
        if let Err(e) = R::spawn(async move {
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

/// <span class="module-item stab portability" style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"><code>testing</code></span> Testing utilities for generic runtimes.
#[cfg(feature = "testing")]
pub mod testing {
    use pyo3::prelude::*;

    use crate::{
        dump_err,
        generic::{run_until_complete, Runtime},
        testing::{parse_args, test_harness, Test},
        with_runtime,
    };

    /// Default main function for the generic test harness.
    ///
    /// This is meant to perform the necessary initialization for most test cases. If you want
    /// additional control over the initialization, you can use this
    /// function as a template.
    pub fn test_main<R>(suite_name: &str, tests: Vec<Test>)
    where
        R: Runtime,
    {
        Python::with_gil(|py| {
            with_runtime(py, || {
                let args = parse_args(suite_name);
                run_until_complete::<R, _>(py, test_harness(tests, args))?;
                Ok(())
            })
            .map_err(dump_err(py))
            .unwrap();
        })
    }
}
