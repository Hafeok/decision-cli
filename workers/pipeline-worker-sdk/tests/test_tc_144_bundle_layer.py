"""TC-144: Curated query helpers over the in-memory bundle sub-graph.

Covers the three success criteria the parent feature_spec (FT-079) names:

1. ``bundle.focal()`` returns a typed Python object whose shape matches the
   bundle SHACL — Feature → ``FeatureAccessor``, ADR → ``ADRAccessor``,
   TC → ``TCAccessor``, etc. The accessor fields agree with the codegen
   output for the resolved type.
2. ``bundle.raw_store`` accesses bump a session-level telemetry counter
   (``bundle_raw_store_access_count``) that surfaces on the completion
   event — gap-surface signal for codegen-extension candidates.
3. Two workers reading the same store through fresh :class:`Bundle`
   instances get byte-identical return values for every curated
   accessor (deterministic / idempotent).

The codegen pipeline (FT-085) is exercised by TC-150; this test focuses
on the hand-written Bundle facade and its handoff to the Session's
telemetry surface.
"""

from __future__ import annotations

import pyoxigraph
import pytest

from pipeline_worker_sdk import (
    Bundle,
    DispatchEvent,
    Session,
    UnknownFocalTypeError,
)
from pipeline_worker_sdk.bundle import accessors
from pipeline_worker_sdk.bundle._facade import ADR_TYPE_IRI, TC_TYPE_IRI


# --------------------------------------------------------------------------- #
# Bundle fixture: a hand-rolled focal Feature + linked ADRs + applicable TCs.  #
# --------------------------------------------------------------------------- #

FEATURE_IRI = "urn:dec:feature:FT-079"
BRIEF_IRI = "urn:dec:brief:pipeline-worker-slice-1"
ADR_A_IRI = "urn:dec:adr:ADR-048"
ADR_B_IRI = "urn:dec:adr:ADR-016"
ADR_UNRELATED_IRI = "urn:dec:adr:ADR-999"
TC_A_IRI = "urn:dec:tc:TC-144"
TC_B_IRI = "urn:dec:tc:TC-150"
TC_UNRELATED_IRI = "urn:dec:tc:TC-999"
GRAPH_IRI = "urn:dec:bundle:fixture"


def _quad(s: str, p: str, o: str, *, lit: bool = False) -> str:
    obj = f'"{o}"' if lit else f"<{o}>"
    return f"<{s}> <{p}> {obj} <{GRAPH_IRI}> ."


def _bundle_nquads() -> str:
    rdf_type = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"
    dec = "https://decision-cli.dev/ns#"
    lines = [
        # focal Feature
        _quad(FEATURE_IRI, rdf_type, f"{dec}Feature"),
        _quad(FEATURE_IRI, f"{dec}decomposesFrom", BRIEF_IRI),
        # Two ADRs that decideFor the focal
        _quad(ADR_A_IRI, rdf_type, f"{dec}ADR"),
        _quad(ADR_A_IRI, f"{dec}decidesFor", FEATURE_IRI),
        _quad(ADR_B_IRI, rdf_type, f"{dec}ADR"),
        _quad(ADR_B_IRI, f"{dec}decidesFor", FEATURE_IRI),
        # An ADR that does NOT decideFor the focal (must be excluded)
        _quad(ADR_UNRELATED_IRI, rdf_type, f"{dec}ADR"),
        _quad(ADR_UNRELATED_IRI, f"{dec}decidesFor", "urn:dec:feature:FT-other"),
        # Two TCs that validate the focal
        _quad(TC_A_IRI, rdf_type, f"{dec}TC"),
        _quad(TC_A_IRI, f"{dec}validates", FEATURE_IRI),
        _quad(TC_B_IRI, rdf_type, f"{dec}TC"),
        _quad(TC_B_IRI, f"{dec}validates", FEATURE_IRI),
        # A TC that validates something else (must be excluded)
        _quad(TC_UNRELATED_IRI, rdf_type, f"{dec}TC"),
        _quad(TC_UNRELATED_IRI, f"{dec}validates", "urn:dec:feature:FT-other"),
    ]
    return "\n".join(lines) + "\n"


def _load_store(nquads: str) -> pyoxigraph.Store:
    store = pyoxigraph.Store()
    store.load(input=nquads, format=pyoxigraph.RdfFormat.N_QUADS)
    return store


def _make_dispatch(nquads: str) -> DispatchEvent:
    return DispatchEvent(
        event_id="1",
        dispatch_id="urn:dec:dispatch:FT-079",
        capability_tag="code-writer",
        nquads_payload=nquads,
        metadata={"role_id": "implementer"},
    )


