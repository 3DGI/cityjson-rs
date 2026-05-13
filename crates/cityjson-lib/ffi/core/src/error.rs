use std::cell::RefCell;
use std::ffi::c_char;
use std::panic::{AssertUnwindSafe, UnwindSafe, catch_unwind};
use std::ptr;

use crate::abi::{cj_error_kind_t, cj_status_t};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiError {
    pub status: cj_status_t,
    pub kind: cj_error_kind_t,
    pub message: String,
}

impl AbiError {
    pub fn new(status: cj_status_t, kind: cj_error_kind_t, message: impl Into<String>) -> Self {
        Self {
            status,
            kind,
            message: message.into(),
        }
    }

    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new(
            cj_status_t::CJ_STATUS_INVALID_ARGUMENT,
            cj_error_kind_t::CJ_ERROR_KIND_INVALID_ARGUMENT,
            message,
        )
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(
            cj_status_t::CJ_STATUS_INTERNAL,
            cj_error_kind_t::CJ_ERROR_KIND_INTERNAL,
            message,
        )
    }
}

impl From<&cityjson_lib::Error> for AbiError {
    fn from(error: &cityjson_lib::Error) -> Self {
        match error {
            cityjson_lib::Error::Io(inner) => Self::new(
                cj_status_t::CJ_STATUS_IO,
                cj_error_kind_t::CJ_ERROR_KIND_IO,
                inner.to_string(),
            ),
            cityjson_lib::Error::Syntax(inner) => Self::new(
                cj_status_t::CJ_STATUS_SYNTAX,
                cj_error_kind_t::CJ_ERROR_KIND_SYNTAX,
                inner.clone(),
            ),
            cityjson_lib::Error::CityJSON(inner) => Self::new(
                cj_status_t::CJ_STATUS_MODEL,
                cj_error_kind_t::CJ_ERROR_KIND_MODEL,
                inner.to_string(),
            ),
            cityjson_lib::Error::MissingVersion => Self::new(
                cj_status_t::CJ_STATUS_VERSION,
                cj_error_kind_t::CJ_ERROR_KIND_VERSION,
                error.to_string(),
            ),
            cityjson_lib::Error::ExpectedCityJSON(_)
            | cityjson_lib::Error::ExpectedCityJSONFeature(_) => Self::new(
                cj_status_t::CJ_STATUS_SHAPE,
                cj_error_kind_t::CJ_ERROR_KIND_SHAPE,
                error.to_string(),
            ),
            cityjson_lib::Error::UnsupportedType(_) => Self::new(
                cj_status_t::CJ_STATUS_UNSUPPORTED,
                cj_error_kind_t::CJ_ERROR_KIND_UNSUPPORTED,
                error.to_string(),
            ),
            cityjson_lib::Error::UnsupportedVersion { .. } => Self::new(
                cj_status_t::CJ_STATUS_VERSION,
                cj_error_kind_t::CJ_ERROR_KIND_VERSION,
                error.to_string(),
            ),
            cityjson_lib::Error::Streaming(_) => Self::new(
                cj_status_t::CJ_STATUS_SHAPE,
                cj_error_kind_t::CJ_ERROR_KIND_SHAPE,
                error.to_string(),
            ),
            cityjson_lib::Error::Import(_) => Self::new(
                cj_status_t::CJ_STATUS_MODEL,
                cj_error_kind_t::CJ_ERROR_KIND_MODEL,
                error.to_string(),
            ),
            cityjson_lib::Error::Projection(_) => Self::new(
                cj_status_t::CJ_STATUS_MODEL,
                cj_error_kind_t::CJ_ERROR_KIND_MODEL,
                error.to_string(),
            ),
            cityjson_lib::Error::UnsupportedFeature(_) => Self::new(
                cj_status_t::CJ_STATUS_UNSUPPORTED,
                cj_error_kind_t::CJ_ERROR_KIND_UNSUPPORTED,
                error.to_string(),
            ),
        }
    }
}

impl From<cityjson_lib::Error> for AbiError {
    fn from(error: cityjson_lib::Error) -> Self {
        Self::from(&error)
    }
}

impl From<cityjson_lib::cityjson_types::error::Error> for AbiError {
    fn from(error: cityjson_lib::cityjson_types::error::Error) -> Self {
        Self::from(cityjson_lib::Error::from(error))
    }
}

