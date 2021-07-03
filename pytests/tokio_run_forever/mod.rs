use std::time::Duration;

use pyo3::prelude::*;

fn dump_err(py: Python<'_>, e: PyErr) {
    // We can't display Python exceptions via std::fmt::Display,
    // so print the error here manually.
    e.print_and_set_sys_last_vars(py);
}

pub(super) fn test_main() {
    Python::with_gil(|py| {
        let event_loop = PyObject::from(pyo3_asyncio::get_event_loop(py).unwrap());

        pyo3_asyncio::tokio::get_runtime().spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;

            Python::with_gil(|py| {
                event_loop
                    .as_ref(py)
                    .call_method1(
                        "call_soon_threadsafe",
                        (event_loop
                            .as_ref(py)
                            .getattr("stop")
                            .map_err(|e| dump_err(py, e))
                            .unwrap(),),
                    )
                    .map_err(|e| dump_err(py, e))
                    .unwrap();
            })
        });

        pyo3_asyncio::run_forever(py)?;

        println!("test test_run_forever ... ok");
        Ok(())
    })
    .map_err(|e| Python::with_gil(|py| dump_err(py, e)))
    .unwrap();
}
