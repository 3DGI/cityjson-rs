//! Canonical geometry fixtures for the geometry test suite.

use cityjson_types::CityModelType;
use cityjson_types::prelude::*;
use cityjson_types::v2_0::appearance::ImageType;
use cityjson_types::v2_0::*;

pub type OwnedModel = CityModel<u32, OwnedStringStorage>;

pub fn make_model() -> OwnedModel {
    CityModel::new(CityModelType::CityJSON)
}

pub struct P1Result {
    pub model: OwnedModel,
    pub handle: GeometryHandle,
}

pub fn build_p1() -> P1Result {
    let mut model = make_model();
    let roof = model
        .add_semantic(OwnedSemantic::new(SemanticType::RoofSurface))
        .unwrap();
    let wall = model
        .add_semantic(OwnedSemantic::new(SemanticType::WallSurface))
        .unwrap();

    let handle = GeometryDraft::multi_point(
        None,
        [
            PointDraft::new([0.0, 0.0, 0.0]).with_semantic(roof),
            PointDraft::new([1.0, 0.0, 0.0]),
            PointDraft::new([2.0, 0.0, 0.0]).with_semantic(wall),
        ],
    )
    .insert_into(&mut model)
    .unwrap();

    P1Result { model, handle }
}

pub struct L1Result {
    pub model: OwnedModel,
    pub handle: GeometryHandle,
}

pub fn build_l1() -> L1Result {
    let mut model = make_model();
    let roof = model
        .add_semantic(OwnedSemantic::new(SemanticType::RoofSurface))
        .unwrap();

    let handle = GeometryDraft::multi_line_string(
        None,
        [
            LineStringDraft::new([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]]),
            LineStringDraft::new([[1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 0.0, 0.0]])
                .with_semantic(roof),
        ],
    )
    .insert_into(&mut model)
    .unwrap();

    L1Result { model, handle }
}

pub struct S1Result {
    pub model: OwnedModel,
    pub handle: GeometryHandle,
}

pub fn build_s1(type_geom: GeometryType) -> S1Result {
    assert!(type_geom == GeometryType::MultiSurface || type_geom == GeometryType::CompositeSurface);

    let mut model = make_model();
    let roof = model
        .add_semantic(OwnedSemantic::new(SemanticType::RoofSurface))
        .unwrap();
    let wall = model
        .add_semantic(OwnedSemantic::new(SemanticType::WallSurface))
        .unwrap();
    let material = model
        .add_material(OwnedMaterial::new("mat-a".to_string()))
        .unwrap();
    let texture = model
        .add_texture(OwnedTexture::new("tex-a.png".to_string(), ImageType::Png))
        .unwrap();
    let theme = "theme-a".to_string();

    let s0 = SurfaceDraft::new(
        RingDraft::new([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [4.0, 0.0, 0.0]]).with_texture(
            theme.clone(),
            texture,
            [[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]],
        ),
        [RingDraft::new([
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
        ])],
    )
    .with_semantic(roof)
    .with_material(theme.clone(), material);

    let s1 = SurfaceDraft::new(
        RingDraft::new([
            [2.0, 0.0, 0.0],
            [3.0, 0.0, 0.0],
            [4.0, 0.0, 0.0],
            [5.0, 0.0, 0.0],
        ])
        .with_texture(
            theme.clone(),
            texture,
            [[0.0, 0.5], [1.0, 0.5], [0.5, 0.0], [0.0, 1.0]],
        ),
        [],
    )
    .with_semantic(wall);

    let draft = match type_geom {
        GeometryType::MultiSurface => GeometryDraft::multi_surface(None, [s0, s1]),
        GeometryType::CompositeSurface => GeometryDraft::composite_surface(None, [s0, s1]),
        _ => unreachable!(),
    };
    let handle = draft.insert_into(&mut model).unwrap();

    S1Result { model, handle }
}

pub struct D1Result {
    pub model: OwnedModel,
    pub handle: GeometryHandle,
}

