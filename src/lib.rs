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
//! be run on the same loop.
//!
//! It's not immediately clear that this would provide worthwhile performance wins either, so in the
//! interest of keeping things simple, this crate creates and manages the Python event loop and
//! handles the communication between separate Rust event loops.
//!
//! ## Python's Event Loop
//!
//! Python is very picky about the threads used by the `asyncio` executor. In particular, it needs
//! to have control over the main thread in order to handle signals like CTRL-C correctly. This
//! means that Cargo's default test harness will no longer work since it doesn't provide a method of
//! overriding the main function to add our event loop initialization and finalization.
//!
//! ## Event Loop References
//!
//! One problem that arises when interacting with Python's asyncio library is that the functions we use to get a reference to the Python event loop can only be called in certain contexts. Since PyO3 Asyncio needs to interact with Python's event loop during conversions, the context of these conversions can matter a lot.
//!
//! > The core conversions we've mentioned so far in this guide should insulate you from these concerns in most cases, but in the event that they don't, this section should provide you with the information you need to solve these problems.
//!
//! ### The Main Dilemma
//!
//! Python programs can have many independent event loop instances throughout the lifetime of the application (`asyncio.run` for example creates its own event loop each time it's called for instance), and they can even run concurrent with other event loops. For this reason, the most correct method of obtaining a reference to the Python event loop is via `asyncio.get_running_loop`.
//!
//! `asyncio.get_running_loop` returns the event loop associated with the current OS thread. It can be used inside Python coroutines to spawn concurrent tasks, interact with timers, or in our case signal between Rust and Python. This is all well and good when we are operating on a Python thread, but since Rust threads are not associated with a Python event loop, `asyncio.get_running_loop` will fail when called on a Rust runtime.
//!
//! ### The Solution
//!
//! A really straightforward way of dealing with this problem is to pass a reference to the associated Python event loop for every conversion. That's why in `v0.14`, we introduced a new set of conversion functions that do just that:
//!
//! - `pyo3_asyncio::into_future_with_loop` - Convert a Python awaitable into a Rust future with the given asyncio event loop.
//! - `pyo3_asyncio::<runtime>::future_into_py_with_loop` - Convert a Rust future into a Python awaitable with the given asyncio event loop.
//! - `pyo3_asyncio::<runtime>::local_future_into_py_with_loop` - Convert a `!Send` Rust future into a Python awaitable with the given asyncio event loop.
//!
//! One clear disadvantage to this approach (aside from the verbose naming) is that the Rust application has to explicitly track its references to the Python event loop. In native libraries, we can't make any assumptions about the underlying event loop, so the only reliable way to make sure our conversions work properly is to store a reference to the current event loop at the callsite to use later on.
//!
//! ```rust
//! use pyo3::{wrap_pyfunction, prelude::*};
//!
//! # #[cfg(feature = "tokio-runtime")]
//! #[pyfunction]
//! fn sleep(py: Python) -> PyResult<&PyAny> {
//!     let current_loop = pyo3_asyncio::get_running_loop(py)?;
//!     let loop_ref = PyObject::from(current_loop);
//!
//!     // Convert the async move { } block to a Python awaitable
//!     pyo3_asyncio::tokio::future_into_py_with_loop(current_loop, async move {
//!         let py_sleep = Python::with_gil(|py| {
//!             // Sometimes we need to call other async Python functions within
//!             // this future. In order for this to work, we need to track the
//!             // event loop from earlier.
//!             pyo3_asyncio::into_future_with_loop(
//!                 loop_ref.as_ref(py),
//!                 py.import("asyncio")?.call_method1("sleep", (1,))?
//!             )
//!         })?;
//!
//!         py_sleep.await?;
//!
//!         Ok(Python::with_gil(|py| py.None()))
//!     })
//! }
//!
//! # #[cfg(feature = "tokio-runtime")]
//! #[pymodule]
//! fn my_mod(py: Python, m: &PyModule) -> PyResult<()> {
//!     m.add_function(wrap_pyfunction!(sleep, m)?)?;
//!     Ok(())
//! }
//! ```
//!
//! > A naive solution to this tracking problem would be to cache a global reference to the asyncio event loop that all PyO3 Asyncio conversions can use. In fact this is what we did in PyO3 Asyncio `v0.13`. This works well for applications, but it soon became clear that this is not so ideal for libraries. Libraries usually have no direct control over how the event loop is managed, they're just expected to work with any event loop at any point in the application. This problem is compounded further when multiple event loops are used in the application since the global reference will only point to one.
//!
//! Another disadvantage to this explicit approach that is less obvious is that we can no longer call our `#[pyfunction] fn sleep` on a Rust runtime since `asyncio.get_running_loop` only works on Python threads! It's clear that we need a slightly more flexible approach.
//!
//! In order to detect the Python event loop at the callsite, we need something like `asyncio.get_running_loop` that works for _both Python and Rust_. In Python, `asyncio.get_running_loop` uses thread-local data to retrieve the event loop associated with the current thread. What we need in Rust is something that can retrieve the Python event loop associated with the current _task_.
//!
//! Enter `pyo3_asyncio::<runtime>::get_current_loop`. This function first checks task-local data for a Python event loop, then falls back on `asyncio.get_running_loop` if no task-local event loop is found. This way both bases are covered.
//!
//! Now, all we need is a way to store the event loop in task-local data. Since this is a runtime-specific feature, you can find the following functions in each runtime module:
//!
//! - `pyo3_asyncio::<runtime>::scope` - Store the event loop in task-local data when executing the given Future.
//! - `pyo3_asyncio::<runtime>::scope_local` - Store the event loop in task-local data when executing the given `!Send` Future.
//!
//! With these new functions, we can make our previous example more correct:
//!
//! ```rust no_run
//! use pyo3::prelude::*;
//!
//! # #[cfg(feature = "tokio-runtime")]
//! #[pyfunction]
//! fn sleep(py: Python) -> PyResult<&PyAny> {
//!     // get the current event loop through task-local data
//!     // OR `asyncio.get_running_loop`
//!     let locals = Python::with_gil(|py| {
//!         pyo3_asyncio::tokio::get_current_locals(py).unwrap()
//!     });
//!
//!     pyo3_asyncio::tokio::future_into_py_with_loop(
//!         locals.event_loop.clone().into_ref(py),
//!         // Store the current loop in task-local data
//!         pyo3_asyncio::tokio::scope(locals.clone(), async move {
//!             let py_sleep = Python::with_gil(|py| {
//!                 pyo3_asyncio::into_future_with_locals(
//!                     // Now we can get the current loop through task-local data
//!                     &pyo3_asyncio::tokio::get_current_locals(py)?,
//!                     py.import("asyncio")?.call_method1("sleep", (1,))?
//!                 )
//!             })?;
//!
//!             py_sleep.await?;
//!
//!             Ok(Python::with_gil(|py| py.None()))
//!         })
//!     )
//! }
//!
//! # #[cfg(feature = "tokio-runtime")]
//! #[pyfunction]
//! fn wrap_sleep(py: Python) -> PyResult<&PyAny> {
//!     // get the current event loop through task-local data
//!     // OR `asyncio.get_running_loop`
//!     let locals = pyo3_asyncio::tokio::get_current_locals(py)?;
//!
//!     pyo3_asyncio::tokio::future_into_py_with_loop(
//!         locals.event_loop.clone().into_ref(py),
//!         // Store the current loop in task-local data
//!         pyo3_asyncio::tokio::scope(locals.clone(), async move {
//!             let py_sleep = Python::with_gil(|py| {
//!                 pyo3_asyncio::into_future_with_locals(
//!                     &pyo3_asyncio::tokio::get_current_locals(py)?,
//!                     // We can also call sleep within a Rust task since the
//!                     // event loop is stored in task local data
//!                     sleep(py)?
//!                 )
//!             })?;
//!
//!             py_sleep.await?;
//!
//!             Ok(Python::with_gil(|py| py.None()))
//!         })
//!     )
//! }
//!
//! # #[cfg(feature = "tokio-runtime")]
//! #[pymodule]
//! fn my_mod(py: Python, m: &PyModule) -> PyResult<()> {
//!     m.add_function(wrap_pyfunction!(sleep, m)?)?;
//!     m.add_function(wrap_pyfunction!(wrap_sleep, m)?)?;
//!     Ok(())
//! }
//! ```
//!
//! Even though this is more correct, it's clearly not more ergonomic. That's why we introduced a new set of functions with this functionality baked in:
//!
//! - `pyo3_asyncio::<runtime>::into_future`
//!   > Convert a Python awaitable into a Rust future (using `pyo3_asyncio::<runtime>::get_current_loop`)
//! - `pyo3_asyncio::<runtime>::future_into_py`
//!   > Convert a Rust future into a Python awaitable (using `pyo3_asyncio::<runtime>::get_current_loop` and `pyo3_asyncio::<runtime>::scope` to set the task-local event loop for the given Rust future)
//! - `pyo3_asyncio::<runtime>::local_future_into_py`
//!   > Convert a `!Send` Rust future into a Python awaitable (using `pyo3_asyncio::<runtime>::get_current_loop` and `pyo3_asyncio::<runtime>::scope_local` to set the task-local event loop for the given Rust future).
//!
//! __These are the functions that we recommend using__. With these functions, the previous example can be rewritten to be more compact:
//!
//! ```rust
//! use pyo3::prelude::*;
//!
//! # #[cfg(feature = "tokio-runtime")]
//! #[pyfunction]
//! fn sleep(py: Python) -> PyResult<&PyAny> {
//!     pyo3_asyncio::tokio::future_into_py(py, async move {
//!         let py_sleep = Python::with_gil(|py| {
//!             pyo3_asyncio::tokio::into_future(
//!                 py.import("asyncio")?.call_method1("sleep", (1,))?
//!             )
//!         })?;
//!
//!         py_sleep.await?;
//!
//!         Ok(Python::with_gil(|py| py.None()))
//!     })
//! }
//!
//! # #[cfg(feature = "tokio-runtime")]
//! #[pyfunction]
//! fn wrap_sleep(py: Python) -> PyResult<&PyAny> {
//!     pyo3_asyncio::tokio::future_into_py(py, async move {
//!         let py_sleep = Python::with_gil(|py| {
//!             pyo3_asyncio::tokio::into_future(sleep(py)?)
//!         })?;
//!
//!         py_sleep.await?;
//!
//!         Ok(Python::with_gil(|py| py.None()))
//!     })
//! }
//!
//! # #[cfg(feature = "tokio-runtime")]
//! #[pymodule]
//! fn my_mod(py: Python, m: &PyModule) -> PyResult<()> {
//!     m.add_function(wrap_pyfunction!(sleep, m)?)?;
//!     m.add_function(wrap_pyfunction!(wrap_sleep, m)?)?;
//!     Ok(())
//! }
//! ```
//!
//! ## A Note for `v0.13` Users
//!
//! Hey guys, I realize that these are pretty major changes for `v0.14`, and I apologize in advance for having to modify the public API so much. I hope
//! the explanation above gives some much needed context and justification for all the breaking changes.
//!
//! Part of the reason why it's taken so long to push out a `v0.14` release is because I wanted to make sure we got this release right. There were a lot of issues with the `v0.13` release that I hadn't anticipated, and it's thanks to your feedback and patience that we've worked through these issues to get a more correct, more flexible version out there!
//!
//! This new release should address most the core issues that users have reported in the `v0.13` release, so I think we can expect more stability going forward.
//!
//! Also, a special thanks to [@ShadowJonathan](https://github.com/ShadowJonathan) for helping with the design and review
//! of these changes!
//!
//! - [@awestlake87](https://github.com/awestlake87)
//!
//! ## Rust's Event Loop
//!
//! Currently only the async-std and Tokio runtimes are supported by this crate.
//!
//! > _In the future, more runtimes may be supported for Rust._
//!
//! ## Features
//!
//! Items marked with
//! <span
//!   class="module-item stab portability"
//!   style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"
//! ><code>attributes</code></span>
//! are only available when the `attributes` Cargo feature is enabled:
//!
//! ```toml
//! [dependencies.pyo3-asyncio]
//! version = "0.13"
//! features = ["attributes"]
//! ```
//!
//! Items marked with
//! <span
//!   class="module-item stab portability"
//!   style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"
//! ><code>async-std-runtime</code></span>
//! are only available when the `async-std-runtime` Cargo feature is enabled:
//!
//! ```toml
//! [dependencies.pyo3-asyncio]
//! version = "0.13"
//! features = ["async-std-runtime"]
//! ```
//!
//! Items marked with
//! <span
//!   class="module-item stab portability"
//!   style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"
//! ><code>tokio-runtime</code></span>
//! are only available when the `tokio-runtime` Cargo feature is enabled:
//!
//! ```toml
//! [dependencies.pyo3-asyncio]
//! version = "0.13"
//! features = ["tokio-runtime"]
//! ```
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
//! version = "0.13"
//! features = ["testing"]
//! ```

