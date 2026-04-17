package example

/** Scaladoc comment for the object */
object Main {
  // This comment should be removed
  val greeting = "Hello // not a comment"

  /* This block comment should be removed */
  def add(a: Int, b: Int): Int = {
    // TODO: add overflow check
    a + b
  }

  // noinspection ScalaStyle
  val x = 42
}
