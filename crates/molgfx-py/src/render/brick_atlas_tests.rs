use super::borrowed_bytes;
use numpy::PyReadonlyArrayDyn;
use pyo3::prelude::*;
use std::process::Command;

#[test]
fn borrowed_brick_bytes_reject_non_contiguous_numpy_views() {
    with_numpy(|py| {
        let expression = pyo3::ffi::c_str!("__import__('numpy').arange(8, dtype='uint8')[::2]");
        let value = match py.eval(expression, None, None) {
            Ok(value) => value,
            Err(error) => panic!("NumPy expression must evaluate: {error}"),
        };
        let Ok(array) = value.extract::<PyReadonlyArrayDyn<'_, u8>>() else {
            panic!("expression must return a uint8 array");
        };
        let error = borrowed_bytes(&array).expect_err("strided input must be rejected");
        assert!(error.to_string().contains("C-contiguous uint8"));
    });
}

#[test]
fn borrowed_brick_bytes_preserve_the_numpy_pointer() {
    with_numpy(|py| {
        let expression = pyo3::ffi::c_str!("__import__('numpy').arange(8, dtype='uint8')");
        let value = match py.eval(expression, None, None) {
            Ok(value) => value,
            Err(error) => panic!("NumPy expression must evaluate: {error}"),
        };
        let Ok(array) = value.extract::<PyReadonlyArrayDyn<'_, u8>>() else {
            panic!("expression must return a uint8 array");
        };
        let source = array.as_array();
        let Ok(bytes) = borrowed_bytes(&array) else {
            panic!("contiguous input must be borrowed");
        };
        assert_eq!(bytes.as_ptr(), source.as_ptr());
        assert_eq!(bytes, &[0, 1, 2, 3, 4, 5, 6, 7]);
    });
}

fn with_numpy<T>(test: impl for<'py> FnOnce(Python<'py>) -> T) -> T {
    let site_packages = numpy_site_packages().unwrap_or_else(|| {
        panic!("no interpreter on this host provides NumPy");
    });

    Python::initialize();
    Python::attach(|py| {
        let sys = py.import("sys").expect("embedded Python must import sys");
        sys.getattr("path")
            .and_then(|path| path.call_method1("insert", (0, site_packages)))
            .expect("NumPy site-packages must be added to embedded Python");
        py.import("numpy")
            .expect("embedded Python must import the discovered NumPy");
        test(py)
    })
}

/// The `NumPy` site-packages directory of the first interpreter that has it.
///
/// `python3` on `PATH` may be a virtual environment without `NumPy`, so the
/// system interpreter is tried as well before giving up.
fn numpy_site_packages() -> Option<String> {
    let probe = "import pathlib, numpy; print(pathlib.Path(numpy.__file__).parent.parent)";
    ["python3", "/usr/bin/python3", "python"]
        .into_iter()
        .find_map(|program| {
            let output = Command::new(program).args(["-c", probe]).output().ok()?;
            if !output.status.success() {
                return None;
            }
            String::from_utf8(output.stdout)
                .ok()
                .map(|text| text.trim().to_owned())
        })
}
