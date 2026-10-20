use cityjson_types::resources::handles::CityObjectHandle;
use cityjson_types::resources::storage::StringStorage;
use cityjson_types::v2_0::{
    BBox, CityModel, CityModelType, CityObject, Contact, ContactRole, ContactType, Extension,
    Metadata, VertexRef, extension::Extensions,
};
use serde::Serialize;
use serde::ser::{Error as _, SerializeMap, SerializeSeq};

use crate::errors::Error;
use crate::ser::appearance::{
    AppearanceSerializer, GeometryTemplatesSerializer, ensure_geometry_templates_supported,
    has_appearance, has_geometry_templates,
};
use crate::ser::attributes::{AttributesSerializer, serialize_attributes_entries};
use crate::ser::context::WriteContext;
use crate::ser::geometry::GeometriesSerializer;

#[derive(Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct CityModelSerializeOptions<'a> {
    pub(crate) type_name: CityModelType,
    pub(crate) include_id: bool,
    pub(crate) include_version: bool,
    pub(crate) transform: Option<&'a cityjson_types::v2_0::Transform>,
    pub(crate) include_transform: bool,
    pub(crate) include_metadata: bool,
    pub(crate) metadata_geographical_extent: Option<&'a BBox>,
    pub(crate) include_extensions: bool,
    pub(crate) include_vertices: bool,
    pub(crate) include_appearance: bool,
    pub(crate) include_geometry_templates: bool,
    pub(crate) include_cityobjects: bool,
    pub(crate) include_extra: bool,
}

impl<'a> CityModelSerializeOptions<'a> {
    pub(crate) fn for_model<VR, SS>(model: &'a CityModel<VR, SS>) -> Self
    where
        VR: VertexRef + serde::Serialize,
        SS: StringStorage,
    {
        let type_name = model.type_citymodel();
        Self {
            type_name,
            include_id: type_name == CityModelType::CityJSONFeature,
            include_version: type_name != CityModelType::CityJSONFeature,
            transform: model.transform(),
            include_transform: model.transform().is_some(),
            include_metadata: true,
            metadata_geographical_extent: None,
            include_extensions: true,
            include_vertices: true,
            include_appearance: true,
            include_geometry_templates: type_name == CityModelType::CityJSON,
            include_cityobjects: true,
            include_extra: true,
        }
    }
}

pub(crate) fn serialize_citymodel_with_options<S, VR, SS>(
    serializer: S,
    model: &CityModel<VR, SS>,
    options: &CityModelSerializeOptions<'_>,
) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
    VR: VertexRef + serde::Serialize,
    SS: StringStorage,
{
    let context = WriteContext::new(model);
    if options.include_geometry_templates {
        ensure_geometry_templates_supported(model, &context).map_err(S::Error::custom)?;
    }
    CityModelSerializer {
        model,
        context: &context,
        options,
    }
    .serialize(serializer)
}

struct CityModelSerializer<'a, VR, SS>
where
    VR: VertexRef + serde::Serialize,
    SS: StringStorage,
{
    model: &'a CityModel<VR, SS>,
    context: &'a WriteContext,
    options: &'a CityModelSerializeOptions<'a>,
}

impl<VR, SS> Serialize for CityModelSerializer<'_, VR, SS>
where
    VR: VertexRef + serde::Serialize,
    SS: StringStorage,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("type", &self.options.type_name.to_string())?;
        if self.options.include_id
            && self.options.type_name == CityModelType::CityJSONFeature
            && let Some(id_handle) = self.model.id()
        {
            let id = self.context.id_by_handle.get(&id_handle).ok_or_else(|| {
                S::Error::custom(Error::InvalidValue(format!(
                    "missing id for CityObject {id_handle}"
                )))
            })?;
            map.serialize_entry("id", id)?;
        }
        if self.options.include_version {
            map.serialize_entry(
                "version",
                &self.model.version().unwrap_or_default().to_string(),
            )?;
        }
        if self.options.include_transform
            && let Some(transform) = self.options.transform
        {
            map.serialize_entry(
                "transform",
                &TransformSerializer {
                    scale: transform.scale(),
                    translate: transform.translate(),
                },
            )?;
        }
        if self.options.include_metadata
            && (self.model.metadata().is_some()
                || self.options.metadata_geographical_extent.is_some())
        {
            map.serialize_entry(
                "metadata",
                &MetadataSerializer {
                    metadata: self.model.metadata(),
                    geographical_extent: self.options.metadata_geographical_extent,
                    model: self.model,
                    context: self.context,
                },
            )?;
        }
        if self.options.include_extensions
            && let Some(extensions) = self.model.extensions()
        {
            map.serialize_entry("extensions", &ExtensionsSerializer(extensions))?;
        }
        if self.options.include_vertices {
            map.serialize_entry(
                "vertices",
                &VerticesSerializer {
                    model: self.model,
                    transform: self.options.transform,
                },
            )?;
        }
        if self.options.include_appearance && has_appearance(self.model) {
            map.serialize_entry("appearance", &AppearanceSerializer { model: self.model })?;
        }
        if self.options.include_geometry_templates && has_geometry_templates(self.model) {
            map.serialize_entry(
                "geometry-templates",
                &GeometryTemplatesSerializer {
                    model: self.model,
                    context: self.context,
                },
            )?;
        }
        if self.options.include_cityobjects {
            map.serialize_entry(
                "CityObjects",
                &CityObjectsSerializer {
                    model: self.model,
                    context: self.context,
                },
            )?;
        }
        if self.options.include_extra
            && let Some(extra) = self.model.extra()
        {
            serialize_attributes_entries(&mut map, extra, self.model, self.context)?;
        }
        map.end()
    }
}

