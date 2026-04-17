(* This block comment should be removed *)

let greeting = "Hello"

(* TODO: support more greetings *)
let fake = "This (* is not a comment *)"

(*
  This multiline comment
  should be removed
*)
let greet name =
  Printf.printf "%s, %s!\n" greeting name

let () = greet "world"
