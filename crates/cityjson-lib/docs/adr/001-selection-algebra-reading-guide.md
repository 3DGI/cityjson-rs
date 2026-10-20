# Selection algebra reading guide

This guide maps the ideas in ADR 001 to a small set of primary papers,
textbooks, university material, and technical documentation. Read the “start”
item in each section first; use the deeper item when you want proofs or broader
context.

Links were followed and their destinations checked on 2026-08-01. “Publisher”
or “DOI” links may require library access; the remaining resources are freely
available.

## Suggested path

1. Relations and materialized query results
2. Orders and distributive lattices
3. Algebraic data types and invariants
4. Reachability, closure, and fixed points
5. Program meaning and extensional equivalence
6. Property-based testing
7. Lean and machine-checked proofs
8. Opaque handles and provenance

## 1. Relations and materialized query results

Start with E. F. Codd’s
[A Relational Model of Data for Large Shared Data Banks](https://research.ibm.com/publications/a-relational-model-of-data-for-large-shared-data-banks)
(free IBM record). It supplies the core habit used by the ADR: describe data as
sets of tuples and define operators by their denotation rather than storage
layout.

Then use [Database System Concepts](https://www.db-book.com/) (free companion
site for the textbook) for relational algebra, selection, projection, joins,
and query evaluation. The useful connection is that ModelSelection is a
materialized relation result, while predicate composition belongs to the query
that produces it.

Research question: write the ADR carrier as two relations, CityObjectSelected
and GeometryAttachmentSelected, and identify the foreign-key-like invariant
between them.

## 2. Orders and distributive lattices

Start with Davey and Priestley,
[Introduction to Lattices and Order](https://www.cambridge.org/core/books/introduction-to-lattices-and-order/946458CB6638AF86D85BA00F5787F4F4)
(publisher page). Focus on partial orders, meets, joins, product orders, and
distributive lattices.

For an executable formal library, browse
[Mathlib.Order.Lattice](https://leanprover-community.github.io/mathlib4_docs/Mathlib/Order/Lattice.html)
(free). It shows the precise vocabulary and theorem names used when these laws
are encoded in Lean.

Research question: order selections component-wise by subset and show that
union is the join and intersection is the meet.

## 3. Algebraic data types and invariants

Benjamin Pierce’s
[Types and Programming Languages](https://www.cis.upenn.edu/~bcpierce/tapl/)
(free author site and textbook resources) is the deeper source for typed
representations and operational reasoning.

For the Rust representation, read
[Defining an Enum](https://doc.rust-lang.org/book/ch06-01-defining-an-enum.html)
(free official documentation). Whole and Partial(H) are a sum type; the map
entry itself supplies the excluded case.

Scott Wlaschin’s
[Making illegal states unrepresentable](https://fsharpforfunandprofit.com/posts/designing-with-types-making-illegal-states-unrepresentable/)
(free practitioner article) is a useful design lens. Apply it carefully here:
the current enum can represent the needed states, but it does not enforce
same-model provenance.

Research question: compare the current map-plus-enum representation with the
extensional pair (C, A). List which invariants each representation enforces and
which remain dynamic preconditions.

## 4. Reachability, closure, and fixed points

Use MIT OpenCourseWare’s
[Introduction to Algorithms](https://ocw.mit.edu/courses/6-006-introduction-to-algorithms-fall-2011/)
(free course) for graph traversal and reachability.

For the order-theoretic foundation, read Tarski’s
[A lattice-theoretical fixpoint theorem and its applications](https://doi.org/10.2307/1990301)
(DOI; library access may be required). Closure operators and least fixed points
explain why repeated relative expansion stabilizes.

Research question: prove that reachable-set closure is extensive, monotone,
idempotent, and union-preserving. Construct a graph showing why it need not
preserve intersection.

## 5. Meaning and extensional equivalence

The Software Foundations chapter
[Program Equivalence](https://softwarefoundations.cis.upenn.edu/plf-current/Equiv.html)
(free) develops the distinction between representation and observable
behavior.

That distinction appears in ADR 001 because Whole and Partial(all attachments)
are structurally different but extract the same model fragment. It also
explains why algebraic laws should state whether equality is structural or
extensional.

Research question: define an extraction denotation and an equivalence relation
S ≈ T when both selections extract the same CityObjects and attachments.

## 6. Property-based testing

Read Claessen and Hughes,
[QuickCheck: a lightweight tool for random testing of Haskell programs](https://doi.org/10.1145/351240.351266)
(original paper DOI; library access may be required). An
[accessible paper copy](https://www.cs.tufts.edu/~nr/cs257/archive/john-hughes/quick.pdf)
is hosted in Tufts course material.

Then use the Rust
[proptest documentation](https://docs.rs/proptest/latest/proptest/) (free) for
strategies, shrinking, and failure persistence.

For this ADR, the generator must create a source model first and derive valid
selections from it. Generating handles independently tests undefined inputs,
not the algebra.

Research question: generate tiny attachment relations exhaustively before
moving to random graphs. Check commutativity, associativity, idempotence,
absorption, distributivity, and closure laws extensionally.

## 7. Lean and machine-checked proofs

Start with
[Theorem Proving in Lean 4](https://lean-lang.org/theorem_proving_in_lean4/)
(free official book) for propositions, sets, structures, and tactics.

Continue with
[Mathematics in Lean](https://leanprover-community.github.io/mathematics_in_lean/)
(free community textbook) for proof engineering with mathlib. Keep the lattice
API page from section 2 open while working.

A real proof artifact needs compilable Lean files, imports, definitions,
theorem statements, proofs, and a build checked in CI. Lean-flavored Markdown
is neither a specification accepted by Lean nor a proof.

Research question: formalize only the fixed-model pair (C, A) first. Prove
closure of union and intersection under dom(A) ⊆ C, then derive the lattice
laws from set laws.

## 8. Opaque handles and provenance

The
[Rust API Guidelines on future proofing](https://rust-lang.github.io/api-guidelines/future-proofing.html)
(free official-community guidance) explains why keeping ModelSelection opaque
preserves implementation freedom.

The [slotmap crate documentation](https://docs.rs/slotmap/latest/slotmap/)
(free) explains generational keys and their guarantees. Compare those
guarantees with model provenance: generation can reject stale slots within a
pool, but a key alone does not necessarily identify which pool owns it.

Research question: design a provenance-aware selection that rejects
cross-model union and extraction without exposing its internal geometry sets.

## A practical study exercise

Use a model with two CityObjects and three geometry attachments.

1. Enumerate every valid (C, A) selection.
2. Translate each selection to no entry, Whole, or Partial(H).
3. Compute union and intersection tables.
4. Add a parent/child edge and compute relative closure.
5. Compare direct conjunction of two geometry predicates with intersection of
   their materialized selections.
6. Turn the examples into deterministic tests.
7. Only then generalize them into property tests or Lean theorems.

This sequence keeps the mathematics tied to the observable cityjson-lib
contract.
