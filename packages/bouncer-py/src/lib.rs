use std::time::{SystemTime, UNIX_EPOCH};

use bouncer_core as core;
use pyo3::exceptions::{PyRuntimeError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBool, PyBytes, PyDict, PyList};
use rusqlite::types::Value;
use rusqlite::{Connection, params_from_iter};

fn py_err<E: std::fmt::Display>(err: E) -> PyErr {
    PyRuntimeError::new_err(err.to_string())
}

fn system_now_ms() -> PyResult<i64> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(py_err)?
        .as_millis();
    i64::try_from(millis).map_err(|_| py_err(format!("system time {millis}ms does not fit in i64")))
}

fn py_to_value(item: &Bound<'_, PyAny>) -> PyResult<Value> {
    if item.is_none() {
        return Ok(Value::Null);
    }
    if let Ok(value) = item.cast::<PyBool>() {
        return Ok(Value::Integer(if value.is_true() { 1 } else { 0 }));
    }
    if let Ok(value) = item.cast::<PyBytes>() {
        return Ok(Value::Blob(value.as_bytes().to_vec()));
    }
    if let Ok(value) = item.extract::<i64>() {
        return Ok(Value::Integer(value));
    }
    if let Ok(value) = item.extract::<f64>() {
        return Ok(Value::Real(value));
    }
    if let Ok(value) = item.extract::<String>() {
        return Ok(Value::Text(value));
    }

    let type_name = item
        .get_type()
        .name()
        .map(|name| name.to_string())
        .unwrap_or_else(|_| "<unknown>".to_string());
    Err(PyTypeError::new_err(format!(
        "unsupported SQL parameter type: {type_name}"
    )))
}

fn build_params(params: Option<&Bound<'_, PyList>>) -> PyResult<Vec<Value>> {
    let mut values = Vec::new();
    if let Some(params) = params {
        for item in params.iter() {
            values.push(py_to_value(&item)?);
        }
    }
    Ok(values)
}

fn lease_dict(py: Python<'_>, lease: &core::LeaseInfo) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("name", &lease.name)?;
    dict.set_item("owner", &lease.owner)?;
    dict.set_item("token", lease.token)?;
    dict.set_item("lease_expires_at_ms", lease.lease_expires_at_ms)?;
    Ok(dict.unbind())
}

fn optional_lease_dict(
    py: Python<'_>,
    lease: Option<&core::LeaseInfo>,
) -> PyResult<Option<Py<PyDict>>> {
    lease.map(|lease| lease_dict(py, lease)).transpose()
}

fn claim_dict(py: Python<'_>, result: core::ClaimResult) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    match result {
        core::ClaimResult::Acquired(lease) => {
            dict.set_item("acquired", true)?;
            dict.set_item("lease", lease_dict(py, &lease)?)?;
            dict.set_item("current", py.None())?;
        }
        core::ClaimResult::Busy(current) => {
            dict.set_item("acquired", false)?;
            dict.set_item("lease", py.None())?;
            dict.set_item("current", lease_dict(py, &current)?)?;
        }
    }
    Ok(dict.unbind())
}

fn renew_dict(py: Python<'_>, result: core::RenewResult) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    match result {
        core::RenewResult::Renewed(lease) => {
            dict.set_item("renewed", true)?;
            dict.set_item("lease", lease_dict(py, &lease)?)?;
            dict.set_item("current", py.None())?;
        }
        core::RenewResult::Rejected { current } => {
            dict.set_item("renewed", false)?;
            dict.set_item("lease", py.None())?;
            dict.set_item("current", optional_lease_dict(py, current.as_ref())?)?;
        }
    }
    Ok(dict.unbind())
}

fn release_dict(
    py: Python<'_>,
    requested_name: &str,
    result: core::ReleaseResult,
) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    match result {
        core::ReleaseResult::Released { name, token } => {
            dict.set_item("released", true)?;
            dict.set_item("name", name)?;
            dict.set_item("token", token)?;
            dict.set_item("current", py.None())?;
        }
        core::ReleaseResult::Rejected { current } => {
            dict.set_item("released", false)?;
            dict.set_item("name", requested_name)?;
            dict.set_item("token", py.None())?;
            dict.set_item("current", optional_lease_dict(py, current.as_ref())?)?;
        }
    }
    Ok(dict.unbind())
}

#[pyclass(unsendable)]
struct NativeBouncer {
    conn: Connection,
    transaction_active: bool,
}

impl Drop for NativeBouncer {
    fn drop(&mut self) {
        if self.transaction_active {
            let _ = self.conn.execute_batch("ROLLBACK");
            self.transaction_active = false;
        }
    }
}

impl NativeBouncer {
    fn ensure_no_active_transaction(&self) -> PyResult<()> {
        if self.transaction_active {
            return Err(py_err(
                "transaction is active; use the transaction handle until it finishes",
            ));
        }
        Ok(())
    }