/// Re-exported for #[test] attributes
#[cfg(all(feature = "attributes", feature = "testing"))]
pub use inventory;

/// <span class="module-item stab portability" style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"><code>testing</code></span> Utilities for writing PyO3 Asyncio tests
#[cfg(feature = "testing")]
pub mod testing;

/// <span class="module-item stab portability" style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"><code>async-std-runtime</code></span> PyO3 Asyncio functions specific to the async-std runtime
#[cfg(feature = "async-std")]
pub mod async_std;

/// <span class="module-item stab portability" style="display: inline; border-radius: 3px; padding: 2px; font-size: 80%; line-height: 1.2;"><code>tokio-runtime</code></span> PyO3 Asyncio functions specific to the tokio runtime
#[cfg(feature = "tokio-runtime")]
pub mod tokio;

/// Errors and exceptions related to PyO3 Asyncio
pub mod err;

/// Generic implementations of PyO3 Asyncio utilities that can be used for any Rust runtime
pub mod generic;

/// Test README
#[doc(hidden)]
pub mod doc_test {
    #[allow(unused)]
    macro_rules! doc_comment {
        ($x:expr, $module:item) => {
            #[doc = $x]
            $module
        };
    }

    #[allow(unused)]
    macro_rules! doctest {
        ($x:expr, $y:ident) => {
            doc_comment!(include_str!($x), mod $y {});
        };
    }