# --------------------------------------------------------------------------- #
# Criterion #1 — focal() returns a typed accessor matching the SHACL shape.    #
# --------------------------------------------------------------------------- #


def test_focal_returns_typed_accessor_matching_shape() -> None:
    store = _load_store(_bundle_nquads())
    bundle = Bundle(store, FEATURE_IRI)

    focal = bundle.focal()

    assert isinstance(focal, accessors.FeatureAccessor)
    assert focal.iri == FEATURE_IRI
    # Field annotations declared by codegen are populated from the store.
    assert focal.decomposesFrom == (BRIEF_IRI,)
    # Unpopulated optional edges default to empty tuples / None per the
    # generated dataclass — and the facade does not invent values.
    assert focal.addresses == ()
    assert focal.originatedFrom == ()
    assert focal.respondsTo == ()


def test_focal_raises_when_type_has_no_generated_accessor() -> None:
    nq = (
        f"<{FEATURE_IRI}> "
        f"<http://www.w3.org/1999/02/22-rdf-syntax-ns#type> "
        f"<urn:dec:unknown-class:FooBar> <{GRAPH_IRI}> ."
    )
    bundle = Bundle(_load_store(nq), FEATURE_IRI)
    with pytest.raises(UnknownFocalTypeError):
        bundle.focal()


def test_focal_iri_property_reflects_constructor_argument() -> None:
    bundle = Bundle(_load_store(_bundle_nquads()), FEATURE_IRI)
    assert bundle.focal_iri == FEATURE_IRI


# --------------------------------------------------------------------------- #
# Curated linked accessors.                                                    #
# --------------------------------------------------------------------------- #


def test_linked_adrs_returns_only_decides_for_focal() -> None:
    bundle = Bundle(_load_store(_bundle_nquads()), FEATURE_IRI)
    adrs = bundle.linked_adrs()
    assert len(adrs) == 2
    iris = [a.iri for a in adrs]
    # Lexicographic order is part of the contract (cross-worker stability).
    assert iris == sorted(iris)
    assert ADR_UNRELATED_IRI not in iris
    for a in adrs:
        assert isinstance(a, accessors.ADRAccessor)
        assert a.decidesFor == (FEATURE_IRI,)


def test_applicable_test_criteria_returns_only_validates_focal() -> None:
    bundle = Bundle(_load_store(_bundle_nquads()), FEATURE_IRI)
    tcs = bundle.applicable_test_criteria()
    assert len(tcs) == 2
    iris = [t.iri for t in tcs]
    assert iris == sorted(iris)
    assert TC_UNRELATED_IRI not in iris
    for t in tcs:
        assert isinstance(t, accessors.TCAccessor)
        assert t.validates == (FEATURE_IRI,)


def test_accessors_of_type_returns_all_of_that_type_in_lex_order() -> None:
    bundle = Bundle(_load_store(_bundle_nquads()), FEATURE_IRI)
    adrs = bundle.accessors_of_type(ADR_TYPE_IRI)
    iris = [a.iri for a in adrs]
    assert iris == sorted([ADR_A_IRI, ADR_B_IRI, ADR_UNRELATED_IRI])
    tcs = bundle.accessors_of_type(TC_TYPE_IRI)
    assert [t.iri for t in tcs] == sorted([TC_A_IRI, TC_B_IRI, TC_UNRELATED_IRI])


def test_accessors_of_type_rejects_unknown_type() -> None:
    bundle = Bundle(_load_store(_bundle_nquads()), FEATURE_IRI)
    with pytest.raises(UnknownFocalTypeError):
        bundle.accessors_of_type("urn:nope")


# --------------------------------------------------------------------------- #
# Criterion #3 — determinism / idempotence across calls and workers.           #
# --------------------------------------------------------------------------- #


def test_repeated_focal_calls_return_equal_accessors() -> None:
    bundle = Bundle(_load_store(_bundle_nquads()), FEATURE_IRI)
    first = bundle.focal()
    second = bundle.focal()
    third = bundle.focal()
    # Frozen dataclasses are equal iff every field is equal — byte-identical.
    assert first == second == third


