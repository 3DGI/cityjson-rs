#pragma once

#include <cstddef>
#include <cstdint>
#include <array>
#include <cstring>
#include <limits>
#include <optional>
#include <span>
#include <stdexcept>
#include <string>
#include <string_view>
#include <type_traits>
#include <utility>
#include <vector>

#include <cityjson_lib/cityjson_lib.h>

namespace cityjson_lib {

using Status = cj_status_t;
using ErrorKind = cj_error_kind_t;
using RootKind = cj_root_kind_t;
using Version = cj_version_t;
using ModelType = cj_model_type_t;
using GeometryType = cj_geometry_type_t;
using Probe = cj_probe_t;
using ModelSummary = cj_model_summary_t;
using ModelCapacities = cj_model_capacities_t;
using Vertex = cj_vertex_t;
using UV = cj_uv_t;
using ContactRole = cj_contact_role_t;
using ContactType = cj_contact_type_t;
using ImageType = cj_image_type_t;
using WrapMode = cj_wrap_mode_t;
using TextureType = cj_texture_type_t;
using BBox = cj_bbox_t;
using Rgb = cj_rgb_t;
using Rgba = cj_rgba_t;
using AffineTransform4x4 = cj_affine_transform_4x4_t;

struct WriteOptions final {
  bool pretty = false;
  bool validate_default_themes = true;
};

struct Transform final {
  std::array<double, 3> scale{1.0, 1.0, 1.0};
  std::array<double, 3> translate{0.0, 0.0, 0.0};
};

struct GeometryBoundary final {
  GeometryType geometry_type;
  bool has_boundaries;
  std::vector<std::size_t> vertex_indices;
  std::vector<std::size_t> ring_offsets;
  std::vector<std::size_t> surface_offsets;
  std::vector<std::size_t> shell_offsets;
  std::vector<std::size_t> solid_offsets;
};

struct GeometrySelectionSpec final {
  constexpr GeometrySelectionSpec(std::string_view cityobject_id, std::size_t geometry_index)
      : cityobject_id(cityobject_id), geometry_index(geometry_index) {}

  std::string_view cityobject_id;
  std::size_t geometry_index;
};

class StatusError final : public std::runtime_error {
 public:
  StatusError(Status status, ErrorKind kind, std::string message)
      : std::runtime_error(std::move(message)), status_(status), kind_(kind) {}

  [[nodiscard]] Status status() const noexcept { return status_; }
  [[nodiscard]] ErrorKind kind() const noexcept { return kind_; }

 private:
  Status status_;
  ErrorKind kind_;
};

inline std::string last_error_message() {
  const std::size_t len = cj_last_error_message_len();
  if (len == 0U) {
    return {};
  }

  std::vector<std::uint8_t> buffer(len + 1U, 0U);
  std::size_t copied = 0U;
  const auto status = cj_last_error_message_copy(buffer.data(), buffer.size(), &copied);
  if (status != CJ_STATUS_SUCCESS) {
    return "failed to retrieve cityjson_lib last-error message";
  }

  return std::string(reinterpret_cast<const char*>(buffer.data()), copied);
}

[[noreturn]] inline void throw_last_error(Status status) {
  throw StatusError(status, cj_last_error_kind(), last_error_message());
}

inline void check_status(Status status) {
  if (status != CJ_STATUS_SUCCESS) {
    throw_last_error(status);
  }
}

inline const std::uint8_t* span_data(std::span<const std::uint8_t> bytes) noexcept {
  return bytes.empty() ? nullptr : bytes.data();
}

inline cj_string_view_t to_view(std::string_view value) noexcept {
  return {
      .data = reinterpret_cast<const std::uint8_t*>(value.data()),
      .len = value.size(),
  };
}

inline cj_json_write_options_t to_native(const WriteOptions& options) noexcept {
  return {
      .pretty = options.pretty,
      .validate_default_themes = options.validate_default_themes,
  };
}

inline cj_transform_t to_native(const Transform& transform) noexcept {
  return {
      .scale_x = transform.scale[0],
      .scale_y = transform.scale[1],
      .scale_z = transform.scale[2],
      .translate_x = transform.translate[0],
      .translate_y = transform.translate[1],
      .translate_z = transform.translate[2],
  };
}

inline cj_affine_transform_4x4_t to_native(const AffineTransform4x4& transform) noexcept {
  return transform;
}

inline std::string take_string(cj_bytes_t bytes) {
  std::string value;
  if (bytes.len > 0U) {
    value.assign(reinterpret_cast<const char*>(bytes.data), bytes.len);
  }
  check_status(cj_bytes_free(bytes));
  return value;
}

inline std::vector<std::string> take_string_list(cj_bytes_list_t bytes) {
  struct FreeGuard {
    cj_bytes_list_t bytes;
    ~FreeGuard() { static_cast<void>(cj_bytes_list_free(bytes)); }
  } guard{bytes};

  std::vector<std::string> value;
  value.reserve(bytes.len);
  for (std::size_t index = 0U; index < bytes.len; ++index) {
    const auto item = bytes.data[index];
    if (item.len > 0U) {
      value.emplace_back(reinterpret_cast<const char*>(item.data), item.len);
    } else {
      value.emplace_back();
    }
  }
  return value;
}

inline std::vector<std::uint8_t> take_bytes(cj_bytes_t bytes) {
  std::vector<std::uint8_t> value;
  if (bytes.len > 0U) {
    value.assign(bytes.data, bytes.data + bytes.len);
  }
  check_status(cj_bytes_free(bytes));
  return value;
}

inline std::vector<Vertex> take_vertices(cj_vertices_t vertices) {
  std::vector<Vertex> value;
  if (vertices.len > 0U) {
    value.assign(vertices.data, vertices.data + vertices.len);
  }
  check_status(cj_vertices_free(vertices));
  return value;
}

inline std::vector<UV> take_uvs(cj_uvs_t uvs) {
  std::vector<UV> value;
  if (uvs.len > 0U) {
    value.assign(uvs.data, uvs.data + uvs.len);
  }
  check_status(cj_uvs_free(uvs));
  return value;
}

inline std::vector<GeometryType> take_geometry_types(cj_geometry_types_t types) {
  std::vector<GeometryType> value;
  if (types.len > 0U) {
    value.assign(types.data, types.data + types.len);
  }
  check_status(cj_geometry_types_free(types));
  return value;
}

inline std::vector<std::size_t> copy_indices(cj_indices_t indices) {
  std::vector<std::size_t> value;
  if (indices.len > 0U) {
    value.assign(indices.data, indices.data + indices.len);
  }
  return value;
}

inline std::string copy_string(cj_bytes_t bytes) {
  std::string value;
  if (bytes.len > 0U) {
    value.assign(reinterpret_cast<const char*>(bytes.data), bytes.len);
  }
  return value;
}

inline GeometryBoundary take_geometry_boundary(cj_geometry_boundary_t boundary) {
  struct FreeGuard {
    cj_geometry_boundary_t boundary;
    ~FreeGuard() { static_cast<void>(cj_geometry_boundary_free(boundary)); }
  } guard{boundary};

  return GeometryBoundary{
      .geometry_type = boundary.geometry_type,
      .has_boundaries = boundary.has_boundaries,
      .vertex_indices = copy_indices(boundary.vertex_indices),
      .ring_offsets = copy_indices(boundary.ring_offsets),
      .surface_offsets = copy_indices(boundary.surface_offsets),
      .shell_offsets = copy_indices(boundary.shell_offsets),
      .solid_offsets = copy_indices(boundary.solid_offsets),
  };
}

namespace detail {

template <typename Handle, Status (*FreeFn)(Handle*)>
class OwnedHandle final {
 public:
  OwnedHandle() = default;