struct MetadataSerializer<'a, VR, SS>
where
    VR: VertexRef + serde::Serialize,
    SS: StringStorage,
{
    metadata: Option<&'a Metadata<SS>>,
    geographical_extent: Option<&'a BBox>,
    model: &'a CityModel<VR, SS>,
    context: &'a WriteContext,
}

impl<VR, SS> Serialize for MetadataSerializer<'_, VR, SS>
where
    VR: VertexRef + serde::Serialize,
    SS: StringStorage,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        if let Some(extent) = self
            .geographical_extent
            .or_else(|| self.metadata.and_then(Metadata::geographical_extent))
        {
            map.serialize_entry("geographicalExtent", &BBoxSerializer(extent))?;
        }
        if let Some(metadata) = self.metadata {
            if let Some(identifier) = metadata.identifier() {
                map.serialize_entry("identifier", &identifier.to_string())?;
            }
            if let Some(contact) = metadata.point_of_contact() {
                map.serialize_entry(
                    "pointOfContact",
                    &ContactSerializer {
                        contact,
                        model: self.model,
                        context: self.context,
                    },
                )?;
            }
            if let Some(reference_date) = metadata.reference_date() {
                map.serialize_entry("referenceDate", &reference_date.to_string())?;
            }
            if let Some(reference_system) = metadata.reference_system() {
                map.serialize_entry("referenceSystem", &reference_system.to_string())?;
            }
            if let Some(title) = metadata.title() {
                map.serialize_entry("title", title)?;
            }
            if let Some(extra) = metadata.extra() {
                serialize_attributes_entries(&mut map, extra, self.model, self.context)?;
            }
        }
        map.end()
    }
}

struct ContactSerializer<'a, VR, SS>
where
    VR: VertexRef + serde::Serialize,
    SS: StringStorage,
{
    contact: &'a Contact<SS>,
    model: &'a CityModel<VR, SS>,
    context: &'a WriteContext,
}

impl<VR, SS> Serialize for ContactSerializer<'_, VR, SS>
where
    VR: VertexRef + serde::Serialize,
    SS: StringStorage,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let contact = self.contact;
        let mut map = serializer.serialize_map(None)?;
        if !contact.contact_name().is_empty() {
            map.serialize_entry("contactName", contact.contact_name())?;
        }
        if !contact.email_address().is_empty() {
            map.serialize_entry("emailAddress", contact.email_address())?;
        }
        if let Some(role) = contact.role() {
            map.serialize_entry("role", contact_role_to_str(role))?;
        }
        if let Some(website) = contact.website().as_ref() {
            map.serialize_entry("website", website.as_ref())?;
        }
        if let Some(kind) = contact.contact_type() {
            map.serialize_entry("contactType", contact_type_to_str(kind))?;
        }
        if let Some(address) = contact.address() {
            map.serialize_entry(
                "address",
                &AttributesSerializer {
                    attributes: address,
                    model: self.model,
                    context: self.context,
                },
            )?;
        }
        if let Some(phone) = contact.phone().as_ref() {
            map.serialize_entry("phone", phone.as_ref())?;
        }
        if let Some(organization) = contact.organization().as_ref() {
            map.serialize_entry("organization", organization.as_ref())?;
        }
        map.end()
    }
}