pub fn build_d1() -> D1Result {
    let mut model = make_model();
    let roof = model
        .add_semantic(OwnedSemantic::new(SemanticType::RoofSurface))
        .unwrap();
    let wall = model
        .add_semantic(OwnedSemantic::new(SemanticType::WallSurface))
        .unwrap();
    let ground = model
        .add_semantic(OwnedSemantic::new(SemanticType::GroundSurface))
        .unwrap();

    let outer = ShellDraft::new([
        SurfaceDraft::new(
            RingDraft::new([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]),
            [],
        )
        .with_semantic(roof),
        SurfaceDraft::new(
            RingDraft::new([[2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [0.0, 1.0, 0.0]]),
            [],
        )
        .with_semantic(wall),
    ]);
    let inner = ShellDraft::new([
        SurfaceDraft::new(
            RingDraft::new([[0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [0.0, 0.0, 0.0]]),
            [],
        )
        .with_semantic(ground),
        SurfaceDraft::new(
            RingDraft::new([[1.0, 0.0, 0.0], [2.0, 1.0, 0.0], [3.0, 1.0, 0.0]]),
            [],
        ),
    ]);

    let handle = GeometryDraft::solid(None, outer, [inner])
        .insert_into(&mut model)
        .unwrap();

    D1Result { model, handle }
}

pub struct MS1Result {
    pub model: OwnedModel,
    pub handle: GeometryHandle,
}

pub fn build_ms1(type_geom: GeometryType) -> MS1Result {
    assert!(type_geom == GeometryType::MultiSolid || type_geom == GeometryType::CompositeSolid);

    let mut model = make_model();
    let roof = model
        .add_semantic(OwnedSemantic::new(SemanticType::RoofSurface))
        .unwrap();
    let wall = model
        .add_semantic(OwnedSemantic::new(SemanticType::WallSurface))
        .unwrap();
    let ground = model
        .add_semantic(OwnedSemantic::new(SemanticType::GroundSurface))
        .unwrap();

    let solid0 = SolidDraft::new(
        ShellDraft::new([
            SurfaceDraft::new(
                RingDraft::new([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]),
                [],
            )
            .with_semantic(roof),
            SurfaceDraft::new(
                RingDraft::new([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, 0.0, 0.0]]),
                [],
            )
            .with_semantic(wall),
        ]),
        [],
    );
    let solid1 = SolidDraft::new(
        ShellDraft::new([
            SurfaceDraft::new(
                RingDraft::new([[3.0, 0.0, 0.0], [4.0, 0.0, 0.0], [5.0, 0.0, 0.0]]),
                [],
            )
            .with_semantic(ground),
            SurfaceDraft::new(
                RingDraft::new([[3.0, 0.0, 0.0], [5.0, 0.0, 0.0], [4.0, 0.0, 0.0]]),
                [],
            ),
        ]),
        [],
    );

    let draft = match type_geom {
        GeometryType::MultiSolid => GeometryDraft::multi_solid(None, [solid0, solid1]),
        GeometryType::CompositeSolid => GeometryDraft::composite_solid(None, [solid0, solid1]),
        _ => unreachable!(),
    };
    let handle = draft.insert_into(&mut model).unwrap();

    MS1Result { model, handle }
}

pub struct T1Result {
    pub model: OwnedModel,
    pub template_handle: GeometryTemplateHandle,
}

pub fn build_t1() -> T1Result {
    let mut model = make_model();
    let roof = model
        .add_semantic(OwnedSemantic::new(SemanticType::RoofSurface))
        .unwrap();
    let wall = model
        .add_semantic(OwnedSemantic::new(SemanticType::WallSurface))
        .unwrap();

    let template_handle = GeometryDraft::multi_surface(
        None,
        [
            SurfaceDraft::new(
                RingDraft::new([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]),
                [],
            )
            .with_semantic(roof),
            SurfaceDraft::new(
                RingDraft::new([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 0.0, 0.0]]),
                [],
            )
            .with_semantic(wall),
        ],
    )
    .insert_template_into(&mut model)
    .unwrap();

    T1Result {
        model,
        template_handle,
    }
}

pub struct I1Result {
    pub model: OwnedModel,
    pub template_handle: GeometryTemplateHandle,
    pub instance_handle: GeometryHandle,
}

pub fn build_i1() -> I1Result {
    let T1Result {
        mut model,
        template_handle,
    } = build_t1();

    let instance_handle = GeometryDraft::instance(
        template_handle,
        [10.0, 20.0, 0.0],
        AffineTransform3D::identity(),
    )
    .insert_into(&mut model)
    .unwrap();

    I1Result {
        model,
        template_handle,
        instance_handle,
    }
}
