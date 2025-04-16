from typing import TypedDict, Optional, Dict, List, Union, Any

class UserData(TypedDict):
    """
    Represents user data structure in our system.
    This is a multi-line docstring.
    """
    id: str
    name: str
    email: str
    age: int
    is_active: bool

class NestedTypedDict(TypedDict):
    """This is an inline docstring."""
    user: UserData
    preferences: Dict[str, Any]

def process_user(user: "UserData") -> Optional[Dict[str, Any]]:
    """
    Process user information.

    Args:
        user: The user data to process

    Returns:
        Processed user data or None if invalid
    """
    if not user["id"]:
        return None


    result = {
        "processed_id": f"user_{user['id']}",
        "display_name": user["name"].upper(),
        "contact": user["email"]
    }

    return result

class UserManager:
    """User manager service"""

    def __init__(self, database_connection: str):
        """
        Initialize the user manager.

        Args:
            database_connection: Connection string to the database
        """
        self.db_conn = database_connection
        self.users: List[UserData] = []

    def get_user(self, user_id: str) -> Optional[UserData]:
        """Fetch user by ID"""
        for user in self.users:
            if user["id"] == user_id:
                return user
        return None
