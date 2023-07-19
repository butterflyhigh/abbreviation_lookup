#![allow(unused)]

use std::{str::FromStr, collections::HashMap};

use pyo3::{prelude::*, types::PyDict};
use tokio;
use crate as lib;

#[pyfunction]
fn search_acronym(search: String, category_name: String) -> PyResult<Vec<String>> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let category = lib::Category::from_str(&category_name).unwrap();
    match rt.block_on(lib::search_acronym(search, category)) {
        Ok(v) => {
            Ok(v)
        }
        Err(e) => {
            Err(PyErr::new::<PyAny, String>(format!("{:?}", e)))
        }
    }
}

#[pyfunction]
fn generate_training_data(num_samples: usize, output_path: String) -> PyResult<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    match rt.block_on(lib::generate_training_data(num_samples, output_path)) {
        Ok(_) => Ok(()),
        Err(e) => Err(PyErr::new::<PyAny, String>(format!("{:?}", e)))
    }
}

#[pyfunction]
fn format_data_for_mlm(py_data: Vec<HashMap<String, String>>, num_answers: usize, output_path: String) {
    let mut data = Vec::<lib::Data>::new();

    for item in py_data {
        let text = item.get("text").unwrap().to_owned();
        let abbr = item.get("abbr").unwrap().to_owned();
        let definition = item.get("definition").unwrap().to_owned();

        data.push(lib::Data {
            text,
            abbr,
            definition
        });
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(lib::format_data_for_mlm(data, num_answers, output_path));
}

#[pymodule]
fn abbreviation_lookup(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(search_acronym, m)?)?;
    m.add_function(wrap_pyfunction!(generate_training_data, m)?)?;
    m.add_function(wrap_pyfunction!(format_data_for_mlm, m)?)?;

    Ok(())
}