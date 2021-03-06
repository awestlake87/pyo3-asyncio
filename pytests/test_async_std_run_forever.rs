use std::time::Duration;

use pyo3::prelude::*;

fn dump_err(py: Python<'_>) -> impl FnOnce(PyErr) + '_ {
    move |e| {
        // We can't display Python exceptions via std::fmt::Display,
        // so print the error here manually.
        e.print_and_set_sys_last_vars(py);
    }
}

fn main() {
    Python::with_gil(|py| {
        pyo3_asyncio::with_runtime(py, || {
            async_std::task::spawn(async move {
                async_std::task::sleep(Duration::from_secs(1)).await;

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
