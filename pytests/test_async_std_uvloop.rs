use pyo3::{prelude::*, types::PyType};

#[cfg(not(target_os = "windows"))]
fn main() -> PyResult<()> {
    pyo3::prepare_freethreaded_python();

    Python::with_gil(|py| {
        let uvloop = py.import("uvloop")?;
        uvloop.call_method0("install")?;

        // store a reference for the assertion
        let uvloop = PyObject::from(uvloop);

        pyo3_asyncio::async_std::run(py, async move {
            // verify that we are on a uvloop.Loop
            Python::with_gil(|py| -> PyResult<()> {
                assert!(uvloop
                    .as_ref(py)
                    .getattr("Loop")?
                    .downcast::<PyType>()
                    .unwrap()
                    .is_instance(pyo3_asyncio::async_std::get_current_loop(py)?)?);
                Ok(())
            })?;

            async_std::task::sleep(std::time::Duration::from_secs(1)).await;

            println!("test test_async_std_uvloop ... ok");
            Ok(())
        })
    })
}

#[cfg(target_os = "windows")]
fn main() {}
