class TypedDictExample(TypedDict):
    """
    This is a TypedDict docstring that shouldn't mangle the code.
    """
    name: str
    age: int

def function_with_docstring():
    """
    This is a multi-line docstring.
    It should be removed while preserving indentation.
    """
    x = 1
    return x

def function_with_type_annotation(param: "Description of the parameter" = None):

    return param

class MyClass:
    """Class docstring."""

    def method(self):
        """Method docstring."""
        return True

    def complex_method(self):
        """
        This is a multi-line method docstring.
        It should be removed properly.
        """

        if True:

            return "test"


x = 5