def test_two_independent_bundles_over_same_store_agree_byte_for_byte() -> None:
    """The cross-worker stability claim: two SDK Bundle instances over the
    same store return exactly the same tuple of accessors for every curated
    method. ``repr()`` is a stable serialization for frozen dataclasses, so
    equality on that string is the strongest cross-worker invariant the
    suite can express in-process.
    """
    store = _load_store(_bundle_nquads())
    a = Bundle(store, FEATURE_IRI)
    b = Bundle(store, FEATURE_IRI)
    assert a.focal() == b.focal()
    assert a.linked_adrs() == b.linked_adrs()
    assert a.applicable_test_criteria() == b.applicable_test_criteria()
    # repr equality catches field-ordering drift between dataclasses even
    # in the unlikely case where __eq__ is overridden upstream.
    assert repr(a.focal()) == repr(b.focal())
    assert [repr(x) for x in a.linked_adrs()] == [
        repr(x) for x in b.linked_adrs()
    ]
    assert [repr(x) for x in a.applicable_test_criteria()] == [
        repr(x) for x in b.applicable_test_criteria()
    ]


def test_two_independent_stores_loaded_from_same_nquads_agree() -> None:
    """Even when the underlying ``pyoxigraph.Store`` is rebuilt from
    scratch, the facade's accessors are stable on identical input. This
    mirrors what happens when two worker processes ingest the same
    N-Quads payload (FT-077 wire surface) and read through their own
    Session-owned stores.
    """
    nq = _bundle_nquads()
    bundle_x = Bundle(_load_store(nq), FEATURE_IRI)
    bundle_y = Bundle(_load_store(nq), FEATURE_IRI)
    assert repr(bundle_x.focal()) == repr(bundle_y.focal())
    assert tuple(repr(a) for a in bundle_x.linked_adrs()) == tuple(
        repr(a) for a in bundle_y.linked_adrs()
    )
    assert tuple(repr(t) for t in bundle_x.applicable_test_criteria()) == tuple(
        repr(t) for t in bundle_y.applicable_test_criteria()
    )


# --------------------------------------------------------------------------- #
# Criterion #2 — raw_store telemetry counter surfaces on the completion event. #
# --------------------------------------------------------------------------- #


def test_raw_store_increments_facade_counter() -> None:
    bundle = Bundle(_load_store(_bundle_nquads()), FEATURE_IRI)
    assert bundle.raw_store_access_count == 0
    _ = bundle.raw_store
    assert bundle.raw_store_access_count == 1
    _ = bundle.raw_store
    _ = bundle.raw_store
    assert bundle.raw_store_access_count == 3


def test_raw_store_does_not_increment_for_curated_accessors() -> None:
    """The whole point of the curated surface is that it does NOT trip the
    gap-surface signal. The facade reaches through to ``self._store``
    directly for curated queries; ``raw_store`` is only the property the
    *worker* invokes by name."""
    bundle = Bundle(_load_store(_bundle_nquads()), FEATURE_IRI)
    bundle.focal()
    bundle.linked_adrs()
    bundle.applicable_test_criteria()
    assert bundle.raw_store_access_count == 0


def test_session_bundle_surfaces_raw_store_count_in_completion_telemetry() -> None:
    dispatch = _make_dispatch(_bundle_nquads())
    session = Session(dispatch, session_iri="urn:dec:session:ft-079")
    bundle = session.bundle(FEATURE_IRI)
    # A curated call must not bump the counter.
    bundle.focal()
    assert session.raw_store_access_count == 0
    # Two raw accesses → counter == 2 on the session and on telemetry.
    _ = bundle.raw_store
    _ = bundle.raw_store
    assert session.raw_store_access_count == 2

    payload = session.build_completion()
    assert payload.telemetry["bundle_raw_store_access_count"] == 2


def test_session_bundle_telemetry_is_zero_when_facade_unused() -> None:
    """No bundle access at all (or curated-only access) leaves the counter
    at zero on the completion event."""
    dispatch = _make_dispatch(_bundle_nquads())
    session = Session(dispatch)
    bundle = session.bundle(FEATURE_IRI)
    bundle.focal()
    payload = session.build_completion()
    assert payload.telemetry["bundle_raw_store_access_count"] == 0


def test_independent_bundles_from_same_session_share_one_counter() -> None:
    """The session-level counter aggregates raw accesses across every
    Bundle the session minted — gap-surface signal is per-session, not
    per-bundle-instance."""
    dispatch = _make_dispatch(_bundle_nquads())
    session = Session(dispatch)
    a = session.bundle(FEATURE_IRI)
    b = session.bundle(FEATURE_IRI)
    _ = a.raw_store
    _ = b.raw_store
    _ = a.raw_store
    assert session.raw_store_access_count == 3
    payload = session.build_completion()
    assert payload.telemetry["bundle_raw_store_access_count"] == 3