    #[cfg(all(
        feature = "async-std-runtime",
        feature = "tokio-runtime",
        feature = "attributes"
    ))]
    doctest!("../README.md", readme_md);
}

use std::future::Future;

use futures::channel::oneshot;
use once_cell::sync::OnceCell;
use pyo3::{
    exceptions::PyKeyboardInterrupt,
    prelude::*,
    types::{PyDict, PyTuple},
};

static ASYNCIO: OnceCell<PyObject> = OnceCell::new();
static CONTEXTVARS: OnceCell<Option<PyObject>> = OnceCell::new();
static ENSURE_FUTURE: OnceCell<PyObject> = OnceCell::new();
static GET_RUNNING_LOOP: OnceCell<PyObject> = OnceCell::new();

const EXPECT_INIT: &str = "PyO3 Asyncio has not been initialized";
static CACHED_EVENT_LOOP: OnceCell<PyObject> = OnceCell::new();
static EXECUTOR: OnceCell<PyObject> = OnceCell::new();

fn ensure_future<'p>(py: Python<'p>, awaitable: &'p PyAny) -> PyResult<&'p PyAny> {
    ENSURE_FUTURE
        .get_or_try_init(|| -> PyResult<PyObject> {
            Ok(asyncio(py)?.getattr("ensure_future")?.into())
        })?
        .as_ref(py)
        .call1((awaitable,))
}

