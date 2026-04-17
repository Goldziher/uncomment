# This is a line comment that should be removed
"""This is a module docstring that should be preserved."""


def greet(name):
    """Return a greeting for the given name."""
    # This comment should be removed
    url = "http://example.com#anchor"  # noqa: E501
    message = "Hello # world"
    # TODO: add logging here
    # FIXME: handle empty name
    return f"{message}, {name}!"


result = greet("Alice")
