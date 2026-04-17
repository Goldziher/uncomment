# This line comment should be removed
// This line comment should also be removed

/* This block comment
   should be removed */

resource "aws_instance" "example" {
  ami           = "ami-12345678"
  instance_type = "t2.micro"
  # TODO: add tags
  name = "hello // world"
}