  explicit OwnedHandle(Handle* handle) : handle_(handle) {}

  OwnedHandle(const OwnedHandle&) = delete;
  OwnedHandle& operator=(const OwnedHandle&) = delete;

  OwnedHandle(OwnedHandle&& other) noexcept : handle_(std::exchange(other.handle_, nullptr)) {}

  OwnedHandle& operator=(OwnedHandle&& other) noexcept {
    if (this != &other) {
      reset();
      handle_ = std::exchange(other.handle_, nullptr);
    }
    return *this;
  }

  ~OwnedHandle() { reset(); }

  [[nodiscard]] Handle* get() const noexcept { return handle_; }

  [[nodiscard]] bool valid() const noexcept { return handle_ != nullptr; }

  [[nodiscard]] Handle* release() noexcept { return std::exchange(handle_, nullptr); }

  void reset(Handle* handle = nullptr) noexcept {
    if (handle_ != nullptr) {
      static_cast<void>(FreeFn(handle_));
    }
    handle_ = handle;
  }

 private:
  Handle* handle_ = nullptr;
};

template <typename RawId>
[[nodiscard]] inline bool id_valid(const RawId& raw) noexcept {
  return raw.slot != 0U || raw.generation != 0U;
}

[[nodiscard]] inline std::uint32_t narrow_index(std::size_t index) {
  if (index > std::numeric_limits<std::uint32_t>::max()) {
    throw std::overflow_error("cityjson_lib index does not fit into uint32_t");
  }
  return static_cast<std::uint32_t>(index);
}

inline cj_string_view_t optional_view(const std::optional<std::string_view>& value) noexcept {
  return value.has_value() ? to_view(*value) : cj_string_view_t{};
}

}  // namespace detail

#define CITYJSON_LIB_DEFINE_ID_WRAPPER(Name, RawName)                \
  class Name final {                                                 \
   public:                                                           \
    Name() = default;                                                \
                                                                     \
    [[nodiscard]] bool valid() const noexcept {                      \
      return detail::id_valid(raw_);                                 \
    }                                                                \
                                                                     \
    friend bool operator==(const Name&, const Name&) = default;      \
                                                                     \
   private:                                                          \
    explicit Name(RawName raw) : raw_(raw) {}                        \
                                                                     \
    [[nodiscard]] RawName raw() const noexcept { return raw_; }      \
                                                                     \
    RawName raw_{};                                                  \
                                                                     \
    friend class Model;                                              \
    friend class Value;                                              \
    friend class RingDraft;                                          \
    friend class SurfaceDraft;                                       \
    friend class GeometryDraft;                                      \
  }

CITYJSON_LIB_DEFINE_ID_WRAPPER(CityObjectId, cj_cityobject_id_t);
CITYJSON_LIB_DEFINE_ID_WRAPPER(GeometryId, cj_geometry_id_t);
CITYJSON_LIB_DEFINE_ID_WRAPPER(GeometryTemplateId, cj_geometry_template_id_t);
CITYJSON_LIB_DEFINE_ID_WRAPPER(SemanticId, cj_semantic_id_t);
CITYJSON_LIB_DEFINE_ID_WRAPPER(MaterialId, cj_material_id_t);
CITYJSON_LIB_DEFINE_ID_WRAPPER(TextureId, cj_texture_id_t);

#undef CITYJSON_LIB_DEFINE_ID_WRAPPER

class Value final {
 public:
  Value() = default;

  Value(const Value&) = delete;
  Value& operator=(const Value&) = delete;
  Value(Value&&) noexcept = default;
  Value& operator=(Value&&) noexcept = default;

  [[nodiscard]] static Value null() {
    cj_value_t* handle = nullptr;
    check_status(cj_value_new_null(&handle));
    return Value(handle);
  }

  [[nodiscard]] static Value boolean(bool value) {
    cj_value_t* handle = nullptr;
    check_status(cj_value_new_bool(value, &handle));
    return Value(handle);
  }

  [[nodiscard]] static Value integer(std::int64_t value) {
    cj_value_t* handle = nullptr;
    check_status(cj_value_new_int64(value, &handle));
    return Value(handle);
  }

  [[nodiscard]] static Value number(double value) {
    cj_value_t* handle = nullptr;
    check_status(cj_value_new_float64(value, &handle));
    return Value(handle);
  }

  [[nodiscard]] static Value string(std::string_view value) {
    cj_value_t* handle = nullptr;
    check_status(cj_value_new_string(to_view(value), &handle));
    return Value(handle);
  }

  [[nodiscard]] static Value geometry(GeometryId value) {
    cj_value_t* handle = nullptr;
    check_status(cj_value_new_geometry_ref(value.raw(), &handle));
    return Value(handle);
  }

  [[nodiscard]] static Value array() {
    cj_value_t* handle = nullptr;
    check_status(cj_value_new_array(&handle));
    return Value(handle);
  }

  [[nodiscard]] static Value object() {
    cj_value_t* handle = nullptr;
    check_status(cj_value_new_object(&handle));
    return Value(handle);
  }