impl From<cityjson_lib::ErrorKind> for cj_error_kind_t {
    fn from(value: cityjson_lib::ErrorKind) -> Self {
        match value {
            cityjson_lib::ErrorKind::Io => Self::CJ_ERROR_KIND_IO,
            cityjson_lib::ErrorKind::Syntax => Self::CJ_ERROR_KIND_SYNTAX,
            cityjson_lib::ErrorKind::Version => Self::CJ_ERROR_KIND_VERSION,
            cityjson_lib::ErrorKind::Shape => Self::CJ_ERROR_KIND_SHAPE,
            cityjson_lib::ErrorKind::Unsupported => Self::CJ_ERROR_KIND_UNSUPPORTED,
            cityjson_lib::ErrorKind::Model => Self::CJ_ERROR_KIND_MODEL,
            cityjson_lib::ErrorKind::Projection => Self::CJ_ERROR_KIND_MODEL,
        }
    }
}

impl From<cityjson_lib::ErrorKind> for cj_status_t {
    fn from(value: cityjson_lib::ErrorKind) -> Self {
        match value {
            cityjson_lib::ErrorKind::Io => Self::CJ_STATUS_IO,
            cityjson_lib::ErrorKind::Syntax => Self::CJ_STATUS_SYNTAX,
            cityjson_lib::ErrorKind::Version => Self::CJ_STATUS_VERSION,
            cityjson_lib::ErrorKind::Shape => Self::CJ_STATUS_SHAPE,
            cityjson_lib::ErrorKind::Unsupported => Self::CJ_STATUS_UNSUPPORTED,
            cityjson_lib::ErrorKind::Model => Self::CJ_STATUS_MODEL,
            cityjson_lib::ErrorKind::Projection => Self::CJ_STATUS_MODEL,
        }
    }
}

#[derive(Debug, Clone)]
struct LastError {
    status: cj_status_t,
    kind: cj_error_kind_t,
    message: String,
}

impl LastError {
    fn empty() -> Self {
        Self {
            status: cj_status_t::CJ_STATUS_SUCCESS,
            kind: cj_error_kind_t::CJ_ERROR_KIND_NONE,
            message: String::new(),
        }
    }
}

thread_local! {
    static LAST_ERROR: RefCell<LastError> = RefCell::new(LastError::empty());
}

pub fn clear_last_error() {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = LastError::empty();
    });
}

pub fn set_last_error(error: AbiError) {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = LastError {
            status: error.status,
            kind: error.kind,
            message: error.message,
        };
    });
}

pub fn set_last_error_from_cityjson_lib_error(error: cityjson_lib::Error) -> cj_status_t {
    let abi_error = AbiError::from(error);
    let status = abi_error.status;
    set_last_error(abi_error);
    status
}

pub fn last_error_kind() -> cj_error_kind_t {
    LAST_ERROR.with(|cell| cell.borrow().kind)
}

pub fn last_error_status() -> cj_status_t {
    LAST_ERROR.with(|cell| cell.borrow().status)
}

pub fn last_error_message_len() -> usize {
    LAST_ERROR.with(|cell| cell.borrow().message.len())
}

pub unsafe fn copy_last_error_message(
    buffer: *mut c_char,
    capacity: usize,
    out_len: *mut usize,
) -> cj_status_t {
    if out_len.is_null() {
        return cj_status_t::CJ_STATUS_INVALID_ARGUMENT;
    }

    let (message_len, message) = LAST_ERROR.with(|cell| {
        let borrowed = cell.borrow();
        (borrowed.message.len(), borrowed.message.clone())
    });

    unsafe {
        ptr::write(out_len, message_len);
    }

    if capacity == 0 {
        if buffer.is_null() {
            return cj_status_t::CJ_STATUS_SUCCESS;
        }

        return cj_status_t::CJ_STATUS_INVALID_ARGUMENT;
    }

    if buffer.is_null() {
        return cj_status_t::CJ_STATUS_INVALID_ARGUMENT;
    }

    let available = capacity.saturating_sub(1);
    let copy_len = message_len.min(available);
    if copy_len > 0 {
        unsafe {
            ptr::copy_nonoverlapping(message.as_ptr().cast::<c_char>(), buffer, copy_len);
        }
    }
    unsafe {
        *buffer.add(copy_len) = 0;
    }

    if message_len >= capacity {
        return cj_status_t::CJ_STATUS_INVALID_ARGUMENT;
    }

    cj_status_t::CJ_STATUS_SUCCESS
}

pub fn run_ffi<T, E, F>(f: F) -> Result<T, cj_status_t>
where
    E: Into<AbiError>,
    F: FnOnce() -> Result<T, E> + UnwindSafe,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(value)) => {
            clear_last_error();
            Ok(value)
        }
        Ok(Err(error)) => {
            let abi_error = error.into();
            let status = abi_error.status;
            set_last_error(abi_error);
            Err(status)
        }
        Err(_) => {
            let abi_error = AbiError::internal("panic across the C ABI boundary");
            let status = abi_error.status;
            set_last_error(abi_error);
            Err(status)
        }
    }
}
