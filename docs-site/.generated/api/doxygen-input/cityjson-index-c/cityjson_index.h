#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef enum cjx_package_type_t {
  CJX_PACKAGE_TYPE_CITYJSON = 0,
  CJX_PACKAGE_TYPE_CITYJSON_SEQ = 1,
  CJX_PACKAGE_TYPE_FEATURE_FILES = 2,
} cjx_package_type_t;

typedef enum cjx_lod_selection_kind_t {
  CJX_LOD_SELECTION_ALL = 0,
  CJX_LOD_SELECTION_HIGHEST = 1,
  CJX_LOD_SELECTION_EXACT = 2,
} cjx_lod_selection_kind_t;

typedef struct cjx_index_t {
  uint8_t _private[0];
} cjx_index_t;

typedef struct cjx_index_status_t {
  bool exists;
  bool needs_reindex;
  uintptr_t indexed_feature_count;
  uintptr_t indexed_source_count;
} cjx_index_status_t;

typedef struct cjx_bounds3d_t {
  double min_x;
  double max_x;
  double min_y;
  double max_y;
  double min_z;
  double max_z;
} cjx_bounds3d_t;

typedef struct cjx_feature_bounds_summary_t {
  uintptr_t package_count;
  uintptr_t cityobject_count;
  bool has_bounds;
  struct cjx_bounds3d_t bounds;
} cjx_feature_bounds_summary_t;

typedef struct cjx_package_ref_t {
  int64_t record_id;
  cj_bytes_t model_id;
  enum cjx_package_type_t package_type;
  bool has_bounds;
  struct cjx_bounds3d_t bounds;
} cjx_package_ref_t;

typedef struct cjx_cityobject_ref_t {
  int64_t record_id;
  cj_bytes_t external_id;
  cj_bytes_t cityobject_type;
  bool has_bounds;
  struct cjx_bounds3d_t bounds;
} cjx_cityobject_ref_t;

typedef struct cjx_bounds2d_t {
  double min_x;
  double max_x;
  double min_y;
  double max_y;
} cjx_bounds2d_t;

typedef struct cjx_string_list_t {
  cj_bytes_t *data;
  uintptr_t len;
} cjx_string_list_t;

typedef struct cjx_lod_selection_t {
  enum cjx_lod_selection_kind_t kind;
  cj_bytes_t exact_lod;
} cjx_lod_selection_t;

typedef struct cjx_lod_by_type_t {
  cj_bytes_t cityobject_type;
  struct cjx_lod_selection_t selection;
} cjx_lod_by_type_t;

typedef struct cjx_package_filter_t {
  bool has_cityobject_types;
  struct cjx_string_list_t cityobject_types;
  struct cjx_lod_selection_t default_lod;
  struct cjx_lod_by_type_t *lods_by_type;
  uintptr_t lods_by_type_len;
} cjx_package_filter_t;

typedef struct cjx_lod_map_entry_t {
  cj_bytes_t cityobject_type;
  struct cjx_string_list_t lods;
} cjx_lod_map_entry_t;

typedef struct cjx_lod_map_t {
  struct cjx_lod_map_entry_t *data;
  uintptr_t len;
} cjx_lod_map_t;

typedef struct cjx_missing_lod_selection_t {
  cj_bytes_t cityobject_type;
  cj_bytes_t requested_lod;
  struct cjx_string_list_t available_lods;
} cjx_missing_lod_selection_t;

typedef struct cjx_missing_lod_selection_list_t {
  struct cjx_missing_lod_selection_t *data;
  uintptr_t len;
} cjx_missing_lod_selection_list_t;

typedef struct cjx_package_filter_report_t {
  struct cjx_string_list_t available_types;
  struct cjx_string_list_t retained_types;
  struct cjx_string_list_t ignored_types;
  struct cjx_lod_map_t available_lods;
  struct cjx_lod_map_t retained_lods;
  struct cjx_missing_lod_selection_list_t missing_lods;
  uintptr_t retained_geometry_count;
} cjx_package_filter_report_t;

typedef struct cjx_filtered_package_t {
  cj_bytes_t model_json;
  struct cjx_package_filter_report_t diagnostics;
} cjx_filtered_package_t;

cj_status_t cjx_clear_error(void);

cj_error_kind_t cjx_last_error_kind(void);

uintptr_t cjx_last_error_message_len(void);

/**
 * # Safety
 *
 * `buffer` must point to `capacity` writable bytes and `out_len` must be a
 * valid writable pointer when non-null.
 */
cj_status_t cjx_last_error_message_copy(char *buffer, uintptr_t capacity, uintptr_t *out_len);

cj_status_t cjx_bytes_free(cj_bytes_t bytes);

