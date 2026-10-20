# ADR 001: CityObject selection and geometry filters

- Status: Accepted
- Date: 2026-08-01
- Scope: cityjson-lib materialized selections

## Context

cityjson-lib represents a selection as an opaque ModelSelection. Internally, each
retained CityObject is either Whole or Partial(H), where H is a set of attached
geometry handles.

The bug reported in cityjson/cjio2#2 exposed three places where the empty
partial state was erased:

1. include_relatives added newly reached objects as Whole, leaking their
   geometry into a geometry-filtered result.
2. intersection dropped an object when its two selected geometry sets were
   disjoint.
3. extraction dropped Partial(∅), so a structurally required object could not
   survive without geometry.

The representation could already express Partial(∅). The problem was not a
missing enum variant; it was inconsistent production, composition, and
interpretation of that state.

## Decision

### Extensional model

Fix one source model M. Let:

- O be its CityObjects;
- G be its stored geometries;
- A_M ⊆ O × G be its CityObject-to-geometry attachment relation.

A materialized selection is a pair S = (C, A), where:

- C ⊆ O is the set of retained CityObjects;
- A ⊆ A_M is the set of retained geometry attachments;
- dom(A) ⊆ C.

C and A are separate on purpose. An object can be retained for structure while
none of its geometry attachments are retained.

The current representation denotes this model as follows:

| Internal state | Extensional meaning |
| --- | --- |
| no map entry | object is not in C |
| Whole | object is in C; all of its source attachments are in A |
| Partial(H) | object is in C; exactly the attachments in H are in A |
| Partial(∅) | object is in C; it has no retained geometry attachment |

Whole and Partial(all attached handles) are equivalent for extraction, although
they are not the same structural value.

### Producers

select_cityobjects retains every matching object and all of its attached
geometries.

select_geometries retains every matching attachment and its owning object. An
object with no matching geometry is not introduced by this producer.

Future predicate, index, or query-language front ends may materialize results
into the same (C, A) carrier. They do not need to change its algebra.

### Union and intersection

For valid selections over the same source model:

- (C₁, A₁) ∪ (C₂, A₂) = (C₁ ∪ C₂, A₁ ∪ A₂)
- (C₁, A₁) ∩ (C₂, A₂) = (C₁ ∩ C₂, A₁ ∩ A₂)

The important intersection case is:

- both operands retain the same object;
- their geometry subsets for that object are disjoint;
- the result retains the object as Partial(∅).

By contrast, selections with no common CityObject have an empty intersection.

At the extensional level, ordinary set laws apply component-wise: commutativity,
associativity, idempotence, absorption, and distributivity. These statements
assume valid handles from one fixed source model. They are an informal
mathematical argument, not a machine-checked proof.

### Relative closure

Let R be the graph formed by parent and child references, and let R*(C) be the
objects reachable from C by zero or more steps in either direction.

include_relatives implements:

    close_M(C, A) = (R*(C), A)

It therefore:

- preserves every existing object and geometry choice;
- adds newly reached relatives without geometry;
- is extensive, monotone, and idempotent;
- distributes over union, but not generally over intersection.

Closure is deliberately separate from union and intersection. Apply it after
set composition unless a workflow specifically wants different ordering
semantics:

    materialize predicates → union/intersection → include relatives → extract

### Extraction and emptiness

Extraction materializes every object in C. Its geometry list contains exactly
the attachments selected for that object, including an empty list for
Partial(∅).

Parent and child references are copied only when both endpoints survive.
Extraction preserves reciprocal relationships only if the source model already
stores them reciprocally; it does not repair one-sided input.

is_empty is object-level:

    is_empty(S) ⇔ C = ∅

A selection containing a geometry-free CityObject is not empty.

### Predicate conjunction is different

A conjunction evaluated on one geometry asks whether that same geometry
matches both predicates. Intersecting two already materialized selections also
intersects their CityObject sets.

For example, one geometry of an object may match LoD 1 and another may match
LoD 2.2. Direct predicate conjunction retains nothing. Intersection of the two
materialized selections retains the common CityObject without geometry.

Callers must choose the operation that expresses their intent.

## Minimum implementation

This decision requires no public API, ABI, or representation change. The
minimum implementation is the three state-preservation fixes described above,
plus regression tests and corrected API documentation.

The contract is limited to CityObjects and their CityObject.geometry
attachments. It does not model address-location geometry, vertices, materials,
textures, semantics, arbitrary JSON paths, or other substructure as independent
selection dimensions.

ModelSelection is valid only for the source model whose handles it contains.
The current API does not encode model identity or reliably reject every foreign
selection: handles contain slot and generation information, not pool
provenance. Callers must not combine or extract selections across models.

## Test obligations

The minimum deterministic suite covers:

- the cjio2 LoD regression: a selected LoD 2.2 child keeps its parent but not the
  parent's LoD 0 geometry;
- preservation of parent and child references after extraction;
- Whole/Partial union and intersection;
- disjoint geometry subsets on the same object retaining Partial(∅);
- selections with no common CityObject remaining empty;
- equivalent behavior through Rust, the C ABI, C++, and Python.

A future algebra expansion should add generated valid same-model selections and
check the stated laws extensionally. Property tests must generate the model and
its selections together; arbitrary or cross-model handles are invalid inputs.

## How this enables later work

The opaque carrier and separate C/A semantics leave several compatible
extension points:

- new attribute, type, LoD, spatial, temporal, indexed, or remote predicate
  producers can target the same materialized carrier;
- the internal map can become bitsets, normalized sets, or a provenance-aware
  representation without changing the public type;
- closure can gain parent-only, child-only, bounded-depth, or policy-driven
  variants without redefining set composition;
- difference and complement can be added after defining an explicit source
  universe;
- additional attachment dimensions can extend the model when selecting only
  CityObject.geometry is no longer sufficient.

These are enabled directions, not commitments in this ADR.

## Consequences

Positive:

- geometry filtering no longer leaks geometry from structurally required
  relatives;
- object retention and geometry retention have one consistent meaning through
  production, composition, and extraction;
- later selection front ends can share a small algebra instead of inventing
  workflow-specific merge rules.

Costs and constraints:

- a non-empty selection may extract CityObjects with no geometry;
- intersection of materialized selections is not interchangeable with predicate
  conjunction;
- include_relatives remains order-sensitive with respect to intersection;
- cross-model validity remains a caller precondition.

## Rejected alternatives

Treating Partial(∅) as object exclusion collapses C and A and recreates the
reported bug.

Adding a new public selection representation is unnecessary for the minimum
fix. Whole | Partial(H) already expresses every state required here.

Automatically adding relatives inside union or intersection hides ordering
semantics and couples graph closure to set composition.

## Verification status

The algebra above is a concise, conditional correctness argument. There is no
Lean development in this repository, and this ADR does not claim formal
verification. Executable confidence comes from the regression suite and CI.

For the underlying theory and a path toward a real formalization, see the
[selection algebra reading guide](001-selection-algebra-reading-guide.md).

## References

- [cityjson/cjio2 issue 2](https://github.com/cityjson/cjio2/issues/2)
- [cityjson-rs pull request 23](https://github.com/3DGI/cityjson-rs/pull/23)
