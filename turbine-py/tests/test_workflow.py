"""Tests for workflow primitives: chain, group, chord."""

import pytest

from turbine.task import task
from turbine.workflow import Chain, Chord, Group, Signature, chain, chord, group


@task
def add(x, y):
    return x + y


@task
def multiply(x, y):
    return x * y


@task
def process(data):
    return data


class TestSignature:
    """Test Signature class."""

    def test_signature_creation(self):
        """Test creating a signature."""
        sig = add.s(1, 2)

        assert isinstance(sig, Signature)
        assert sig.task == add
        assert sig.args == (1, 2)
        assert sig.kwargs == {}
        assert sig.immutable is False

    def test_signature_with_kwargs(self):
        """Test signature with keyword arguments."""
        sig = Signature(add, args=(1,), kwargs={"y": 2})

        assert sig.args == (1,)
        assert sig.kwargs == {"y": 2}

    def test_immutable_signature(self):
        """Test immutable signature."""
        sig = add.si(1, 2)

        assert sig.immutable is True

    def test_signature_clone(self):
        """Test cloning a signature."""
        sig1 = add.s(1, 2)
        sig2 = sig1.clone(args=(0,))

        assert sig2.args == (0, 1, 2)  # Prepended
        assert sig1 is not sig2

    def test_signature_set_options(self):
        """Test setting options on signature."""
        sig = add.s(1, 2)
        sig.set(queue="high-priority", countdown=60)

        assert sig.options["queue"] == "high-priority"
        assert sig.options["countdown"] == 60

    def test_signature_to_dict(self):
        """Test signature serialization."""
        sig = add.s(1, 2)
        sig.set(queue="test")

        d = sig.to_dict()

        assert d["name"] == add.name
        assert d["args"] == [1, 2]
        assert d["kwargs"] == {}
        assert d["options"]["queue"] == "test"

    def test_signature_or_creates_chain(self):
        """Test | operator creates chain."""
        chain_result = add.s(1, 2) | multiply.s(3)

        assert isinstance(chain_result, Chain)
        assert len(chain_result.tasks) == 2


class TestChain:
    """Test Chain class."""

    def test_chain_creation(self):
        """Test creating a chain."""
        c = chain(add.s(1, 2), multiply.s(3), process.s())

        assert isinstance(c, Chain)
        assert len(c.tasks) == 3

    def test_chain_from_signatures(self):
        """Test chain from signatures."""
        c = Chain(add.s(1, 2), multiply.s(3))

        assert len(c) == 2
        assert c.tasks[0].task == add
        assert c.tasks[1].task == multiply

    def test_chain_or_appends(self):
        """Test | operator appends to chain."""
        c1 = chain(add.s(1, 2))
        c2 = c1 | multiply.s(3)

        assert len(c2) == 2
        assert c2.tasks[1].task == multiply

    def test_chain_nested(self):
        """Test nested chains are flattened."""
        c1 = chain(add.s(1, 2), multiply.s(3))
        c2 = chain(c1, process.s())

        assert len(c2) == 3

    def test_chain_using_pipe_operator(self):
        """Test chain using | operator."""
        c = add.s(1, 2) | multiply.s(3) | process.s()

        assert isinstance(c, Chain)
        assert len(c) == 3


class TestGroup:
    """Test Group class."""

    def test_group_creation(self):
        """Test creating a group."""
        g = group(add.s(1, 2), add.s(3, 4), add.s(5, 6))

        assert isinstance(g, Group)
        assert len(g.tasks) == 3

    def test_group_iteration(self):
        """Test iterating over group."""
        g = group(add.s(1, 2), multiply.s(3, 4))

        tasks = list(g)
        assert len(tasks) == 2
        assert tasks[0].task == add
        assert tasks[1].task == multiply

    def test_group_or_creates_chord(self):
        """Test | operator on group creates chord."""
        g = group(add.s(1, 2), add.s(3, 4))
        c = g | process.s()

        assert isinstance(c, Chord)
        assert c.group == g
        assert c.callback.task == process


class TestChord:
    """Test Chord class."""

    def test_chord_creation(self):
        """Test creating a chord."""
        c = chord(
            [add.s(1, 2), add.s(3, 4)],
            process.s()
        )

        assert isinstance(c, Chord)
        assert len(c.group) == 2
        assert c.callback.task == process

    def test_chord_from_group(self):
        """Test chord from Group object."""
        g = group(add.s(1, 2), add.s(3, 4))
        c = chord(g, process.s())

        assert c.group == g

    def test_chord_using_pipe(self):
        """Test creating chord using | operator."""
        c = group(add.s(1, 2), add.s(3, 4)) | process.s()

        assert isinstance(c, Chord)
        assert len(c.group) == 2


class TestWorkflowRepr:
    """Test string representations."""

    def test_signature_repr(self):
        """Test signature repr."""
        sig = add.s(1, 2)
        assert "Signature" in repr(sig)
        assert "add" in repr(sig)

    def test_chain_repr(self):
        """Test chain repr."""
        c = chain(add.s(1, 2), multiply.s(3))
        assert "Chain" in repr(c)

    def test_group_repr(self):
        """Test group repr."""
        g = group(add.s(1, 2), add.s(3, 4))
        assert "Group" in repr(g)
        assert "2 tasks" in repr(g)

    def test_chord_repr(self):
        """Test chord repr."""
        c = chord([add.s(1, 2)], process.s())
        assert "Chord" in repr(c)