  Value& push(Value value) & {
    check_status(cj_value_array_push(handle_.get(), value.release()));
    return *this;
  }

  Value&& push(Value value) && {
    check_status(cj_value_array_push(handle_.get(), value.release()));
    return std::move(*this);
  }

  Value& insert(std::string_view key, Value value) & {
    check_status(cj_value_object_insert(handle_.get(), to_view(key), value.release()));
    return *this;
  }

  Value&& insert(std::string_view key, Value value) && {
    check_status(cj_value_object_insert(handle_.get(), to_view(key), value.release()));
    return std::move(*this);
  }

 private:
  explicit Value(cj_value_t* handle) : handle_(handle) {}

  [[nodiscard]] cj_value_t* release() noexcept { return handle_.release(); }

  detail::OwnedHandle<cj_value_t, cj_value_free> handle_;

  friend class Contact;
  friend class Model;
  friend class CityObjectDraft;
};

class Contact final {
 public:
  Contact() {
    cj_contact_t* handle = nullptr;
    check_status(cj_contact_new(&handle));
    handle_.reset(handle);
  }

  Contact(const Contact&) = delete;
  Contact& operator=(const Contact&) = delete;
  Contact(Contact&&) noexcept = default;
  Contact& operator=(Contact&&) noexcept = default;

  Contact& set_name(std::string_view value) & {
    check_status(cj_contact_set_name(handle_.get(), to_view(value)));
    return *this;
  }

  Contact&& set_name(std::string_view value) && {
    check_status(cj_contact_set_name(handle_.get(), to_view(value)));
    return std::move(*this);
  }

  Contact& set_email(std::string_view value) & {
    check_status(cj_contact_set_email(handle_.get(), to_view(value)));
    return *this;
  }

  Contact&& set_email(std::string_view value) && {
    check_status(cj_contact_set_email(handle_.get(), to_view(value)));
    return std::move(*this);
  }

  Contact& set_role(ContactRole value) & {
    check_status(cj_contact_set_role(handle_.get(), value));
    return *this;
  }

  Contact&& set_role(ContactRole value) && {
    check_status(cj_contact_set_role(handle_.get(), value));
    return std::move(*this);
  }

  Contact& set_website(std::string_view value) & {
    check_status(cj_contact_set_website(handle_.get(), to_view(value)));
    return *this;
  }

  Contact&& set_website(std::string_view value) && {
    check_status(cj_contact_set_website(handle_.get(), to_view(value)));
    return std::move(*this);
  }

  Contact& set_type(ContactType value) & {
    check_status(cj_contact_set_type(handle_.get(), value));
    return *this;
  }

  Contact&& set_type(ContactType value) && {
    check_status(cj_contact_set_type(handle_.get(), value));
    return std::move(*this);
  }

  Contact& set_phone(std::string_view value) & {
    check_status(cj_contact_set_phone(handle_.get(), to_view(value)));
    return *this;
  }

  Contact&& set_phone(std::string_view value) && {
    check_status(cj_contact_set_phone(handle_.get(), to_view(value)));
    return std::move(*this);
  }

  Contact& set_organization(std::string_view value) & {
    check_status(cj_contact_set_organization(handle_.get(), to_view(value)));
    return *this;
  }

  Contact&& set_organization(std::string_view value) && {
    check_status(cj_contact_set_organization(handle_.get(), to_view(value)));
    return std::move(*this);
  }

  Contact& set_address(Value value) & {
    check_status(cj_contact_set_address(handle_.get(), value.release()));
    return *this;
  }

  Contact&& set_address(Value value) && {
    check_status(cj_contact_set_address(handle_.get(), value.release()));
    return std::move(*this);
  }

 private:
  [[nodiscard]] cj_contact_t* release() noexcept { return handle_.release(); }

  detail::OwnedHandle<cj_contact_t, cj_contact_free> handle_;

  friend class Model;
};

class CityObjectDraft final {
 public:
  CityObjectDraft(std::string_view id, std::string_view type) {
    cj_cityobject_draft_t* handle = nullptr;
    check_status(cj_cityobject_draft_new(to_view(id), to_view(type), &handle));
    handle_.reset(handle);
  }

  CityObjectDraft(const CityObjectDraft&) = delete;
  CityObjectDraft& operator=(const CityObjectDraft&) = delete;
  CityObjectDraft(CityObjectDraft&&) noexcept = default;
  CityObjectDraft& operator=(CityObjectDraft&&) noexcept = default;

  CityObjectDraft& set_geographical_extent(const BBox& bbox) & {
    check_status(cj_cityobject_draft_set_geographical_extent(handle_.get(), bbox));
    return *this;
  }

  CityObjectDraft&& set_geographical_extent(const BBox& bbox) && {
    check_status(cj_cityobject_draft_set_geographical_extent(handle_.get(), bbox));
    return std::move(*this);
  }

  CityObjectDraft& set_attribute(std::string_view key, Value value) & {
    check_status(cj_cityobject_draft_set_attribute(handle_.get(), to_view(key), value.release()));
    return *this;
  }

  CityObjectDraft&& set_attribute(std::string_view key, Value value) && {
    check_status(cj_cityobject_draft_set_attribute(handle_.get(), to_view(key), value.release()));
    return std::move(*this);
  }

  CityObjectDraft& set_extra(std::string_view key, Value value) & {
    check_status(cj_cityobject_draft_set_extra(handle_.get(), to_view(key), value.release()));
    return *this;
  }

  CityObjectDraft&& set_extra(std::string_view key, Value value) && {
    check_status(cj_cityobject_draft_set_extra(handle_.get(), to_view(key), value.release()));
    return std::move(*this);
  }

 private:
  [[nodiscard]] cj_cityobject_draft_t* release() noexcept { return handle_.release(); }

  detail::OwnedHandle<cj_cityobject_draft_t, cj_cityobject_draft_free> handle_;

  friend class Model;
};

class RingDraft final {
 public:
  RingDraft() {
    cj_ring_draft_t* handle = nullptr;
    check_status(cj_ring_draft_new(&handle));
    handle_.reset(handle);
  }

  RingDraft(const RingDraft&) = delete;
  RingDraft& operator=(const RingDraft&) = delete;
  RingDraft(RingDraft&&) noexcept = default;
  RingDraft& operator=(RingDraft&&) noexcept = default;

