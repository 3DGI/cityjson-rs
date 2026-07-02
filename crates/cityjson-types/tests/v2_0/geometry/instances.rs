//! Template geometry and `GeometryInstance` separation tests.

use super::fixtures::*;
use cityjson_types::v2_0::*;

/// Inputs: I1 instance fixture with a valid T1 template. Assertions: the
/// instance stores no boundary/map payload, references the existing template,
/// uses a regular-pool reference point, has the expected transform, and resolves
/// to the template geometry type. Purpose: positive coverage for instance
/// indirection without duplicating one assertion per test.
#[test]
fn geometry_instance_resolves_template_without_own_payload() {
    let I1Result {
        model,
        template_handle,
        instance_handle,
    } = build_i1();
    let geom = model.get_geometry(instance_handle).unwrap();

    assert_eq!(geom.type_geometry(), &GeometryType::GeometryInstance);
    assert!(geom.boundaries().is_none());
    assert!(geom.semantics().is_none());
    assert!(geom.materials().is_none());
    assert!(geom.textures().is_none());

    let instance = geom.instance().unwrap();
    assert_eq!(instance.template(), template_handle);
    assert!(model.get_geometry_template(instance.template()).is_some());
    assert!(model.get_vertex(instance.reference_point()).is_some());
    assert_eq!(instance.transformation(), AffineTransform3D::identity());

    let resolved = model.resolve_geometry(instance_handle).unwrap();
    assert_eq!(resolved.type_geometry(), &GeometryType::MultiSurface);
}

/// Inputs: T1 template fixture and P1 regular fixture. Assertions: template
/// geometry validates as surface geometry, uses only template vertices, and is
/// stored separately from regular geometry. Purpose: positive coverage for the
/// template/regular pool split.
#[test]
fn template_geometry_uses_template_pool_and_regular_shape_rules() {
    let T1Result {
        model,
        template_handle,
    } = build_t1();
    let geom = model.get_geometry_template(template_handle).unwrap();

    assert_eq!(geom.type_geometry(), &GeometryType::MultiSurface);
    let boundary = geom.boundaries().unwrap();
    assert!(boundary.is_consistent());
    assert_eq!(boundary.surfaces().len(), 2);
    assert_eq!(model.vertices().len(), 0);
    assert!(!model.template_vertices().is_empty());
    assert_eq!(model.geometry_count(), 0);
    assert_eq!(model.geometry_template_count(), 1);

    let P1Result { model, .. } = build_p1();
    assert_eq!(model.geometry_count(), 1);
    assert_eq!(model.geometry_template_count(), 0);
}
