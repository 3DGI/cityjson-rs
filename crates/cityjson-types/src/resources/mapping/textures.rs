use crate::resources::handles::TextureHandle;
use crate::resources::id::{ResourceId, ResourceId32};
use crate::v2_0::vertex::{VertexIndex, VertexRef};

#[repr(C)]
#[derive(Clone, Debug, Default, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub(crate) struct TextureMapCore<VR: VertexRef, RR: ResourceId> {
    vertices: Vec<Option<VertexIndex<VR>>>,
    rings: Vec<VertexIndex<VR>>,
    ring_textures: Vec<Option<RR>>,
    // The boundary is authoritative for surface, shell, and solid topology.
}

impl<VR: VertexRef, RR: ResourceId> TextureMapCore<VR, RR> {
    pub(crate) fn is_empty(&self) -> bool {
        self.vertices.is_empty() && self.rings.is_empty() && self.ring_textures.is_empty()
    }

    pub(crate) fn add_vertex(&mut self, vertex: Option<VertexIndex<VR>>) {
        self.vertices.push(vertex);
    }

    pub(crate) fn add_ring(&mut self, ring_start: VertexIndex<VR>) {
        self.rings.push(ring_start);
    }

    pub(crate) fn add_ring_texture(&mut self, texture: Option<RR>) {
        self.ring_textures.push(texture);
    }

    pub(crate) fn vertices(&self) -> &[Option<VertexIndex<VR>>] {
        &self.vertices
    }

    pub(crate) fn vertices_mut(&mut self) -> &mut [Option<VertexIndex<VR>>] {
        &mut self.vertices
    }

    pub(crate) fn rings(&self) -> &[VertexIndex<VR>] {
        &self.rings
    }

    pub(crate) fn rings_mut(&mut self) -> &mut [VertexIndex<VR>] {
        &mut self.rings
    }

    pub(crate) fn ring_textures(&self) -> &[Option<RR>] {
        &self.ring_textures
    }

    pub(crate) fn ring_textures_mut(&mut self) -> &mut [Option<RR>] {
        &mut self.ring_textures
    }
}

#[repr(C)]
#[derive(Clone, Debug, Default, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct TextureMap<VR: VertexRef> {
    inner: TextureMapCore<VR, ResourceId32>,
}

impl<VR: VertexRef> TextureMap<VR> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn add_vertex(&mut self, vertex: Option<VertexIndex<VR>>) {
        self.inner.add_vertex(vertex);
    }

    pub fn add_ring(&mut self, ring_start: VertexIndex<VR>) {
        self.inner.add_ring(ring_start);
    }

    pub fn add_ring_texture(&mut self, texture: Option<TextureHandle>) {
        self.inner
            .add_ring_texture(texture.map(super::super::handles::TextureHandle::to_raw));
    }

    #[must_use]
    pub fn vertices(&self) -> &[Option<VertexIndex<VR>>] {
        self.inner.vertices()
    }

    pub fn vertices_mut(&mut self) -> &mut [Option<VertexIndex<VR>>] {
        self.inner.vertices_mut()
    }

    #[must_use]
    pub fn rings(&self) -> &[VertexIndex<VR>] {
        self.inner.rings()
    }

    pub fn rings_mut(&mut self) -> &mut [VertexIndex<VR>] {
        self.inner.rings_mut()
    }

    #[must_use]
    pub fn ring_textures(&self) -> Vec<Option<TextureHandle>> {
        self.inner
            .ring_textures()
            .iter()
            .copied()
            .map(|r| r.map(TextureHandle::from_raw))
            .collect()
    }

    pub fn set_ring_texture(&mut self, ring_index: usize, texture: Option<TextureHandle>) -> bool {
        let Some(slot) = self.inner.ring_textures_mut().get_mut(ring_index) else {
            return false;
        };
        *slot = texture.map(super::super::handles::TextureHandle::to_raw);
        true
    }

    #[allow(dead_code)]
    pub(crate) fn from_raw(inner: TextureMapCore<VR, ResourceId32>) -> Self {
        Self { inner }
    }

    #[allow(dead_code)]
    pub(crate) fn into_raw(self) -> TextureMapCore<VR, ResourceId32> {
        self.inner
    }

    #[allow(dead_code)]
    pub(crate) fn to_raw(&self) -> &TextureMapCore<VR, ResourceId32> {
        &self.inner
    }
}

// ---------------------------------------------------------------------------
// Unit tests for TextureMapCore / TextureMap
// Family 8: texture topology (vertices / rings / ring_textures)
// Family 9: vertex-reuse — same geometric vertex, different UV per ring
// ---------------------------------------------------------------------------

#[cfg(test)]
mod texture_map {
    use super::*;
    use crate::resources::id::ResourceId32;

    type Core = TextureMapCore<u32, ResourceId32>;

    fn make_tex_id(index: u32) -> ResourceId32 {
        ResourceId32::new(index, 0)
    }

    fn vi(n: u32) -> VertexIndex<u32> {
        VertexIndex::new(n)
    }

    /// Inputs: texture maps with one untextured ring and two textured rings.
    /// Assertions: ring offsets, ring texture handles, null texture entries, and
    /// UV vertex slots are preserved. Purpose: positive unit coverage for dense
    /// texture-map topology.
    #[test]
    fn texture_map_preserves_ring_offsets_textures_and_nulls() {
        let mut core = Core::default();
        core.add_vertex(Some(vi(0)));
        core.add_vertex(Some(vi(1)));
        core.add_vertex(Some(vi(2)));
        core.add_ring(vi(0));
        core.add_ring_texture(Some(make_tex_id(0)));

        core.add_vertex(None);
        core.add_vertex(None);
        core.add_ring(vi(3));
        core.add_ring_texture(None);

        core.add_vertex(Some(vi(0)));
        core.add_vertex(Some(vi(2)));
        core.add_vertex(Some(vi(3)));
        core.add_ring(vi(5));
        core.add_ring_texture(Some(make_tex_id(1)));

        assert_eq!(core.vertices().len(), 8);
        assert_eq!(core.rings(), &[vi(0), vi(3), vi(5)]);
        assert_eq!(core.ring_textures().len(), 3);
        assert_eq!(core.ring_textures()[0], Some(make_tex_id(0)));
        assert!(core.ring_textures()[1].is_none());
        assert_eq!(core.ring_textures()[2], Some(make_tex_id(1)));
        assert!(core.vertices()[3..5].iter().all(Option::is_none));
    }

    /// Inputs: two texture-map rings that reuse the same geometric vertex refs
    /// in distinct UV slots. Assertions: repeated geometric refs remain separate
    /// occurrences with different ring starts. Purpose: protect occurrence-level
    /// UV mapping for reused geometry vertices.
    #[test]
    fn vertex_reuse_different_uvs_per_ring() {
        let mut core = Core::default();
        core.add_vertex(Some(vi(0)));
        core.add_vertex(Some(vi(1)));
        core.add_ring(vi(0));
        core.add_ring_texture(Some(make_tex_id(0)));
        core.add_vertex(Some(vi(0)));
        core.add_vertex(Some(vi(1)));
        core.add_ring(vi(2));
        core.add_ring_texture(Some(make_tex_id(0)));

        assert_eq!(core.vertices().len(), 4);
        assert_eq!(core.vertices()[0], core.vertices()[2]);
        assert_ne!(core.rings()[0], core.rings()[1]);
    }
}
