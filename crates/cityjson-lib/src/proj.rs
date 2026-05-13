//! Minimal 3D PROJ wrapper used by `ops`.
//!
//! Adapted from GeoRust `proj` and Tyler's previous local wrapper, with owned
//! cleanup for all allocated PROJ objects.

use std::ffi::{CStr, CString, NulError};
use std::{fmt::Debug, str};

use libc::{c_char, c_double, c_int};
use num_traits::Float;
use proj_sys::{
    PJ_AREA, PJ_CONTEXT, PJ_COORD, PJ_DIRECTION_PJ_FWD, PJ_XYZT, PJconsts, proj_area_create,
    proj_area_destroy, proj_area_set_bbox, proj_context_create, proj_context_destroy,
    proj_context_errno, proj_context_errno_string, proj_create_crs_to_crs, proj_destroy,
    proj_errno, proj_errno_reset, proj_errno_string, proj_normalize_for_visualization, proj_trans,
};
use thiserror::Error;

pub(crate) trait CoordinateType: Float + Copy + PartialOrd + Debug {}

impl<T: Float + Copy + PartialOrd + Debug> CoordinateType for T {}

#[derive(Copy, Clone, Debug)]
pub(crate) struct Area {
    pub(crate) north: f64,
    pub(crate) south: f64,
    pub(crate) east: f64,
    pub(crate) west: f64,
}

pub(crate) trait Coord<T>
where
    T: CoordinateType,
{
    fn x(&self) -> T;
    fn y(&self) -> T;
    fn z(&self) -> T;
    fn from_xyz(x: T, y: T, z: T) -> Self;
}

impl<T: CoordinateType> Coord<T> for (T, T, T) {
    fn x(&self) -> T {
        self.0
    }

    fn y(&self) -> T {
        self.1
    }

    fn z(&self) -> T {
        self.2
    }

    fn from_xyz(x: T, y: T, z: T) -> Self {
        (x, y, z)
    }
}

pub(crate) struct Proj {
    pj: *mut PJconsts,
    ctx: *mut PJ_CONTEXT,
    area: Option<*mut PJ_AREA>,
}

unsafe impl Send for Proj {}

impl Proj {
    pub(crate) fn new_known_crs(
        from: &str,
        to: &str,
        area: Option<Area>,
    ) -> Result<Self, ProjCreateError> {
        let ctx = unsafe { proj_context_create() };
        if ctx.is_null() {
            return Err(ProjCreateError::ProjError(
                "failed to create PROJ context".to_string(),
            ));
        }
        transform_epsg(ctx, from, to, area)
    }

    pub(crate) fn convert<C, F>(&self, point: C) -> Result<C, ProjError>
    where
        C: Coord<F>,
        F: CoordinateType,
    {
        let c_x: c_double = point.x().to_f64().ok_or(ProjError::FloatConversion)?;
        let c_y: c_double = point.y().to_f64().ok_or(ProjError::FloatConversion)?;
        let c_z: c_double = point.z().to_f64().ok_or(ProjError::FloatConversion)?;

        let xyzt = PJ_XYZT {
            x: c_x,
            y: c_y,
            z: c_z,
            t: f64::INFINITY,
        };
        let (new_x, new_y, new_z, err) = unsafe {
            proj_errno_reset(self.pj);
            let trans = proj_trans(self.pj, PJ_DIRECTION_PJ_FWD, PJ_COORD { xyzt });
            (trans.xyz.x, trans.xyz.y, trans.xyz.z, proj_errno(self.pj))
        };

        if err == 0 {
            Ok(C::from_xyz(
                F::from(new_x).ok_or(ProjError::FloatConversion)?,
                F::from(new_y).ok_or(ProjError::FloatConversion)?,
                F::from(new_z).ok_or(ProjError::FloatConversion)?,
            ))
        } else {
            Err(ProjError::Conversion(error_message(err)?))
        }
    }
}

impl Drop for Proj {
    fn drop(&mut self) {
        unsafe {
            proj_destroy(self.pj);
            if let Some(area) = self.area {
                proj_area_destroy(area);
            }
            proj_context_destroy(self.ctx);
        }
    }
}