fn create_future(event_loop: &PyAny) -> PyResult<&PyAny> {
    event_loop.call_method0("create_future")
}

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
/// ```
/// use pyo3::prelude::*;
///
/// fn main() {
///     // Call this function or use pyo3's "auto-initialize" feature
///     pyo3::prepare_freethreaded_python();
///
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
#[deprecated(
    since = "0.14.0",
    note = "Use the pyo3_asyncio::async_std::run or pyo3_asyncio::tokio::run instead\n    (see the [migration guide](https://github.com/awestlake87/pyo3-asyncio/#migrating-from-013-to-014) for more details)"
)]
#[allow(deprecated)]
pub fn with_runtime<F, R>(py: Python, f: F) -> PyResult<R>
where
    F: FnOnce() -> PyResult<R>,
{
    try_init(py)?;

    let result = (f)()?;

    try_close(py)?;

    Ok(result)
}

fn close(event_loop: &PyAny) -> PyResult<()> {
    event_loop.call_method1(
        "run_until_complete",
        (event_loop.call_method0("shutdown_asyncgens")?,),
    )?;

    // how to do this prior to 3.9?
    if event_loop.hasattr("shutdown_default_executor")? {
        event_loop.call_method1(
            "run_until_complete",
            (event_loop.call_method0("shutdown_default_executor")?,),
        )?;
    }

    event_loop.call_method0("close")?;

    Ok(())
}