struct ExtensionsSerializer<'a, SS>(&'a Extensions<SS>)
where
    SS: StringStorage;

impl<SS> Serialize for ExtensionsSerializer<'_, SS>
where
    SS: StringStorage,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for extension in self.0 {
            map.serialize_entry(extension.name().as_ref(), &ExtensionSerializer(extension))?;
        }
        map.end()
    }
}

struct ExtensionSerializer<'a, SS>(&'a Extension<SS>)
where
    SS: StringStorage;

impl<SS> Serialize for ExtensionSerializer<'_, SS>
where
    SS: StringStorage,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("url", self.0.url().as_ref())?;
        map.serialize_entry("version", self.0.version().as_ref())?;
        map.end()
    }
}

struct CityObjectsSerializer<'a, VR, SS>
where
    VR: VertexRef + serde::Serialize,
    SS: StringStorage,
{
    model: &'a CityModel<VR, SS>,
    context: &'a WriteContext,
}

impl<VR, SS> Serialize for CityObjectsSerializer<'_, VR, SS>
where
    VR: VertexRef + serde::Serialize,
    SS: StringStorage,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.model.cityobjects().len()))?;
        for (handle, cityobject) in self.model.cityobjects().iter() {
            let id = self.context.id_by_handle.get(&handle).ok_or_else(|| {
                S::Error::custom(Error::InvalidValue(format!(
                    "missing id for CityObject {handle}"
                )))
            })?;
            map.serialize_entry(
                id,
                &CityObjectSerializer {
                    model: self.model,
                    cityobject,
                    context: self.context,
                },
            )?;
        }
        map.end()
    }
}

struct CityObjectSerializer<'a, VR, SS>
where
    VR: VertexRef + serde::Serialize,
    SS: StringStorage,
{
    model: &'a CityModel<VR, SS>,
    cityobject: &'a CityObject<SS>,
    context: &'a WriteContext,
}

impl<VR, SS> Serialize for CityObjectSerializer<'_, VR, SS>
where
    VR: VertexRef + serde::Serialize,
    SS: StringStorage,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let cityobject = self.cityobject;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("type", &cityobject.type_cityobject().to_string())?;
        if let Some(extent) = cityobject.geographical_extent() {
            map.serialize_entry("geographicalExtent", &BBoxSerializer(extent))?;
        }
        if let Some(attributes) = cityobject.attributes()
            && !attributes.is_empty()
        {
            map.serialize_entry(
                "attributes",
                &AttributesSerializer {
                    attributes,
                    model: self.model,
                    context: self.context,
                },
            )?;
        }
        if let Some(geometry) = cityobject.geometry() {
            map.serialize_entry(
                "geometry",
                &GeometriesSerializer {
                    model: self.model,
                    handles: geometry,
                    context: self.context,
                },
            )?;
        }
        if let Some(parents) = cityobject.parents()
            && !parents.is_empty()
        {
            map.serialize_entry(
                "parents",
                &RelationSerializer {
                    source_id: cityobject.id(),
                    relation: "parent",
                    handles: parents,
                    context: self.context,
                },
            )?;
        }
        if let Some(children) = cityobject.children()
            && !children.is_empty()
        {
            map.serialize_entry(
                "children",
                &RelationSerializer {
                    source_id: cityobject.id(),
                    relation: "child",
                    handles: children,
                    context: self.context,
                },
            )?;
        }
        if let Some(extra) = cityobject.extra() {
            serialize_attributes_entries(&mut map, extra, self.model, self.context)?;
        }
        map.end()
    }
}

struct RelationSerializer<'a> {
    source_id: &'a str,
    relation: &'static str,
    handles: &'a [CityObjectHandle],
    context: &'a WriteContext,
}