  RingDraft& push_vertex_index(std::uint32_t index) & {
    check_status(cj_ring_draft_push_vertex_index(handle_.get(), index));
    return *this;
  }

  RingDraft&& push_vertex_index(std::uint32_t index) && {
    check_status(cj_ring_draft_push_vertex_index(handle_.get(), index));
    return std::move(*this);
  }

  RingDraft& push_vertex(const Vertex& vertex) & {
    check_status(cj_ring_draft_push_vertex(handle_.get(), vertex));
    return *this;
  }

  RingDraft&& push_vertex(const Vertex& vertex) && {
    check_status(cj_ring_draft_push_vertex(handle_.get(), vertex));
    return std::move(*this);
  }

  RingDraft& add_texture(
      std::string_view theme,
      TextureId texture,
      std::span<const std::uint32_t> uv_indices) & {
    check_status(cj_ring_draft_add_texture(
        handle_.get(),
        to_view(theme),
        texture.raw(),
        uv_indices.empty() ? nullptr : uv_indices.data(),
        uv_indices.size()));
    return *this;
  }

  RingDraft& add_texture(
      std::string_view theme,
      TextureId texture,
      std::span<const UV> uvs) & {
    check_status(cj_ring_draft_add_texture_uvs(
        handle_.get(),
        to_view(theme),
        texture.raw(),
        uvs.empty() ? nullptr : uvs.data(),
        uvs.size()));
    return *this;
  }

  RingDraft&& add_texture(
      std::string_view theme,
      TextureId texture,
      std::span<const std::uint32_t> uv_indices) && {
    check_status(cj_ring_draft_add_texture(
        handle_.get(),
        to_view(theme),
        texture.raw(),
        uv_indices.empty() ? nullptr : uv_indices.data(),
        uv_indices.size()));
    return std::move(*this);
  }

  RingDraft&& add_texture(
      std::string_view theme,
      TextureId texture,
      std::span<const UV> uvs) && {
    check_status(cj_ring_draft_add_texture_uvs(
        handle_.get(),
        to_view(theme),
        texture.raw(),
        uvs.empty() ? nullptr : uvs.data(),
        uvs.size()));
    return std::move(*this);
  }

 private:
  [[nodiscard]] cj_ring_draft_t* release() noexcept { return handle_.release(); }

  detail::OwnedHandle<cj_ring_draft_t, cj_ring_draft_free> handle_;

  friend class SurfaceDraft;
};

class SurfaceDraft final {
 public:
  explicit SurfaceDraft(RingDraft outer) {
    cj_surface_draft_t* handle = nullptr;
    check_status(cj_surface_draft_new(outer.release(), &handle));
    handle_.reset(handle);
  }

  SurfaceDraft(const SurfaceDraft&) = delete;
  SurfaceDraft& operator=(const SurfaceDraft&) = delete;
  SurfaceDraft(SurfaceDraft&&) noexcept = default;
  SurfaceDraft& operator=(SurfaceDraft&&) noexcept = default;

  SurfaceDraft& add_inner_ring(RingDraft inner) & {
    check_status(cj_surface_draft_add_inner_ring(handle_.get(), inner.release()));
    return *this;
  }

  SurfaceDraft&& add_inner_ring(RingDraft inner) && {
    check_status(cj_surface_draft_add_inner_ring(handle_.get(), inner.release()));
    return std::move(*this);
  }

  SurfaceDraft& set_semantic(SemanticId semantic) & {
    check_status(cj_surface_draft_set_semantic(handle_.get(), semantic.raw()));
    return *this;
  }

  SurfaceDraft&& set_semantic(SemanticId semantic) && {
    check_status(cj_surface_draft_set_semantic(handle_.get(), semantic.raw()));
    return std::move(*this);
  }

  SurfaceDraft& add_material(std::string_view theme, MaterialId material) & {
    check_status(cj_surface_draft_add_material(handle_.get(), to_view(theme), material.raw()));
    return *this;
  }

  SurfaceDraft&& add_material(std::string_view theme, MaterialId material) && {
    check_status(cj_surface_draft_add_material(handle_.get(), to_view(theme), material.raw()));
    return std::move(*this);
  }

 private:
  [[nodiscard]] cj_surface_draft_t* release() noexcept { return handle_.release(); }

  detail::OwnedHandle<cj_surface_draft_t, cj_surface_draft_free> handle_;

  friend class ShellDraft;
  friend class GeometryDraft;
};

class ShellDraft final {
 public:
  ShellDraft() {
    cj_shell_draft_t* handle = nullptr;
    check_status(cj_shell_draft_new(&handle));
    handle_.reset(handle);
  }

  ShellDraft(const ShellDraft&) = delete;
  ShellDraft& operator=(const ShellDraft&) = delete;
  ShellDraft(ShellDraft&&) noexcept = default;
  ShellDraft& operator=(ShellDraft&&) noexcept = default;

  ShellDraft& add_surface(SurfaceDraft surface) & {
    check_status(cj_shell_draft_add_surface(handle_.get(), surface.release()));
    return *this;
  }

  ShellDraft&& add_surface(SurfaceDraft surface) && {
    check_status(cj_shell_draft_add_surface(handle_.get(), surface.release()));
    return std::move(*this);
  }

 private:
  [[nodiscard]] cj_shell_draft_t* release() noexcept { return handle_.release(); }

  detail::OwnedHandle<cj_shell_draft_t, cj_shell_draft_free> handle_;

  friend class GeometryDraft;
};

class GeometryDraft final {
 public:
  GeometryDraft(const GeometryDraft&) = delete;
  GeometryDraft& operator=(const GeometryDraft&) = delete;
  GeometryDraft(GeometryDraft&&) noexcept = default;
  GeometryDraft& operator=(GeometryDraft&&) noexcept = default;

  [[nodiscard]] static GeometryDraft multi_point(
      std::optional<std::string_view> lod = std::nullopt) {
    return GeometryDraft(CJ_GEOMETRY_TYPE_MULTI_POINT, lod);
  }

  [[nodiscard]] static GeometryDraft multi_line_string(
      std::optional<std::string_view> lod = std::nullopt) {
    return GeometryDraft(CJ_GEOMETRY_TYPE_MULTI_LINE_STRING, lod);
  }

  [[nodiscard]] static GeometryDraft multi_surface(
      std::optional<std::string_view> lod = std::nullopt) {
    return GeometryDraft(CJ_GEOMETRY_TYPE_MULTI_SURFACE, lod);
  }

