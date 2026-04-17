program test
  ! This comment should be removed
  implicit none

  character(len=20) :: name
  character(len=50) :: msg

  ! TODO: add command line argument parsing
  name = "world"
  msg = "Hello! This is not a comment"

  ! This comment should also be removed
  print *, trim(msg) // " " // trim(name) // "!"

end program test