#[derive(Error, Debug)]
pub(crate) enum ProjError {
    #[error("conversion failed: {0}")]
    Conversion(String),
    #[error("could not create raw pointer from string")]
    Creation(#[from] NulError),
    #[error("could not convert PROJ bytes to UTF-8")]
    Utf8Error(#[from] str::Utf8Error),
    #[error("could not convert number to f64")]
    FloatConversion,
}

#[derive(Error, Debug)]
pub(crate) enum ProjCreateError {
    #[error("nul byte in PROJ string definition or CRS argument: {0}")]
    ArgumentNulError(NulError),
    #[error("underlying PROJ call failed: {0}")]
    ProjError(String),
}

struct Errno(c_int);

impl Errno {
    fn message(&self, context: *mut PJ_CONTEXT) -> String {
        let ptr = unsafe { proj_context_errno_string(context, self.0) };
        if ptr.is_null() {
            format!("PROJ error code {}", self.0)
        } else {
            unsafe { raw_string(ptr).unwrap_or_else(|_| format!("PROJ error code {}", self.0)) }
        }
    }
}

fn transform_epsg(
    ctx: *mut PJ_CONTEXT,
    from: &str,
    to: &str,
    area: Option<Area>,
) -> Result<Proj, ProjCreateError> {
    let from_c = CString::new(from).map_err(ProjCreateError::ArgumentNulError)?;
    let to_c = CString::new(to).map_err(ProjCreateError::ArgumentNulError)?;
    let proj_area = unsafe { proj_area_create() };
    if proj_area.is_null() {
        unsafe {
            proj_context_destroy(ctx);
        }
        return Err(ProjCreateError::ProjError(
            "failed to create PROJ area".to_string(),
        ));
    }

    area_set_bbox(proj_area, area);
    let ptr = match result_from_create(ctx, unsafe {
        proj_create_crs_to_crs(ctx, from_c.as_ptr(), to_c.as_ptr(), proj_area)
    }) {
        Ok(ptr) => ptr,
        Err(error) => {
            let message = error.message(ctx);
            unsafe {
                proj_area_destroy(proj_area);
                proj_context_destroy(ctx);
            }
            return Err(ProjCreateError::ProjError(message));
        }
    };
    let normalised = unsafe {
        let normalised = proj_normalize_for_visualization(ctx, ptr);
        proj_destroy(ptr);
        normalised
    };
    if normalised.is_null() {
        let error = Errno(unsafe { proj_context_errno(ctx) }).message(ctx);
        unsafe {
            proj_area_destroy(proj_area);
            proj_context_destroy(ctx);
        }
        return Err(ProjCreateError::ProjError(error));
    }

    Ok(Proj {
        pj: normalised,
        ctx,
        area: Some(proj_area),
    })
}

fn area_set_bbox(parea: *mut PJ_AREA, new_area: Option<Area>) {
    if let Some(area) = new_area {
        unsafe {
            proj_area_set_bbox(parea, area.west, area.south, area.east, area.north);
        }
    }
}

fn result_from_create<T>(context: *mut PJ_CONTEXT, ptr: *mut T) -> Result<*mut T, Errno> {
    if ptr.is_null() {
        Err(Errno(unsafe { proj_context_errno(context) }))
    } else {
        Ok(ptr)
    }
}

fn error_message(error: c_int) -> Result<String, str::Utf8Error> {
    let ptr = unsafe { proj_errno_string(error) };
    unsafe { raw_string(ptr) }
}

unsafe fn raw_string(raw_ptr: *const c_char) -> Result<String, str::Utf8Error> {
    assert!(!raw_ptr.is_null());
    let c_str = unsafe { CStr::from_ptr(raw_ptr) };
    Ok(str::from_utf8(c_str.to_bytes())?.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_known_point_to_finite_ecef() {
        let transformer = Proj::new_known_crs("EPSG:7415", "EPSG:4978", None).unwrap();
        let result = transformer.convert((85285.279, 446606.813, 10.0)).unwrap();

        assert!(result.0.is_finite());
        assert!(result.1.is_finite());
        assert!(result.2.is_finite());
        assert!((result.0 - 3_923_215.044).abs() < 10.0);
        assert!((result.1 - 299_940.760).abs() < 10.0);
        assert!((result.2 - 5_003_047.651).abs() < 10.0);
    }

    #[test]
    fn invalid_crs_returns_creation_error() {
        assert!(Proj::new_known_crs("EPSG:0", "EPSG:4978", None).is_err());
    }
}