  [[nodiscard]] static GeometryDraft composite_surface(
      std::optional<std::string_view> lod = std::nullopt) {
    return GeometryDraft(CJ_GEOMETRY_TYPE_COMPOSITE_SURFACE, lod);
  }

  [[nodiscard]] static GeometryDraft solid(std::optional<std::string_view> lod = std::nullopt) {
    return GeometryDraft(CJ_GEOMETRY_TYPE_SOLID, lod);
  }

  [[nodiscard]] static GeometryDraft multi_solid(
      std::optional<std::string_view> lod = std::nullopt) {
    return GeometryDraft(CJ_GEOMETRY_TYPE_MULTI_SOLID, lod);
  }

  [[nodiscard]] static GeometryDraft composite_solid(
      std::optional<std::string_view> lod = std::nullopt) {
    return GeometryDraft(CJ_GEOMETRY_TYPE_COMPOSITE_SOLID, lod);
  }

  [[nodiscard]] static GeometryDraft instance(
      GeometryTemplateId template_id,
      std::uint32_t reference_vertex_index,
      const AffineTransform4x4& transform) {
    cj_geometry_draft_t* handle = nullptr;
    check_status(cj_geometry_draft_new_instance(
        template_id.raw(), reference_vertex_index, to_native(transform), &handle));
    return GeometryDraft(handle);
  }

  GeometryDraft& add_point(
      std::uint32_t vertex_index,
      std::optional<SemanticId> semantic = std::nullopt) & {
    const auto* semantic_ptr =
        semantic.has_value() ? &semantic->raw_ : nullptr;
    check_status(cj_geometry_draft_add_point_vertex_index(handle_.get(), vertex_index, semantic_ptr));
    return *this;
  }

  GeometryDraft&& add_point(
      std::uint32_t vertex_index,
      std::optional<SemanticId> semantic = std::nullopt) && {
    const auto* semantic_ptr =
        semantic.has_value() ? &semantic->raw_ : nullptr;
    check_status(cj_geometry_draft_add_point_vertex_index(handle_.get(), vertex_index, semantic_ptr));
    return std::move(*this);
  }

  GeometryDraft& add_linestring(
      std::span<const std::uint32_t> vertex_indices,
      std::optional<SemanticId> semantic = std::nullopt) & {
    const auto* semantic_ptr =
        semantic.has_value() ? &semantic->raw_ : nullptr;
    check_status(cj_geometry_draft_add_linestring(
        handle_.get(),
        vertex_indices.empty() ? nullptr : vertex_indices.data(),
        vertex_indices.size(),
        semantic_ptr));
    return *this;
  }

  GeometryDraft&& add_linestring(
      std::span<const std::uint32_t> vertex_indices,
      std::optional<SemanticId> semantic = std::nullopt) && {
    const auto* semantic_ptr =
        semantic.has_value() ? &semantic->raw_ : nullptr;
    check_status(cj_geometry_draft_add_linestring(
        handle_.get(),
        vertex_indices.empty() ? nullptr : vertex_indices.data(),
        vertex_indices.size(),
        semantic_ptr));
    return std::move(*this);
  }

  GeometryDraft& add_surface(SurfaceDraft surface) & {
    check_status(cj_geometry_draft_add_surface(handle_.get(), surface.release()));
    return *this;
  }

  GeometryDraft&& add_surface(SurfaceDraft surface) && {
    check_status(cj_geometry_draft_add_surface(handle_.get(), surface.release()));
    return std::move(*this);
  }

  GeometryDraft& add_solid(ShellDraft outer, std::vector<ShellDraft> inner_shells = {}) & {
    cj_solid_draft_t* solid = nullptr;
    check_status(cj_solid_draft_new(outer.release(), &solid));
    detail::OwnedHandle<cj_solid_draft_t, cj_solid_draft_free> solid_handle(solid);
    for (auto& inner : inner_shells) {
      check_status(cj_solid_draft_add_inner_shell(solid_handle.get(), inner.release()));
    }
    check_status(cj_geometry_draft_add_solid(handle_.get(), solid_handle.release()));
    return *this;
  }

  GeometryDraft&& add_solid(ShellDraft outer, std::vector<ShellDraft> inner_shells = {}) && {
    cj_solid_draft_t* solid = nullptr;
    check_status(cj_solid_draft_new(outer.release(), &solid));
    detail::OwnedHandle<cj_solid_draft_t, cj_solid_draft_free> solid_handle(solid);
    for (auto& inner : inner_shells) {
      check_status(cj_solid_draft_add_inner_shell(solid_handle.get(), inner.release()));
    }
    check_status(cj_geometry_draft_add_solid(handle_.get(), solid_handle.release()));
    return std::move(*this);
  }

 private:
  GeometryDraft(GeometryType geometry_type, const std::optional<std::string_view>& lod) {
    cj_geometry_draft_t* handle = nullptr;
    check_status(cj_geometry_draft_new(geometry_type, detail::optional_view(lod), &handle));
    handle_.reset(handle);
  }

  explicit GeometryDraft(cj_geometry_draft_t* handle) : handle_(handle) {}

  [[nodiscard]] cj_geometry_draft_t* release() noexcept { return handle_.release(); }

  detail::OwnedHandle<cj_geometry_draft_t, cj_geometry_draft_free> handle_;

  friend class Model;
};

class ModelSelection;

class ProjTransformer final {
 public:
  ProjTransformer() = default;

  ProjTransformer(const ProjTransformer&) = delete;
  ProjTransformer& operator=(const ProjTransformer&) = delete;

  ProjTransformer(ProjTransformer&& other) noexcept
      : handle_(std::exchange(other.handle_, nullptr)) {}

  ProjTransformer& operator=(ProjTransformer&& other) noexcept {
    if (this != &other) {
      reset();
      handle_ = std::exchange(other.handle_, nullptr);
    }
    return *this;
  }

  ~ProjTransformer() { reset(); }

  [[nodiscard]] static ProjTransformer create(
      std::string_view source_crs,
      std::string_view target_crs) {
    cj_proj_transformer_t* handle = nullptr;
    check_status(cj_proj_transformer_create(to_view(source_crs), to_view(target_crs), &handle));
    return ProjTransformer(handle);
  }

  void reset() noexcept {
    if (handle_ != nullptr) {
      static_cast<void>(cj_proj_transformer_free(handle_));
      handle_ = nullptr;
    }
  }