impl Serialize for RelationSerializer<'_> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.handles.len()))?;
        for handle in self.handles {
            let id = self.context.id_by_handle.get(handle).ok_or_else(|| {
                S::Error::custom(Error::UnresolvedCityObjectReference {
                    source_id: self.source_id.to_owned(),
                    target_id: handle.to_string(),
                    relation: self.relation,
                })
            })?;
            seq.serialize_element(id)?;
        }
        seq.end()
    }
}

struct VerticesSerializer<'a, VR, SS>
where
    VR: VertexRef + serde::Serialize,
    SS: StringStorage,
{
    model: &'a CityModel<VR, SS>,
    transform: Option<&'a cityjson_types::v2_0::Transform>,
}

impl<VR, SS> Serialize for VerticesSerializer<'_, VR, SS>
where
    VR: VertexRef + serde::Serialize,
    SS: StringStorage,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let vertices = self.model.vertices().as_slice();
        let mut seq = serializer.serialize_seq(Some(vertices.len()))?;
        for vertex in vertices {
            seq.serialize_element(&VertexSerializer {
                x: vertex.x(),
                y: vertex.y(),
                z: vertex.z(),
                transform: self.transform,
            })?;
        }
        seq.end()
    }
}

struct VertexSerializer<'a> {
    x: f64,
    y: f64,
    z: f64,
    transform: Option<&'a cityjson_types::v2_0::Transform>,
}

impl Serialize for VertexSerializer<'_> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(3))?;
        if let Some(transform) = self.transform {
            let scale = transform.scale();
            let translate = transform.translate();
            seq.serialize_element(&QuantizedNumber((self.x - translate[0]) / scale[0]))?;
            seq.serialize_element(&QuantizedNumber((self.y - translate[1]) / scale[1]))?;
            seq.serialize_element(&QuantizedNumber((self.z - translate[2]) / scale[2]))?;
        } else {
            // -- uncompressed document: vertices are real-world values and
            //    must keep their fractional part (CityJSON specs: "vertices"
            //    without a transform are float coordinates)
            seq.serialize_element(&self.x)?;
            seq.serialize_element(&self.y)?;
            seq.serialize_element(&self.z)?;
        }
        seq.end()
    }
}

struct TransformSerializer {
    scale: [f64; 3],
    translate: [f64; 3],
}

impl Serialize for TransformSerializer {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("scale", &self.scale)?;
        map.serialize_entry("translate", &self.translate)?;
        map.end()
    }
}

struct BBoxSerializer<'a>(&'a BBox);

impl Serialize for BBoxSerializer<'_> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let extent: [f64; 6] = (*self.0).into();
        let mut seq = serializer.serialize_seq(Some(6))?;
        for value in extent {
            seq.serialize_element(&value)?;
        }
        seq.end()
    }
}

struct QuantizedNumber(f64);

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
impl Serialize for QuantizedNumber {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let rounded = self.0.round();
        if rounded >= i64::MIN as f64 && rounded <= i64::MAX as f64 {
            serializer.serialize_i64(rounded as i64)
        } else if rounded >= 0.0 && rounded <= u64::MAX as f64 {
            serializer.serialize_u64(rounded as u64)
        } else {
            Err(S::Error::custom(Error::InvalidValue(format!(
                "cannot serialize quantized coordinate '{}'",
                self.0
            ))))
        }
    }
}

fn contact_role_to_str(role: ContactRole) -> &'static str {
    match role {
        ContactRole::Author => "author",
        ContactRole::CoAuthor => "co-author",
        ContactRole::Processor => "processor",
        ContactRole::PointOfContact => "pointOfContact",
        ContactRole::Owner => "owner",
        ContactRole::User => "user",
        ContactRole::Distributor => "distributor",
        ContactRole::Originator => "originator",
        ContactRole::Custodian => "custodian",
        ContactRole::ResourceProvider => "resourceProvider",
        ContactRole::RightsHolder => "rightsHolder",
        ContactRole::Sponsor => "sponsor",
        ContactRole::PrincipalInvestigator => "principalInvestigator",
        ContactRole::Stakeholder => "stakeholder",
        ContactRole::Publisher => "publisher",
    }
}

fn contact_type_to_str(kind: ContactType) -> &'static str {
    match kind {
        ContactType::Individual => "individual",
        ContactType::Organization => "organization",
    }
}