/// Attempt to initialize the Python and Rust event loops
///
/// - Must be called before any other pyo3-asyncio functions.
/// - Calling `try_init` a second time returns `Ok(())` and does nothing.
///   > In future versions this may return an `Err`.
#[deprecated(
    since = "0.14.0",
    note = "see the [migration guide](https://github.com/awestlake87/pyo3-asyncio/#migrating-from-013-to-014) for more details"
)]
pub fn try_init(py: Python) -> PyResult<()> {
    CACHED_EVENT_LOOP.get_or_try_init(|| -> PyResult<PyObject> {
        let event_loop = asyncio_get_event_loop(py)?;
        let executor = py
            .import("concurrent.futures.thread")?
            .call_method0("ThreadPoolExecutor")?;
        event_loop.call_method1("set_default_executor", (executor,))?;

        EXECUTOR.set(executor.into()).unwrap();
        Ok(event_loop.into())
    })?;

    Ok(())
}

fn asyncio(py: Python) -> PyResult<&PyAny> {
    ASYNCIO
        .get_or_try_init(|| Ok(py.import("asyncio")?.into()))
        .map(|asyncio| asyncio.as_ref(py))
}

fn asyncio_get_event_loop(py: Python) -> PyResult<&PyAny> {
    asyncio(py)?.call_method0("get_event_loop")
}

/// Get a reference to the Python Event Loop from Rust
///
/// Equivalent to `asyncio.get_running_loop()` in Python 3.7+.
/// > For Python 3.6, this function falls back to `asyncio.get_event_loop()` which has slightly
/// different behaviour. See the [`asyncio.get_event_loop`](https://docs.python.org/3/library/asyncio-eventloop.html#asyncio.get_event_loop)
/// docs to better understand the differences.
pub fn get_running_loop(py: Python) -> PyResult<&PyAny> {
    // Ideally should call get_running_loop, but calls get_event_loop for compatibility when
    // get_running_loop is not available.
    GET_RUNNING_LOOP
        .get_or_try_init(|| -> PyResult<PyObject> {
            let asyncio = asyncio(py)?;

            if asyncio.hasattr("get_running_loop")? {
                // correct behaviour with Python 3.7+
                Ok(asyncio.getattr("get_running_loop")?.into())
            } else {
                // Python 3.6 compatibility mode
                Ok(asyncio.getattr("get_event_loop")?.into())
            }
        })?
        .as_ref(py)
        .call0()
}

/// Returns None only if contextvars cannot be imported (Python 3.6 fallback)
fn contextvars(py: Python) -> Option<&PyAny> {
    CONTEXTVARS
        .get_or_init(|| match py.import("contextvars") {
            Ok(contextvars) => Some(contextvars.into()),
            Err(_) => None,
        })
        .as_ref()
        .map(|contextvars| contextvars.as_ref(py))
}

/// Returns Ok(None) only if contextvars cannot be imported (Python 3.6 fallback)
fn copy_context(py: Python) -> PyResult<Option<&PyAny>> {
    if let Some(contextvars) = contextvars(py) {
        Ok(Some(contextvars.call_method0("copy_context")?))
    } else {
        Ok(None)
    }
}

/// Task-local data to store for Python conversions.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct TaskLocals {
    /// Track the event loop of the Python task
    pub event_loop: PyObject,
    /// Track the contextvars of the Python task
    pub context: PyObject,
}

impl TaskLocals {
    /// At a minimum, TaskLocals must store the event loop.
    pub fn new(event_loop: &PyAny) -> Self {
        Self {
            event_loop: event_loop.into(),
            context: event_loop.py().None(),
        }
    }

    /// Manually provide the contextvars for the current task.
    pub fn context(self, context: &PyAny) -> Self {
        Self {
            context: context.into(),
            ..self
        }
    }

    /// Capture the current task's contextvars
    pub fn copy_context(self, py: Python) -> PyResult<Self> {
        // No-op if context cannot be copied (Python 3.6 fallback)
        if let Some(cx) = copy_context(py)? {
            Ok(self.context(cx))
        } else {
            Ok(self)
        }
    }
}

