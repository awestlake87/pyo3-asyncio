use std::{future::pending, thread, time::Duration};

use lazy_static::lazy_static;
use pyo3::prelude::*;
use tokio::runtime::{Builder, Runtime};

lazy_static! {
    static ref CURRENT_THREAD_RUNTIME: Runtime = {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Couldn't build the runtime")
    };
}

fn dump_err(py: Python<'_>) -> impl FnOnce(PyErr) + '_ {
    move |e| {
        // We can't display Python exceptions via std::fmt::Display,
        // so print the error here manually.
        e.print_and_set_sys_last_vars(py);
    }
}

fn main() {
    thread::spawn(|| {
        CURRENT_THREAD_RUNTIME.block_on(pending::<()>());
    });

    Python::with_gil(|py| {
        pyo3_asyncio::with_runtime(py, || {
            CURRENT_THREAD_RUNTIME.spawn(async move {
                tokio::time::sleep(Duration::from_secs(1)).await;

                Python::with_gil(|py| {
                    let event_loop = pyo3_asyncio::get_event_loop(py);

                    event_loop
                        .call_method1(
                            "call_soon_threadsafe",
                            (event_loop.getattr("stop").map_err(dump_err(py)).unwrap(),),
                        )
                        .map_err(dump_err(py))
                        .unwrap();
                })
            });

            pyo3_asyncio::run_forever(py)?;

            println!("test test_run_forever ... ok");
            Ok(())
        })
        .map_err(dump_err(py))
        .unwrap();
    })
}
