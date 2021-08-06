mod tokio_run_forever;

fn main() {
    pyo3::prepare_freethreaded_python();
    tokio_run_forever::test_main();
}