/// Get a reference to the Python event loop cached by `try_init` (0.13 behaviour)
#[deprecated(
    since = "0.14.0",
    note = "see the [migration guide](https://github.com/awestlake87/pyo3-asyncio/#migrating-from-013-to-014) for more details"
)]
pub fn get_event_loop(py: Python) -> &PyAny {
    CACHED_EVENT_LOOP.get().expect(EXPECT_INIT).as_ref(py)
}

/// Run the event loop forever
///
/// This can be called instead of `run_until_complete` to run the event loop
/// until `stop` is called rather than driving a future to completion.
///
/// After this function returns, the event loop can be resumed with either `run_until_complete` or
/// [`crate::run_forever`]
///
/// # Arguments
/// * `py` - The current PyO3 GIL guard
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "async-std-runtime")]
/// fn main() -> pyo3::PyResult<()> {
///     use std::time::Duration;
///     use pyo3::prelude::*;
///     
///     // call this or use pyo3 0.14 "auto-initialize" feature
///     pyo3::prepare_freethreaded_python();
///
///     Python::with_gil(|py| {
///         pyo3_asyncio::with_runtime(py, || {
///             let event_loop_hdl = PyObject::from(pyo3_asyncio::get_event_loop(py));
///             // Wait 1 second, then stop the event loop
///             async_std::task::spawn(async move {
///                 async_std::task::sleep(Duration::from_secs(1)).await;
///                 Python::with_gil(|py| {
///                     event_loop_hdl
///                         .as_ref(py)
///                         .call_method1(
///                             "call_soon_threadsafe",
///                             (event_loop_hdl
///                                 .as_ref(py)
///                                 .getattr("stop")
///                                 .map_err(|e| e.print_and_set_sys_last_vars(py))
///                                 .unwrap(),),
///                             )
///                             .unwrap();
///                 })
///             });
///     
///             pyo3_asyncio::run_forever(py)?;
///
///             Ok(())
///         })
///     })
/// }
/// # #[cfg(not(feature = "async-std-runtime"))]
/// # fn main() {}
#[deprecated(
    since = "0.14.0",
    note = "see the [migration guide](https://github.com/awestlake87/pyo3-asyncio/#migrating-from-013-to-014) for more details"
)]
#[allow(deprecated)]
pub fn run_forever(py: Python) -> PyResult<()> {
    if let Err(e) = get_event_loop(py).call_method0("run_forever") {
        if e.is_instance::<PyKeyboardInterrupt>(py) {
            Ok(())
        } else {
            Err(e)
        }
    } else {
        Ok(())
    }
}

/// Shutdown the event loops and perform any necessary cleanup
#[deprecated(
    since = "0.14.0",
    note = "see the [migration guide](https://github.com/awestlake87/pyo3-asyncio/#migrating-from-013-to-014) for more details"
)]
pub fn try_close(py: Python) -> PyResult<()> {
    if let Some(exec) = EXECUTOR.get() {
        // Shutdown the executor and wait until all threads are cleaned up
        exec.call_method0(py, "shutdown")?;
    }

    if let Some(event_loop) = CACHED_EVENT_LOOP.get() {
        close(event_loop.as_ref(py))?;
    }

    Ok(())
}

#[pyclass]
struct PyTaskCompleter {
    tx: Option<oneshot::Sender<PyResult<PyObject>>>,
}

#[pymethods]
impl PyTaskCompleter {
    #[args(task)]
    pub fn __call__(&mut self, task: &PyAny) -> PyResult<()> {
        debug_assert!(task.call_method0("done")?.extract()?);

        let result = match task.call_method0("result") {
            Ok(val) => Ok(val.into()),
            Err(e) => Err(e),
        };

        // unclear to me whether or not this should be a panic or silent error.
        //
        // calling PyTaskCompleter twice should not be possible, but I don't think it really hurts
        // anything if it happens.
        if let Some(tx) = self.tx.take() {
            if tx.send(result).is_err() {
                // cancellation is not an error
            }
        }

        Ok(())
    }
}

#[pyclass]
struct PyEnsureFuture {
    awaitable: PyObject,
    tx: Option<oneshot::Sender<PyResult<PyObject>>>,
}

#[pymethods]
impl PyEnsureFuture {
    pub fn __call__(&mut self) -> PyResult<()> {
        Python::with_gil(|py| {
            let task = ensure_future(py, self.awaitable.as_ref(py))?;
            let on_complete = PyTaskCompleter { tx: self.tx.take() };
            task.call_method1("add_done_callback", (on_complete,))?;

            Ok(())
        })
    }
}