  [[nodiscard]] bool valid() const noexcept { return handle_ != nullptr; }

  [[nodiscard]] Vertex transform(const Vertex& point) const {
    Vertex transformed{};
    check_status(cj_proj_transformer_transform(handle_, point, &transformed));
    return transformed;
  }

 private:
  explicit ProjTransformer(cj_proj_transformer_t* handle) : handle_(handle) {}

  cj_proj_transformer_t* handle_ = nullptr;
};

class Model final {
 public:
  Model() = default;

  explicit Model(cj_model_t* handle) : handle_(handle) {}

  Model(const Model&) = delete;
  Model& operator=(const Model&) = delete;

  Model(Model&& other) noexcept : handle_(std::exchange(other.handle_, nullptr)) {}

  Model& operator=(Model&& other) noexcept {
    if (this != &other) {
      reset();
      handle_ = std::exchange(other.handle_, nullptr);
    }
    return *this;
  }

  ~Model() { reset(); }

  [[nodiscard]] static Probe probe(std::span<const std::uint8_t> bytes) {
    Probe probe{};
    check_status(cj_probe_bytes(span_data(bytes), bytes.size(), &probe));
    return probe;
  }

  [[nodiscard]] static Model parse_document(std::span<const std::uint8_t> bytes) {
    cj_model_t* handle = nullptr;
    check_status(cj_model_parse_document_bytes(span_data(bytes), bytes.size(), &handle));
    return Model(handle);
  }

  [[nodiscard]] static Model parse_feature(std::span<const std::uint8_t> bytes) {
    cj_model_t* handle = nullptr;
    check_status(cj_model_parse_feature_bytes(span_data(bytes), bytes.size(), &handle));
    return Model(handle);
  }

  [[nodiscard]] static Model parse_feature_with_base(
      std::span<const std::uint8_t> feature_bytes,
      std::span<const std::uint8_t> base_bytes) {
    cj_model_t* handle = nullptr;
    check_status(cj_model_parse_feature_with_base_bytes(
        span_data(feature_bytes), feature_bytes.size(), span_data(base_bytes), base_bytes.size(),
        &handle));
    return Model(handle);
  }

  [[nodiscard]] static Model parse_arrow(std::span<const std::uint8_t> bytes) {
    cj_model_t* handle = nullptr;
    check_status(cj_model_parse_arrow_bytes(span_data(bytes), bytes.size(), &handle));
    return Model(handle);
  }

  [[nodiscard]] static Model parse_parquet_file(std::string_view path) {
    cj_model_t* handle = nullptr;
    check_status(cj_model_parse_parquet_file(to_view(path), &handle));
    return Model(handle);
  }

  [[nodiscard]] static Model parse_parquet_dataset_dir(std::string_view path) {
    cj_model_t* handle = nullptr;
    check_status(cj_model_parse_parquet_dataset_dir(to_view(path), &handle));
    return Model(handle);
  }

  [[nodiscard]] static Model create(ModelType type) {
    cj_model_t* handle = nullptr;
    check_status(cj_model_create(type, &handle));
    return Model(handle);
  }

  [[nodiscard]] bool valid() const noexcept { return handle_ != nullptr; }

  void reset() noexcept {
    if (handle_ != nullptr) {
      static_cast<void>(cj_model_free(handle_));
      handle_ = nullptr;
    }
  }

  [[nodiscard]] ModelSummary summary() const {
    ModelSummary summary{};
    check_status(cj_model_get_summary(handle_, &summary));
    return summary;
  }

  [[nodiscard]] std::string metadata_title() const {
    cj_bytes_t bytes{};
    check_status(cj_model_get_metadata_title(handle_, &bytes));
    return take_string(bytes);
  }

  [[nodiscard]] std::string metadata_identifier() const {
    cj_bytes_t bytes{};
    check_status(cj_model_get_metadata_identifier(handle_, &bytes));
    return take_string(bytes);
  }

  void set_metadata_title(std::string_view title) {
    check_status(cj_model_set_metadata_title(handle_, to_view(title)));
  }

  void set_metadata_identifier(std::string_view identifier) {
    check_status(cj_model_set_metadata_identifier(handle_, to_view(identifier)));
  }

  void set_metadata_geographical_extent(const BBox& bbox) {
    check_status(cj_model_set_metadata_geographical_extent(handle_, bbox));
  }

  void set_metadata_reference_date(std::string_view value) {
    check_status(cj_model_set_metadata_reference_date(handle_, to_view(value)));
  }

  void set_metadata_reference_system(std::string_view value) {
    check_status(cj_model_set_metadata_reference_system(handle_, to_view(value)));
  }

  void set_metadata_contact(Contact contact) {
    check_status(cj_model_set_metadata_contact(handle_, contact.release()));
  }

  void set_metadata_extra(std::string_view key, Value value) {
    check_status(cj_model_set_metadata_extra(handle_, to_view(key), value.release()));
  }

  void set_root_extra(std::string_view key, Value value) {
    check_status(cj_model_set_root_extra(handle_, to_view(key), value.release()));
  }

  void add_extension(std::string_view name, std::string_view url, std::string_view version) {
    check_status(cj_model_add_extension(handle_, to_view(name), to_view(url), to_view(version)));
  }

  void set_transform(const Transform& transform) {
    check_status(cj_model_set_transform(handle_, to_native(transform)));
  }

  void clear_transform() {
    check_status(cj_model_clear_transform(handle_));
  }

  void reproject(std::string_view target_crs) {
    check_status(cj_model_reproject(handle_, to_view(target_crs)));
  }

  [[nodiscard]] std::vector<std::string> cityobject_ids() const {
    cj_bytes_list_t ids{};
    check_status(cj_model_copy_cityobject_ids(handle_, &ids));
    return take_string_list(ids);
  }

  void remove_cityobject(std::string_view id) {
    check_status(cj_model_remove_cityobject(handle_, to_view(id)));
  }

  [[nodiscard]] std::vector<GeometryType> geometry_types() const {
    cj_geometry_types_t types{};
    check_status(cj_model_copy_geometry_types(handle_, &types));
    return take_geometry_types(types);
  }

  [[nodiscard]] std::vector<UV> uv_coordinates() const {
    cj_uvs_t uvs{};
    check_status(cj_model_copy_uv_coordinates(handle_, &uvs));
    return take_uvs(uvs);
  }