/**
 * # Safety
 *
 * `items` must either be null or point to `count` byte buffers allocated by this ABI.
 */
cj_status_t cjx_bytes_array_free(cj_bytes_t *items, uintptr_t count);

cj_status_t cjx_index_open(const char *dataset_dir,
                           uintptr_t dataset_dir_len,
                           const char *index_path,
                           uintptr_t index_path_len,
                           struct cjx_index_t **out_index);

cj_status_t cjx_index_free(struct cjx_index_t *handle);

cj_status_t cjx_index_status(const struct cjx_index_t *handle,
                             struct cjx_index_status_t *out_status);

cj_status_t cjx_index_reindex(struct cjx_index_t *handle);

cj_status_t cjx_index_feature_bounds_summary(const struct cjx_index_t *handle,
                                             struct cjx_feature_bounds_summary_t *out_summary);

cj_status_t cjx_index_package_ref_page_after_record_id(const struct cjx_index_t *handle,
                                                       bool has_after_record_id,
                                                       int64_t after_record_id,
                                                       uintptr_t limit,
                                                       struct cjx_package_ref_t **out_refs,
                                                       uintptr_t *out_count);

cj_status_t cjx_index_cityobject_ref_page_after_record_id(const struct cjx_index_t *handle,
                                                          bool has_after_record_id,
                                                          int64_t after_record_id,
                                                          uintptr_t limit,
                                                          struct cjx_cityobject_ref_t **out_refs,
                                                          uintptr_t *out_count);

cj_status_t cjx_index_query_package_refs_page(const struct cjx_index_t *handle,
                                              struct cjx_bounds2d_t bbox,
                                              bool has_after_record_id,
                                              int64_t after_record_id,
                                              uintptr_t limit,
                                              struct cjx_package_ref_t **out_refs,
                                              uintptr_t *out_count);

cj_status_t cjx_index_query_cityobject_refs_page(const struct cjx_index_t *handle,
                                                 struct cjx_bounds2d_t bbox,
                                                 bool has_after_record_id,
                                                 int64_t after_record_id,
                                                 uintptr_t limit,
                                                 struct cjx_cityobject_ref_t **out_refs,
                                                 uintptr_t *out_count);

cj_status_t cjx_index_lookup_package_ref_by_record_id(const struct cjx_index_t *handle,
                                                      int64_t record_id,
                                                      bool *out_found,
                                                      struct cjx_package_ref_t *out_ref);

cj_status_t cjx_index_lookup_cityobject_refs(const struct cjx_index_t *handle,
                                             const char *external_id,
                                             uintptr_t external_id_len,
                                             struct cjx_cityobject_ref_t **out_refs,
                                             uintptr_t *out_count);

cj_status_t cjx_index_package_refs_for_cityobject(const struct cjx_index_t *handle,
                                                  const struct cjx_cityobject_ref_t *cityobject,
                                                  struct cjx_package_ref_t **out_refs,
                                                  uintptr_t *out_count);

/**
 * # Safety
 *
 * `refs` must either be null or point to `count` `CityObject` refs allocated by this ABI.
 */
cj_status_t cjx_cityobject_refs_free(struct cjx_cityobject_ref_t *refs, uintptr_t count);

cj_status_t cjx_package_ref_free(struct cjx_package_ref_t value);

/**
 * # Safety
 *
 * `refs` must either be null or point to `count` package refs allocated by this ABI.
 */
cj_status_t cjx_package_refs_free(struct cjx_package_ref_t *refs, uintptr_t count);

cj_status_t cjx_index_read_package_model_bytes(const struct cjx_index_t *handle,
                                               const struct cjx_package_ref_t *package,
                                               cj_bytes_t *out_bytes);

cj_status_t cjx_index_read_package_by_record_id_model_bytes(const struct cjx_index_t *handle,
                                                            int64_t record_id,
                                                            bool *out_found,
                                                            cj_bytes_t *out_bytes);

cj_status_t cjx_index_read_packages_model_bytes(const struct cjx_index_t *handle,
                                                const struct cjx_package_ref_t *refs,
                                                uintptr_t ref_count,
                                                cj_bytes_t **out_items,
                                                uintptr_t *out_count);

cj_status_t cjx_index_read_filtered_packages(const struct cjx_index_t *handle,
                                             const struct cjx_package_ref_t *refs,
                                             uintptr_t ref_count,
                                             const struct cjx_package_filter_t *filter,
                                             struct cjx_filtered_package_t **out_packages,
                                             uintptr_t *out_count);

/**
 * # Safety
 *
 * `packages` must either be null or point to `count` filtered packages
 * allocated by `cjx_index_read_filtered_packages`.
 */
cj_status_t cjx_filtered_packages_free(struct cjx_filtered_package_t *packages, uintptr_t count);
