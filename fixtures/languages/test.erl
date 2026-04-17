-module(example).
-export([add/2]).

% This comment should be removed
greeting() -> "Hello % not a comment".

%% @doc Doc comment for the function
add(A, B) ->
    % TODO: add type guards
    A + B.

% nolint
X = 42.