  [[nodiscard]] GeometryBoundary geometry_boundary(std::size_t index) const {
    cj_geometry_boundary_t boundary{};
    check_status(cj_model_copy_geometry_boundary(handle_, index, &boundary));
    return take_geometry_boundary(boundary);
  }

  [[nodiscard]] std::vector<Vertex> geometry_boundary_coordinates(std::size_t index) const {
    cj_vertices_t vertices{};
    check_status(cj_model_copy_geometry_boundary_coordinates(handle_, index, &vertices));
    return take_vertices(vertices);
  }

  [[nodiscard]] std::vector<std::uint8_t> serialize_document_bytes(
      const WriteOptions& options = {}) const {
    cj_bytes_t bytes{};
    check_status(cj_model_serialize_document_with_options(handle_, to_native(options), &bytes));
    return take_bytes(bytes);
  }

  [[nodiscard]] std::string serialize_document(const WriteOptions& options = {}) const {
    const auto bytes = serialize_document_bytes(options);
    return std::string(bytes.begin(), bytes.end());
  }

  [[nodiscard]] std::vector<std::uint8_t> serialize_feature_bytes(
      const WriteOptions& options = {}) const {
    cj_bytes_t bytes{};
    check_status(cj_model_serialize_feature_with_options(handle_, to_native(options), &bytes));
    return take_bytes(bytes);
  }

  [[nodiscard]] std::string serialize_feature(const WriteOptions& options = {}) const {
    const auto bytes = serialize_feature_bytes(options);
    return std::string(bytes.begin(), bytes.end());
  }

  [[nodiscard]] std::vector<std::uint8_t> serialize_arrow_bytes() const {
    cj_bytes_t bytes{};
    check_status(cj_model_serialize_arrow_bytes(handle_, &bytes));
    return take_bytes(bytes);
  }

  void serialize_parquet_file(std::string_view path) const {
    check_status(cj_model_serialize_parquet_file(handle_, to_view(path)));
  }

  void serialize_parquet_dataset_dir(std::string_view path) const {
    check_status(cj_model_serialize_parquet_dataset_dir(handle_, to_view(path)));
  }

  [[nodiscard]] static std::vector<std::uint8_t> serialize_feature_stream(
      std::span<const Model* const> models,
      const WriteOptions& options = {}) {
    std::vector<const cj_model_t*> handles;
    handles.reserve(models.size());
    for (const Model* model : models) {
      handles.push_back(model->raw_handle());
    }

    cj_bytes_t bytes{};
    check_status(cj_model_serialize_feature_stream(
        handles.empty() ? nullptr : handles.data(), handles.size(), to_native(options), &bytes));
    return take_bytes(bytes);
  }

  [[nodiscard]] static Model merge_feature_stream(std::span<const std::uint8_t> bytes) {
    cj_model_t* handle = nullptr;
    check_status(cj_model_parse_feature_stream_merge_bytes(
        span_data(bytes), bytes.size(), &handle));
    return Model(handle);
  }

  [[nodiscard]] static Model merge_models(std::span<const Model* const> models) {
    std::vector<const cj_model_t*> handles;
    handles.reserve(models.size());
    for (const Model* model : models) {
      handles.push_back(model->raw_handle());
    }

    cj_model_t* handle = nullptr;
    check_status(cj_model_merge_models(
        handles.empty() ? nullptr : handles.data(), handles.size(), &handle));
    return Model(handle);
  }

  [[nodiscard]] Model subset_cityobjects(
      std::span<const std::string_view> ids,
      bool exclude = false) const {
    std::vector<cj_string_view_t> views;
    views.reserve(ids.size());
    for (const std::string_view id : ids) {
      views.push_back(to_view(id));
    }

    cj_model_t* handle = nullptr;
    check_status(cj_model_subset_cityobjects(
        handle_, views.empty() ? nullptr : views.data(), views.size(), exclude, &handle));
    return Model(handle);
  }

  [[nodiscard]] Model extract_selection(const ModelSelection& selection) const;

  void append_model(const Model& source) {
    check_status(cj_model_append_model(handle_, source.raw_handle()));
  }

  void cleanup() {
    check_status(cj_model_cleanup(handle_));
  }

  void reserve_import(const ModelCapacities& capacities) {
    check_status(cj_model_reserve_import(handle_, capacities));
  }

  [[nodiscard]] std::uint32_t add_vertex(const Vertex& vertex) {
    std::size_t index = 0U;
    check_status(cj_model_add_vertex(handle_, vertex, &index));
    return detail::narrow_index(index);
  }

  [[nodiscard]] std::uint32_t add_template_vertex(const Vertex& vertex) {
    std::size_t index = 0U;
    check_status(cj_model_add_template_vertex(handle_, vertex, &index));
    return detail::narrow_index(index);
  }

  void set_vertex(std::size_t index, const Vertex& vertex) {
    check_status(cj_model_set_vertex(handle_, index, vertex));
  }

  void set_template_vertex(std::size_t index, const Vertex& vertex) {
    check_status(cj_model_set_template_vertex(handle_, index, vertex));
  }

  [[nodiscard]] std::uint32_t add_uv_coordinate(const UV& uv) {
    std::size_t index = 0U;
    check_status(cj_model_add_uv_coordinate(handle_, uv, &index));
    return detail::narrow_index(index);
  }

  [[nodiscard]] SemanticId add_semantic(std::string_view semantic_type) {
    cj_semantic_id_t id{};
    check_status(cj_model_add_semantic(handle_, to_view(semantic_type), &id));
    return SemanticId(id);
  }

  void set_semantic_parent(SemanticId semantic, SemanticId parent) {
    check_status(cj_model_set_semantic_parent(handle_, semantic.raw(), parent.raw()));
  }

  void set_semantic_extra(SemanticId semantic, std::string_view key, Value value) {
    check_status(cj_model_semantic_set_extra(handle_, semantic.raw(), to_view(key), value.release()));
  }

  [[nodiscard]] MaterialId add_material(std::string_view name) {
    cj_material_id_t id{};
    check_status(cj_model_add_material(handle_, to_view(name), &id));
    return MaterialId(id);
  }

  void set_material_ambient_intensity(MaterialId id, float value) {
    check_status(cj_model_material_set_ambient_intensity(handle_, id.raw(), value));
  }

  void set_material_diffuse_color(MaterialId id, const Rgb& value) {
    check_status(cj_model_material_set_diffuse_color(handle_, id.raw(), value));
  }

