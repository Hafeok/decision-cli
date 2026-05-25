"""Dataclass definitions over SHACL/RDF constants for the codegen pipeline."""

from __future__ import annotations

from dataclasses import dataclass

NS_DEC = "https://decision-cli.dev/ns#"
NS_SH = "http://www.w3.org/ns/shacl#"
NS_RDF = "http://www.w3.org/1999/02/22-rdf-syntax-ns#"
NS_XSD = "http://www.w3.org/2001/XMLSchema#"

SH_NODE_SHAPE = f"{NS_SH}NodeShape"
SH_TARGET_CLASS = f"{NS_SH}targetClass"
SH_PROPERTY = f"{NS_SH}property"
SH_PATH = f"{NS_SH}path"
SH_CLASS = f"{NS_SH}class"
SH_DATATYPE = f"{NS_SH}datatype"
SH_MIN_COUNT = f"{NS_SH}minCount"
SH_MAX_COUNT = f"{NS_SH}maxCount"
SH_AND = f"{NS_SH}and"
SH_OR = f"{NS_SH}or"
RDF_FIRST = f"{NS_RDF}first"
RDF_REST = f"{NS_RDF}rest"
RDF_NIL = f"{NS_RDF}nil"
RDF_TYPE = f"{NS_RDF}type"

BOUNDARY_ARTIFACT = f"{NS_DEC}BoundaryArtifact"

DATATYPE_TO_PYTHON: dict[str, str] = {
    f"{NS_XSD}string": "str",
    f"{NS_XSD}boolean": "bool",
    f"{NS_XSD}integer": "int",
    f"{NS_XSD}int": "int",
    f"{NS_XSD}long": "int",
    f"{NS_XSD}decimal": "float",
    f"{NS_XSD}double": "float",
    f"{NS_XSD}float": "float",
    f"{NS_XSD}dateTime": "str",
    f"{NS_XSD}date": "str",
}


@dataclass(frozen=True)
class PropertyField:
    """A required body field on the shape (``sh:property`` + ``sh:datatype``)."""

    local_name: str
    iri: str
    datatype_iri: str
    python_type: str
    required: bool
    single_valued: bool


@dataclass(frozen=True)
class EdgeField:
    """A typed forward edge (``sh:property`` + ``sh:class``)."""

    local_name: str
    iri: str
    range_class_iri: str
    range_class_local: str
    required: bool
    single_valued: bool


@dataclass(frozen=True)
class MotivationalAlternative:
    """One alternative inside the shape's ``sh:or`` (excluding boundary)."""

    predicate_local: str
    predicate_iri: str
    target_class_iri: str
    target_class_local: str


@dataclass(frozen=True)
class ShapeSpec:
    """All the per-shape metadata the emitters need."""

    source_file: str
    shape_iri: str
    target_class_iri: str
    target_class_local: str
    fields: tuple[PropertyField, ...] = ()
    edges: tuple[EdgeField, ...] = ()
    motivational: tuple[MotivationalAlternative, ...] = ()
    accepts_boundary: bool = True
