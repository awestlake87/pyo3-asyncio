mod tokio_run_forever;

fn main() {
    pyo3::prepare_freethreaded_python();
    pyo3_asyncio::tokio::init_current_thread();

    tokio_run_forever::test_main();
}