fn call_soon_threadsafe(
    event_loop: &PyAny,
    context: &PyAny,
    args: impl IntoPy<Py<PyTuple>>,
) -> PyResult<()> {
    let py = event_loop.py();

    let kwargs = PyDict::new(py);
    kwargs.set_item("context", context)?;

    event_loop.call_method("call_soon_threadsafe", args, Some(kwargs))?;
    Ok(())
}

/// Convert a Python `awaitable` into a Rust Future
///
/// This function converts the `awaitable` into a Python Task using `run_coroutine_threadsafe`. A
/// completion handler sends the result of this Task through a
/// `futures::channel::oneshot::Sender<PyResult<PyObject>>` and the future returned by this function
/// simply awaits the result through the `futures::channel::oneshot::Receiver<PyResult<PyObject>>`.
///
/// # Arguments
/// * `event_loop` - The Python event loop that the awaitable should be attached to
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
///         pyo3_asyncio::into_future_with_loop(
///             pyo3_asyncio::get_running_loop(py)?,
///             test_mod
///                 .call_method1(py, "py_sleep", (seconds.into_py(py),))?
///                 .as_ref(py),
///         )
///     })?
///     .await?;
///     Ok(())    
/// }
/// ```
pub fn into_future_with_locals(
    locals: &TaskLocals,
    awaitable: &PyAny,
) -> PyResult<impl Future<Output = PyResult<PyObject>> + Send> {
    let py = awaitable.py();
    let (tx, rx) = oneshot::channel();

    call_soon_threadsafe(
        locals.event_loop.as_ref(py),
        locals.context.as_ref(py),
        (PyEnsureFuture {
            awaitable: awaitable.into(),
            tx: Some(tx),
        },),
    )?;

    Ok(async move {
        match rx.await {
            Ok(item) => item,
            Err(_) => Python::with_gil(|py| {
                Err(PyErr::from_instance(
                    asyncio(py)?.call_method0("CancelledError")?,
                ))
            }),
        }
    })
}

/// Convert a Python `awaitable` into a Rust Future
///
/// This function converts the `awaitable` into a Python Task using `run_coroutine_threadsafe`. A
/// completion handler sends the result of this Task through a
/// `futures::channel::oneshot::Sender<PyResult<PyObject>>` and the future returned by this function
/// simply awaits the result through the `futures::channel::oneshot::Receiver<PyResult<PyObject>>`.
///
/// # Arguments
/// * `event_loop` - The Python event loop that the awaitable should be attached to
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
///         pyo3_asyncio::into_future_with_loop(
///             pyo3_asyncio::get_running_loop(py)?,
///             test_mod
///                 .call_method1(py, "py_sleep", (seconds.into_py(py),))?
///                 .as_ref(py),
///         )
///     })?
///     .await?;
///     Ok(())    
/// }
/// ```
pub fn into_future_with_loop(
    event_loop: &PyAny,
    awaitable: &PyAny,
) -> PyResult<impl Future<Output = PyResult<PyObject>> + Send> {
    into_future_with_locals(
        &TaskLocals::new(event_loop).copy_context(event_loop.py())?,
        awaitable,
    )
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
///         // Only works with cached event loop
///         pyo3_asyncio::into_future(
///             test_mod
///                 .call_method1(py, "py_sleep", (seconds.into_py(py),))?
///                 .as_ref(py),
///         )
///     })?
///     .await?;
///     Ok(())    
/// }
/// ```
#[deprecated(
    since = "0.14.0",
    note = "Use pyo3_asyncio::async_std::into_future or pyo3_asyncio::tokio::into_future instead\n    (see the [migration guide](https://github.com/awestlake87/pyo3-asyncio/#migrating-from-013-to-014) for more details)"
)]
#[allow(deprecated)]
pub fn into_future(awaitable: &PyAny) -> PyResult<impl Future<Output = PyResult<PyObject>> + Send> {
    into_future_with_loop(get_event_loop(awaitable.py()), awaitable)
}

fn dump_err(py: Python<'_>) -> impl FnOnce(PyErr) + '_ {
    move |e| {
        // We can't display Python exceptions via std::fmt::Display,
        // so print the error here manually.
        e.print_and_set_sys_last_vars(py);
    }
}