    fn ensure_active_transaction(&self) -> PyResult<()> {
        if !self.transaction_active {
            return Err(py_err("transaction is not active"));
        }
        Ok(())
    }
}

#[pymethods]
impl NativeBouncer {
    #[new]
    fn new(path: String) -> PyResult<Self> {
        Ok(Self {
            conn: Connection::open(path).map_err(py_err)?,
            transaction_active: false,
        })
    }

    fn bootstrap(&self) -> PyResult<()> {
        core::bootstrap_bouncer_schema(&self.conn).map_err(py_err)
    }

    fn inspect(&self, py: Python<'_>, name: String) -> PyResult<Option<Py<PyDict>>> {
        self.ensure_no_active_transaction()?;
        let lease = core::inspect(&self.conn, &name, system_now_ms()?).map_err(py_err)?;
        optional_lease_dict(py, lease.as_ref())
    }

    fn claim(
        &self,
        py: Python<'_>,
        name: String,
        owner: String,
        ttl_ms: i64,
    ) -> PyResult<Py<PyDict>> {
        self.ensure_no_active_transaction()?;
        let result =
            core::claim(&self.conn, &name, &owner, system_now_ms()?, ttl_ms).map_err(py_err)?;
        claim_dict(py, result)
    }

    fn renew(
        &self,
        py: Python<'_>,
        name: String,
        owner: String,
        ttl_ms: i64,
    ) -> PyResult<Py<PyDict>> {
        self.ensure_no_active_transaction()?;
        let result =
            core::renew(&self.conn, &name, &owner, system_now_ms()?, ttl_ms).map_err(py_err)?;
        renew_dict(py, result)
    }

    fn release(&self, py: Python<'_>, name: String, owner: String) -> PyResult<Py<PyDict>> {
        self.ensure_no_active_transaction()?;
        let result = core::release(&self.conn, &name, &owner, system_now_ms()?).map_err(py_err)?;
        release_dict(py, &name, result)
    }

    fn begin_transaction(&mut self) -> PyResult<()> {
        self.ensure_no_active_transaction()?;
        self.conn.execute_batch("BEGIN IMMEDIATE").map_err(py_err)?;
        self.transaction_active = true;
        Ok(())
    }

    fn commit_transaction(&mut self) -> PyResult<()> {
        self.ensure_active_transaction()?;
        self.conn.execute_batch("COMMIT").map_err(py_err)?;
        self.transaction_active = false;
        Ok(())
    }

    fn rollback_transaction(&mut self) -> PyResult<()> {
        self.ensure_active_transaction()?;
        self.conn.execute_batch("ROLLBACK").map_err(py_err)?;
        self.transaction_active = false;
        Ok(())
    }

    #[pyo3(signature = (sql, params=None))]
    fn execute_in_transaction(
        &self,
        sql: String,
        params: Option<Bound<'_, PyList>>,
    ) -> PyResult<usize> {
        self.ensure_active_transaction()?;
        let values = build_params(params.as_ref())?;
        let mut stmt = self.conn.prepare_cached(&sql).map_err(py_err)?;
        stmt.execute(params_from_iter(values)).map_err(py_err)
    }

    fn inspect_in_transaction(&self, py: Python<'_>, name: String) -> PyResult<Option<Py<PyDict>>> {
        self.ensure_active_transaction()?;
        let lease = core::inspect(&self.conn, &name, system_now_ms()?).map_err(py_err)?;
        optional_lease_dict(py, lease.as_ref())
    }

    fn claim_in_transaction(
        &self,
        py: Python<'_>,
        name: String,
        owner: String,
        ttl_ms: i64,
    ) -> PyResult<Py<PyDict>> {
        self.ensure_active_transaction()?;
        let result = core::claim_in_tx(&self.conn, &name, &owner, system_now_ms()?, ttl_ms)
            .map_err(py_err)?;
        claim_dict(py, result)
    }

    fn renew_in_transaction(
        &self,
        py: Python<'_>,
        name: String,
        owner: String,
        ttl_ms: i64,
    ) -> PyResult<Py<PyDict>> {
        self.ensure_active_transaction()?;
        let result = core::renew_in_tx(&self.conn, &name, &owner, system_now_ms()?, ttl_ms)
            .map_err(py_err)?;
        renew_dict(py, result)
    }

    fn release_in_transaction(
        &self,
        py: Python<'_>,
        name: String,
        owner: String,
    ) -> PyResult<Py<PyDict>> {
        self.ensure_active_transaction()?;
        let result =
            core::release_in_tx(&self.conn, &name, &owner, system_now_ms()?).map_err(py_err)?;
        release_dict(py, &name, result)
    }
}

#[pymodule]
fn _bouncer_native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<NativeBouncer>()?;
    Ok(())
}