  void set_material_emissive_color(MaterialId id, const Rgb& value) {
    check_status(cj_model_material_set_emissive_color(handle_, id.raw(), value));
  }

  void set_material_specular_color(MaterialId id, const Rgb& value) {
    check_status(cj_model_material_set_specular_color(handle_, id.raw(), value));
  }

  void set_material_shininess(MaterialId id, float value) {
    check_status(cj_model_material_set_shininess(handle_, id.raw(), value));
  }

  void set_material_transparency(MaterialId id, float value) {
    check_status(cj_model_material_set_transparency(handle_, id.raw(), value));
  }

  void set_material_is_smooth(MaterialId id, bool value) {
    check_status(cj_model_material_set_is_smooth(handle_, id.raw(), value));
  }

  [[nodiscard]] TextureId add_texture(std::string_view image, ImageType image_type) {
    cj_texture_id_t id{};
    check_status(cj_model_add_texture(handle_, to_view(image), image_type, &id));
    return TextureId(id);
  }

  void set_texture_wrap_mode(TextureId id, WrapMode value) {
    check_status(cj_model_texture_set_wrap_mode(handle_, id.raw(), value));
  }

  void set_texture_type(TextureId id, TextureType value) {
    check_status(cj_model_texture_set_texture_type(handle_, id.raw(), value));
  }

  void set_texture_border_color(TextureId id, const Rgba& value) {
    check_status(cj_model_texture_set_border_color(handle_, id.raw(), value));
  }

  void set_default_material_theme(std::string_view theme) {
    check_status(cj_model_set_default_material_theme(handle_, to_view(theme)));
  }

  void set_default_texture_theme(std::string_view theme) {
    check_status(cj_model_set_default_texture_theme(handle_, to_view(theme)));
  }

  [[nodiscard]] GeometryId add_geometry(GeometryDraft draft) {
    cj_geometry_id_t id{};
    check_status(cj_model_add_geometry(handle_, draft.release(), &id));
    return GeometryId(id);
  }

  [[nodiscard]] GeometryTemplateId add_geometry_template(GeometryDraft draft) {
    cj_geometry_template_id_t id{};
    check_status(cj_model_add_geometry_template(handle_, draft.release(), &id));
    return GeometryTemplateId(id);
  }

  [[nodiscard]] CityObjectId add_cityobject(CityObjectDraft draft) {
    cj_cityobject_id_t id{};
    check_status(cj_model_add_cityobject(handle_, draft.release(), &id));
    return CityObjectId(id);
  }

  void add_cityobject_geometry(CityObjectId cityobject, GeometryId geometry) {
    check_status(cj_model_cityobject_add_geometry(handle_, cityobject.raw(), geometry.raw()));
  }

  void add_cityobject_parent(CityObjectId child, CityObjectId parent) {
    check_status(cj_model_cityobject_add_parent(handle_, child.raw(), parent.raw()));
  }

  [[nodiscard]] cj_model_t* raw_handle() const noexcept { return handle_; }

 private:
  cj_model_t* handle_ = nullptr;
};

class ModelSelection final {
 public:
  ModelSelection() = default;

  ModelSelection(const ModelSelection&) = delete;
  ModelSelection& operator=(const ModelSelection&) = delete;
  ModelSelection(ModelSelection&&) noexcept = default;
  ModelSelection& operator=(ModelSelection&&) noexcept = default;

  [[nodiscard]] static ModelSelection select_cityobjects_by_id(
      const Model& model,
      std::span<const std::string_view> ids) {
    std::vector<cj_string_view_t> views;
    views.reserve(ids.size());
    for (const std::string_view id : ids) {
      views.push_back(to_view(id));
    }

    cj_model_selection_t* handle = nullptr;
    check_status(cj_model_select_cityobjects_by_id(
        model.raw_handle(), views.empty() ? nullptr : views.data(), views.size(), &handle));
    return ModelSelection(handle);
  }

  [[nodiscard]] static ModelSelection select_geometries_by_cityobject_id_and_index(
      const Model& model,
      std::span<const GeometrySelectionSpec> specs) {
    std::vector<cj_geometry_selection_spec_t> native_specs;
    native_specs.reserve(specs.size());
    for (const GeometrySelectionSpec& spec : specs) {
      native_specs.push_back(cj_geometry_selection_spec_t{
          .cityobject_id = to_view(spec.cityobject_id),
          .geometry_index = spec.geometry_index,
      });
    }

    cj_model_selection_t* handle = nullptr;
    check_status(cj_model_select_geometries_by_cityobject_id_and_index(
        model.raw_handle(),
        native_specs.empty() ? nullptr : native_specs.data(),
        native_specs.size(),
        &handle));
    return ModelSelection(handle);
  }

  [[nodiscard]] bool valid() const noexcept { return handle_.valid(); }

  [[nodiscard]] ModelSelection include_relatives(const Model& model) const {
    cj_model_selection_t* handle = nullptr;
    check_status(cj_model_selection_include_relatives(raw_handle(), model.raw_handle(), &handle));
    return ModelSelection(handle);
  }

  [[nodiscard]] ModelSelection union_with(const ModelSelection& other) const {
    cj_model_selection_t* handle = nullptr;
    check_status(cj_model_selection_union(raw_handle(), other.raw_handle(), &handle));
    return ModelSelection(handle);
  }

  [[nodiscard]] ModelSelection intersection_with(const ModelSelection& other) const {
    cj_model_selection_t* handle = nullptr;
    check_status(cj_model_selection_intersection(raw_handle(), other.raw_handle(), &handle));
    return ModelSelection(handle);
  }

  [[nodiscard]] bool is_empty() const {
    bool value = false;
    check_status(cj_model_selection_is_empty(raw_handle(), &value));
    return value;
  }

 private:
  explicit ModelSelection(cj_model_selection_t* handle) : handle_(handle) {}

  [[nodiscard]] cj_model_selection_t* raw_handle() const noexcept { return handle_.get(); }

  detail::OwnedHandle<cj_model_selection_t, cj_model_selection_free> handle_;

  friend class Model;
};

inline Model Model::extract_selection(const ModelSelection& selection) const {
  cj_model_t* handle = nullptr;
  check_status(cj_model_extract_selection(handle_, selection.raw_handle(), &handle));
  return Model(handle);
}

}  // namespace cityjson_lib
